import argparse
import copy
import json
import math
import re
import sys

from backend_source import duplicate_member_source
from check_headless_backend import run_backend
from check_schemas import ROOT, apply_changes, load_json, parse_json, validator_for
from check_theme_derivation_backend import (
    derivation_mismatch, foundation_text, omit_at_pointer, template_texts,
)
from environmental_pigment import (
    TEMPLATE_HUE_CHROMA_FLOOR,
    ADAPTABLE_ROLES, FROST_ROLE, PigmentError, adapt, adapted_source, half_turn, template_roles,
)
from theme_derivation import (
    EPSILON, JND, THEMES, composed_tokens, gamut_map, guard_pairs,
    oklch_from_srgb,
)

FOUNDATION_URI = "foundation.json"
DEFAULT_PREFERENCES = {"reducedMotion": False, "reducedTransparency": False, "highContrast": False}


def profile():
    return load_json(ROOT / "definitions/environmental-pigment-profile.json")


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
    request = {
        "schemaVersion": "0.1.0",
        "profile": profile(),
        "accessibilityPreferences": copy.deepcopy(
            case.get("accessibilityPreferences", DEFAULT_PREFERENCES)),
        "templates": texts,
        "externalSources": external,
    }
    if "observation" in case:
        request["observation"] = copy.deepcopy(case["observation"])
    request = apply_changes(request, case.get("requestChanges", []))
    for pointer in case.get("requestOmit", []):
        omit_at_pointer(request, pointer)
    return json.dumps(request, ensure_ascii=False, allow_nan=False), request


def check_independent_result(result, request):
    applied = result["adaptation"]["applied"]
    for name in THEMES:
        theme = result["themes"][name]
        template = parse_json(request["templates"][name], f"{name} template")
        colors, alphas, _ = template_roles(template, request["externalSources"])
        pairs = [(pair["foreground"], pair["background"], pair["minimumContrast"])
                 for pair in theme["legibilityGuard"]]
        if pairs != guard_pairs():
            raise AssertionError(f"{name} theme must publish every legibility guard pair in order")
        if not all(pair["contrastRatio"] >= pair["minimumContrast"] for pair in theme["legibilityGuard"]):
            raise AssertionError(f"{name} theme published a failing guard pair")
        certification = theme.get("frostCertification")
        if certification is not None and any(
                entry["passed"] != (entry["contrastBound"] >= entry["minimumContrast"])
                for entry in certification):
            raise AssertionError(f"{name} Frost certification disagrees with its bounds")
        adapted_frost = FROST_ROLE in theme["roles"]
        if adapted_frost != (certification is not None and all(e["passed"] for e in certification)):
            raise AssertionError(f"{name} Frost chrome must adapt exactly when it is certified")
        if not applied and (theme["roles"] or certification is not None or theme["source"] != template):
            raise AssertionError(f"{name} theme changed although no adaptation was applied")
        if "range" not in request.get("observation", {}) and certification is not None:
            raise AssertionError(f"{name} certified Frost chrome without an observed range")
        ranges = request["profile"]["themes"][name]
        for role, adapted in theme["roles"].items():
            components = adapted["color"]["components"]
            if any(not isinstance(value, (int, float)) or isinstance(value, bool)
                   or not 0 <= value <= 1 for value in components):
                raise AssertionError(f"{name} {role} has an sRGB component outside [0, 1]")
            lightness, chroma, hue = adapted["targetOklch"]
            template_lightness, template_chroma, template_hue = oklch_from_srgb(colors[role])
            if template_chroma < TEMPLATE_HUE_CHROMA_FLOOR:
                template_hue = 0.0
            if abs(lightness - template_lightness) > 1e-9:
                raise AssertionError(f"{name} {role} changed lightness")
            if not ranges[role]["chroma"]["min"] <= chroma <= ranges[role]["chroma"]["max"]:
                raise AssertionError(f"{name} {role} chroma left its authored range")
            offset = half_turn(hue - template_hue)
            if chroma > 0 and not ranges[role]["hue"]["min"] - 1e-9 <= offset <= ranges[role]["hue"]["max"] + 1e-9:
                raise AssertionError(f"{name} {role} hue left its authored range")
            mapped = gamut_map(adapted["targetOklch"])
            if any(abs(left - right) > 1e-9 for left, right in zip(components, mapped)):
                raise AssertionError(f"{name} {role} is not the gamut mapping of its target")
            if abs(oklch_from_srgb(components)[0] - lightness) > JND + EPSILON:
                raise AssertionError(f"{name} {role} lightness moved more than the gamut mapping allows")
            if (role == FROST_ROLE) != ("alpha" in adapted["color"]) or (
                    role == FROST_ROLE and adapted["color"]["alpha"] != alphas[role]):
                raise AssertionError(f"{name} {role} must keep exactly its authored alpha")
        if not set(theme["roles"]) <= set(ADAPTABLE_ROLES):
            raise AssertionError(f"{name} theme adapted a role outside the eligible roles")
        expected = adapted_source(template, theme["roles"]) if theme["roles"] else template
        differing = derivation_mismatch(theme["source"], expected)
        if differing:
            raise AssertionError(f"{name} adapted source differs from its template at {differing}")


def cases():
    case_validator = validator_for("schemas/environmental-pigment-case.schema.json")
    request_validator = validator_for("schemas/environmental-pigment-request.schema.json")
    templates, foundation = template_texts(), foundation_text()
    names = set()
    for case in load_json(ROOT / "conformance/themes/environmental-pigment-vectors.json"):
        errors = list(case_validator.iter_errors(case))
        if errors:
            raise ValueError(f"{case.get('name')}: invalid case: {errors[0].message}")
        if case["name"] in names:
            raise ValueError(f"duplicate environmental pigment case name: {case['name']}")
        names.add(case["name"])
        source, request = build_request(case, templates, foundation)
        document = parse_json(source, case["name"]) if request is None else request
        request_errors = list(request_validator.iter_errors(document))
        if bool(request_errors) == case["requestSchemaValid"]:
            detail = request_errors[0].message if request_errors else "unexpectedly valid"
            raise ValueError(f"{case['name']}: request schema: {detail}")
        expected = None
        if case["outcome"] == "valid":
            independent = adapt(request)
            reduced = {"adaptation": independent["adaptation"], "themes": {
                name: {key: value for key, value in theme.items() if key != "source"}
                for name, theme in independent["themes"].items()}}
            differing = derivation_mismatch(reduced, case["expected"])
            if differing:
                raise ValueError(f"{case['name']}: independent adaptation differs from the vector at {differing}")
            expected = independent
        elif case["requestSchemaValid"]:
            try:
                adapt(request)
            except PigmentError as error:
                failed = sorted(re.findall(r"^(light|dark) theme:", str(error), re.MULTILINE))
                if failed != sorted(case.get("failingThemes", [])):
                    raise ValueError(f"{case['name']}: independent adaptation failed {failed}")
            else:
                raise ValueError(f"{case['name']}: independent adaptation unexpectedly succeeded")
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
        if named != (name in failing_themes):
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
            raise AssertionError(f"output differs from the adaptation at {differing}")
        if attempt and actual != previous:
            raise AssertionError("repeating the same request changed the result")
        previous = actual


def main():
    parser = argparse.ArgumentParser(
        description="Check a Resina environmental pigment backend through the public command protocol."
    )
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or arguments.timeout <= 0 or not math.isfinite(arguments.timeout):
        parser.error("provide a backend command and a positive finite timeout")
    result_validator = validator_for("schemas/environmental-pigment-result.schema.json")
    checked = 0
    baseline = None
    try:
        for case, source, request, expected in cases():
            try:
                if expected is None:
                    check_failure(command, source, arguments.timeout, case.get("failingThemes", ()))
                else:
                    if request.get("observation") is not None and baseline is None:
                        baseline = request
                    check_success(command, source, request, expected, result_validator, arguments.timeout)
            except AssertionError as error:
                raise AssertionError(f"{case['name']}: {error}") from error
            checked += 1
        if baseline is None:
            raise ValueError("environmental pigment conformance requires an adapted request")
        check_failure(command, duplicate_member_source(baseline, "/schemaVersion"), arguments.timeout)
        overflow = copy.deepcopy(baseline)
        overflow["observation"]["color"]["components"] = "observed-components"
        overflow = json.dumps(overflow, ensure_ascii=False).replace(
            '"observed-components"', "[1e400, 0, 0]", 1)
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
    print(f"Environmental pigment backend passed {checked} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
