import argparse
import copy
import json
import math
import sys
from itertools import chain

from check_headless_backend import mismatch, run_backend
from check_schemas import ROOT, apply_changes, load_json, parse_json, validator_for


def theme_source(case, base_text, foundation_text):
    variant = case.get("sourceVariant")
    sources = {}
    if variant:
        theme = parse_json(base_text, "illustrative theme source")
        tokens = theme.pop("tokens")
        if variant == "resolverBackedInline":
            theme["tokenResolver"] = {
                "version": "2025.10",
                "resolutionOrder": [{"type": "set", "name": "theme", "sources": [tokens]}],
            }
        else:
            del tokens["space"]
            del tokens["type"]["size"]
            theme["tokenResolver"] = {
                "version": "2025.10",
                "sets": {
                    "foundation": {"sources": [{"$ref": "foundation.json"}]},
                    "theme": {"sources": [tokens]},
                },
                "resolutionOrder": [
                    {"$ref": "#/sets/foundation"},
                    {"$ref": "#/sets/theme"},
                ],
            }
            for role in theme["spatialAssignments"]["roles"]:
                theme["spatialAssignments"]["roles"][role] = "space.4"
            for role in theme["typographyAssignments"]["roles"].values():
                role["fontSize"] = "type.size.4"
            if not case.get("omitFoundationSource"):
                sources["foundation.json"] = foundation_text
        theme["tokenInput"] = {}
        source = json.dumps(theme, ensure_ascii=False, allow_nan=False)
    else:
        source = base_text
    if "sourceReplace" in case:
        replacement = case["sourceReplace"]
        if source.count(replacement["find"]) != 1:
            raise ValueError(f"source replacement must match once: {case['name']}")
        source = source.replace(replacement["find"], replacement["with"], 1)
    if "sourceChanges" in case:
        theme = apply_changes(parse_json(source, case["name"]), case["sourceChanges"])
        source = json.dumps(theme, ensure_ascii=False, allow_nan=False)
    return source, sources


def source_cases():
    source_validator = validator_for("schemas/theme-source.schema.json")
    environment = load_json(ROOT / "conformance/headless/valid-request.json")["environment"]
    names = set()
    for vector in load_json(ROOT / "conformance/themes/source-vectors.json"):
        if (
            set(vector) != {"name", "document", "error"}
            or not isinstance(vector["name"], str)
            or not vector["name"].strip()
            or not isinstance(vector["error"], str)
            or not vector["error"].strip()
        ):
            raise ValueError("invalid theme source rejection vector")
        if vector["name"] in names:
            raise ValueError(f"duplicate theme source case name: {vector['name']}")
        names.add(vector["name"])
        if source_validator.is_valid(vector["document"]):
            raise ValueError(f"{vector['name']}: rejection vector is schema-valid")
        request = {
            "schemaVersion": "0.1.0",
            "themeSource": json.dumps(vector["document"], ensure_ascii=False, allow_nan=False),
            "externalSources": {},
            "environment": environment,
        }
        yield vector["name"], json.dumps(request, ensure_ascii=False, allow_nan=False), None


def cases():
    base_text = (ROOT / "conformance/themes/valid-source.json").read_text(encoding="utf-8")
    foundation_text = (ROOT / "tokens/foundation.json").read_text(encoding="utf-8")
    headless = load_json(ROOT / "conformance/headless/valid-request.json")
    expected = load_json(ROOT / "conformance/headless/expected-resolution.json")
    case_validator = validator_for("schemas/theme-resolution-case.schema.json")
    request_validator = validator_for("schemas/theme-resolution-request.schema.json")
    names = set()
    for case in load_json(ROOT / "conformance/themes/resolution-cases.json"):
        errors = list(case_validator.iter_errors(case))
        if errors:
            raise ValueError(f"{case.get('name')}: {errors[0].message}")
        if case["name"] in names:
            raise ValueError(f"duplicate theme resolution case name: {case['name']}")
        names.add(case["name"])
        source, external_sources = theme_source(case, base_text, foundation_text)
        request = {
            "schemaVersion": "0.1.0",
            "themeSource": source,
            "externalSources": external_sources,
            "environment": copy.deepcopy(headless["environment"]),
        }
        request = apply_changes(request, case.get("requestChanges", []))
        errors = list(request_validator.iter_errors(request))
        if bool(errors) == case["requestSchemaValid"]:
            detail = errors[0].message if errors else "unexpectedly valid"
            raise ValueError(f"{case['name']}: request schema: {detail}")
        wanted = apply_changes(expected, case.get("expectedChanges", []))
        yield case["name"], json.dumps(request, ensure_ascii=False, allow_nan=False), (
            wanted if case["outcome"] == "valid" else None
        )
    for vector in load_json(ROOT / "conformance/themes/request-vectors.json"):
        if (
            set(vector) != {"name", "document", "error"}
            or not isinstance(vector["name"], str)
            or not vector["name"].strip()
            or not isinstance(vector["error"], str)
            or not vector["error"].strip()
        ):
            raise ValueError("invalid theme request rejection vector")
        if vector["name"] in names:
            raise ValueError(f"duplicate theme resolution case name: {vector['name']}")
        names.add(vector["name"])
        if request_validator.is_valid(vector["document"]):
            raise ValueError(f"{vector['name']}: rejection vector is schema-valid")
        yield vector["name"], json.dumps(
            vector["document"], ensure_ascii=False, allow_nan=False
        ), None


def check_case(command, name, source, expected, result_validator, timeout):
    completed = run_backend(command, source, timeout)
    if expected is None:
        if completed.returncode != 1 or completed.stdout or not completed.stderr.strip():
            raise AssertionError(
                f"invalid request must exit 1 with empty stdout and a diagnostic; "
                f"got exit {completed.returncode}, stdout {completed.stdout[:200]!r}, "
                f"stderr {completed.stderr[:200]!r}"
            )
        return
    if completed.returncode != 0 or completed.stderr:
        raise AssertionError(
            f"valid request must exit 0 without stderr; got exit {completed.returncode}, "
            f"stderr {completed.stderr[:200]!r}"
        )
    actual = parse_json(completed.stdout, f"backend output for {name}")
    errors = list(result_validator.iter_errors(actual))
    if errors:
        raise AssertionError(f"output violates the result schema: {errors[0].message}")
    differing = mismatch(actual, expected)
    if differing:
        raise AssertionError(f"output differs from conformance result at {differing}")
    repeated = run_backend(command, source, timeout)
    if repeated.returncode != 0 or repeated.stderr:
        raise AssertionError("repeating a valid request changed its status or emitted a diagnostic")
    repeated_output = parse_json(repeated.stdout, f"repeat output for {name}")
    differing = mismatch(repeated_output, actual)
    if differing:
        raise AssertionError(f"repeating the same request changed the result at {differing}")


def main():
    parser = argparse.ArgumentParser(
        description="Check a Resina theme backend using the public stdin/stdout protocol."
    )
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or arguments.timeout <= 0 or not math.isfinite(arguments.timeout):
        parser.error("provide a backend command and a positive finite timeout")

    result_validator = validator_for("schemas/headless-result.schema.json")
    checked = 0
    names = set()
    for name, source, expected in chain(cases(), source_cases()):
        try:
            if name in names:
                raise ValueError(f"duplicate theme backend case name: {name}")
            names.add(name)
            check_case(command, name, source, expected, result_validator, arguments.timeout)
        except (AssertionError, OSError, ValueError) as error:
            print(f"FAIL {name}: {error}", file=sys.stderr)
            return 1
        checked += 1
    print(f"Theme backend passed {checked} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
