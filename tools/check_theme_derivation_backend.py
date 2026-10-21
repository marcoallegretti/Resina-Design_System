import argparse
import copy
import json
import math
import re
import sys

from backend_source import duplicate_member_source
from check_headless_backend import run_backend
from check_schemas import ROOT, apply_changes, load_json, parse_json, validator_for
from theme_derivation import (
    DERIVED_ROLES, THEMES, DerivationError, composed_tokens, derive, derived_source, guard_pairs,
)

TOLERANCE = 1e-9
FOUNDATION_URI = "foundation.json"


def derivation_mismatch(actual, expected, pointer=""):
    if isinstance(expected, dict):
        if not isinstance(actual, dict) or actual.keys() != expected.keys():
            return pointer or "/"
        for name, value in expected.items():
            escaped = name.replace("~", "~0").replace("/", "~1")
            found = derivation_mismatch(actual[name], value, f"{pointer}/{escaped}")
            if found:
                return found
        return None
    if isinstance(expected, list):
        if not isinstance(actual, list) or len(actual) != len(expected):
            return pointer or "/"
        for index, value in enumerate(expected):
            found = derivation_mismatch(actual[index], value, f"{pointer}/{index}")
            if found:
                return found
        return None
    if isinstance(expected, bool) or isinstance(actual, bool):
        return None if actual is expected else pointer or "/"
    if isinstance(expected, (int, float)) and isinstance(actual, (int, float)):
        if not math.isfinite(actual):
            return pointer or "/"
        return None if abs(actual - expected) <= TOLERANCE else pointer or "/"
    return None if actual == expected else pointer or "/"


def omit_at_pointer(document, pointer):
    segments = [segment.replace("~1", "/").replace("~0", "~") for segment in pointer[1:].split("/")]
    current = document
    for segment in segments[:-1]:
        current = current[segment]
    if segments[-1] not in current:
        raise ValueError(f"omitted member does not exist: {pointer}")
    del current[segments[-1]]


def template_texts():
    return {name: (ROOT / f"tokens/themes/{name}.json").read_text(encoding="utf-8") for name in THEMES}


def foundation_text():
    return (ROOT / "tokens/foundation.json").read_text(encoding="utf-8")


def build_request(case, templates=None, foundation=None):
    if "requestDocument" in case:
        return json.dumps(case["requestDocument"], ensure_ascii=False, allow_nan=False), None
    templates = template_texts() if templates is None else templates
    foundation = foundation_text() if foundation is None else foundation
    external = {FOUNDATION_URI: foundation}
    texts = {}
    for name in THEMES:
        theme = parse_json(templates[name], f"{name} template")
        if case.get("templateVariant") == "embeddedTokens":
            theme["tokens"] = composed_tokens(theme, external)
            del theme["tokenResolver"], theme["tokenInput"]
        changes = [change for change in case.get("templateChanges", []) if change["theme"] == name]
        if changes:
            theme = apply_changes(theme, changes)
        texts[name] = json.dumps(theme, ensure_ascii=False, allow_nan=False)
    request = {"schemaVersion": "0.1.0", "templates": texts, "externalSources": external}
    if "seed" in case:
        request["seed"] = copy.deepcopy(case["seed"])
    request = apply_changes(request, case.get("requestChanges", []))
    for pointer in case.get("requestOmit", []):
        omit_at_pointer(request, pointer)
    return json.dumps(request, ensure_ascii=False, allow_nan=False), request


def expected_sources(request):
    return {
        name: parse_json(request["templates"][name], f"{name} template")
        for name in THEMES
    }


def check_independent_result(result, request):
    templates = expected_sources(request)
    for name in THEMES:
        theme = result["themes"][name]
        if set(theme["roles"]) != set(DERIVED_ROLES):
            raise AssertionError(f"{name} theme must derive exactly {', '.join(DERIVED_ROLES)}")
        pairs = [(pair["foreground"], pair["background"], pair["minimumContrast"])
                 for pair in theme["legibilityGuard"]]
        if pairs != guard_pairs():
            raise AssertionError(f"{name} theme must publish every legibility guard pair in order")
        for pair in theme["legibilityGuard"]:
            if not pair["contrastRatio"] >= pair["minimumContrast"]:
                raise AssertionError(f"{name} theme published a failing guard pair")
        mapped = {}
        for role in DERIVED_ROLES:
            components = theme["roles"][role]["color"]["components"]
            if any(not isinstance(value, (int, float)) or isinstance(value, bool)
                   or not 0 <= value <= 1 for value in components):
                raise AssertionError(f"{name} {role} has an sRGB component outside [0, 1]")
            mapped[role] = components
        differing = derivation_mismatch(theme["source"], derived_source(templates[name], mapped))
        if differing:
            raise AssertionError(f"{name} derived source differs from its template at {differing}")


def cases():
    case_validator = validator_for("schemas/theme-derivation-case.schema.json")
    request_validator = validator_for("schemas/theme-derivation-request.schema.json")
    templates, foundation = template_texts(), foundation_text()
    names = set()
    for case in load_json(ROOT / "conformance/themes/derivation-vectors.json"):
        errors = list(case_validator.iter_errors(case))
        if errors:
            raise ValueError(f"{case.get('name')}: invalid case: {errors[0].message}")
        if case["name"] in names:
            raise ValueError(f"duplicate theme derivation case name: {case['name']}")
        names.add(case["name"])
        source, request = build_request(case, templates, foundation)
        document = parse_json(source, case["name"]) if request is None else request
        request_errors = list(request_validator.iter_errors(document))
        if bool(request_errors) == case["requestSchemaValid"]:
            detail = request_errors[0].message if request_errors else "unexpectedly valid"
            raise ValueError(f"{case['name']}: request schema: {detail}")
        expected = None
        if case["outcome"] == "valid":
            independent = derive(request)
            reduced = {"seed": independent["seed"], "themes": {
                name: {key: value for key, value in theme.items() if key != "source"}
                for name, theme in independent["themes"].items()}}
            differing = derivation_mismatch(reduced, case["expected"])
            if differing:
                raise ValueError(f"{case['name']}: independent derivation differs from the vector at {differing}")
            expected = independent
        elif "failingThemes" in case:
            try:
                derive(request)
            except DerivationError as error:
                failed = sorted(re.findall(r"^(light|dark) theme:", str(error), re.MULTILINE))
                if failed != sorted(case["failingThemes"]):
                    raise ValueError(f"{case['name']}: independent derivation failed {failed}")
            else:
                raise ValueError(f"{case['name']}: independent derivation unexpectedly succeeded")
        yield case, source, request, expected


def check_failure(command, source, timeout, failing_themes=()):
    completed = run_backend(command, source, timeout)
    if completed.returncode != 1 or completed.stdout or not completed.stderr.strip():
        raise AssertionError(
            f"failure must exit 1 with empty stdout and a diagnostic; got exit "
            f"{completed.returncode}, stdout {completed.stdout[:200]!r}, stderr {completed.stderr[:200]!r}"
        )
    for name in THEMES:
        named = f"{name} theme" in completed.stderr
        if failing_themes and named != (name in failing_themes):
            raise AssertionError(f"diagnostic must name exactly the failing themes {failing_themes}")


def check_success(command, source, request, expected, result_validator, timeout):
    previous = None
    for attempt in range(2):
        completed = run_backend(command, source, timeout)
        if completed.returncode != 0 or completed.stderr:
            raise AssertionError(
                f"attempt {attempt + 1} failed with exit {completed.returncode}, "
                f"stderr {completed.stderr[:200]!r}"
            )
        actual = parse_json(completed.stdout, "backend output")
        errors = list(result_validator.iter_errors(actual))
        if errors:
            raise AssertionError(f"output violates the result schema at "
                                 f"/{'/'.join(map(str, errors[0].absolute_path))}: {errors[0].message[:200]}")
        check_independent_result(actual, request)
        differing = derivation_mismatch(actual, expected)
        if differing:
            raise AssertionError(f"output differs from the derivation at {differing}")
        if attempt and actual != previous:
            raise AssertionError("repeating the same request changed the result")
        previous = actual


def main():
    parser = argparse.ArgumentParser(
        description="Check a Resina theme derivation backend through the public command protocol."
    )
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or arguments.timeout <= 0 or not math.isfinite(arguments.timeout):
        parser.error("provide a backend command and a positive finite timeout")
    result_validator = validator_for("schemas/theme-derivation-result.schema.json")
    checked = 0
    baseline = None
    try:
        for case, source, request, expected in cases():
            try:
                if expected is None:
                    check_failure(command, source, arguments.timeout, case.get("failingThemes", ()))
                else:
                    baseline = baseline or request
                    check_success(command, source, request, expected, result_validator, arguments.timeout)
            except AssertionError as error:
                raise AssertionError(f"{case['name']}: {error}") from error
            checked += 1
        if baseline is None:
            raise ValueError("theme derivation conformance requires a successful request")
        check_failure(command, duplicate_member_source(baseline, "/schemaVersion"), arguments.timeout)
        overflow = copy.deepcopy(baseline)
        overflow["seed"]["components"] = "seed-components"
        overflow = json.dumps(overflow, ensure_ascii=False).replace('"seed-components"', "[1e400, 0, 0]", 1)
        check_failure(command, overflow, arguments.timeout)
        duplicate = copy.deepcopy(baseline)
        duplicate["templates"]["light"] = duplicate["templates"]["light"].replace(
            '"schemaVersion": "0.1.0"', '"schemaVersion": "0.1.0", "schemaVersion": "0.1.0"', 1)
        if duplicate["templates"]["light"] == baseline["templates"]["light"]:
            raise ValueError("duplicate template member could not be injected")
        check_failure(command, json.dumps(duplicate, ensure_ascii=False), arguments.timeout, ("light",))
        checked += 3
    except (AssertionError, OSError, ValueError) as error:
        print(f"FAIL {error}", file=sys.stderr)
        return 1
    print(f"Theme derivation backend passed {checked} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
