import argparse
import copy
import json
import math
import sys

from check_headless_backend import mismatch, run_backend
from check_schemas import ROOT, apply_changes, load_json, parse_json, validator_for


def baseline():
    resolution = load_json(ROOT / "conformance/headless/valid-request.json")
    resolution["colorAssignments"]["roles"]["content.primary"] = "palette.opaqueAlt"
    resolution["opaqueColorAssignments"]["roles"]["outline.strong"] = "palette.opaqueAlt"
    binding = load_json(ROOT / "conformance/surfaces/binding-vectors.json")[0]
    surface = copy.deepcopy(binding["document"])
    surface["states"]["states"] = ["rest"]
    surface["treatmentStack"]["treatments"] = ["none"]
    expected_binding = copy.deepcopy(binding["expected"])
    expected_binding["states"]["states"] = ["rest"]
    expected_binding["treatmentStack"]["treatments"] = ["none"]
    request = {
        "schemaVersion": "0.1.0",
        "scenario": {
            "schemaVersion": "0.4.0",
            "resolution": resolution,
            "surface": surface,
        },
        "foregroundRole": "content.primary",
        "postTreatmentBackdrop": {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1},
        "adjacentColor": {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1},
        "minimumContentContrast": 3,
        "minimumEdgeContrast": 3,
    }
    return request, expected_binding


def check_success(actual, request, expected, binding):
    binding = copy.deepcopy(binding)
    binding["states"]["states"] = [
        state for state in ("rest", "focused")
        if state in request["scenario"]["surface"]["states"]["states"]
    ]
    if mismatch(actual["binding"], binding):
        raise AssertionError("binding differs from the surface conformance result")
    if actual["foregroundRole"] != request["foregroundRole"]:
        raise AssertionError("foreground role changed")
    assigned = request["scenario"]["resolution"]["colorAssignments"]["roles"][request["foregroundRole"]]
    colors = request["scenario"]["resolution"]["tokens"]["palette"]
    token = colors[assigned.split(".", 1)[1]]["$value"]
    foreground = {"colorSpace": "srgb", "components": token["components"], "alpha": 1}
    if mismatch(actual["foreground"], foreground):
        raise AssertionError("foreground differs from its authored opaque assignment")
    legibility = actual["legibility"]
    edge = actual["edge"]
    for field, value in [
        (legibility["representation"], expected["representation"]),
        (legibility["fallbackApplied"], expected["contentFallback"]),
        (edge["colorRole"], expected["edgeRole"]),
        (edge["fallbackApplied"], expected["edgeFallback"]),
    ]:
        if field != value:
            raise AssertionError(f"expected {value!r}, got {field!r}")
    selected_body = binding["opaqueColorFallback"] if expected["contentFallback"] else binding["frostPortableBody"]
    if mismatch(legibility["body"], selected_body):
        raise AssertionError("legibility body differs from the bound Frost body or opaque fallback")
    edge_role = edge["colorRole"]
    edge_assignment = request["scenario"]["resolution"]["opaqueColorAssignments"]["roles"][edge_role]
    edge_token = colors[edge_assignment.split(".", 1)[1]]["$value"]
    edge_color = {"colorSpace": "srgb", "components": edge_token["components"], "alpha": 1}
    if mismatch(edge["color"], edge_color):
        raise AssertionError("edge color differs from its authored opaque assignment")


def main():
    parser = argparse.ArgumentParser(description="Check a Resina Frost surface readability backend.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or arguments.timeout <= 0 or not math.isfinite(arguments.timeout):
        parser.error("provide a backend command and a positive finite timeout")

    base, binding = baseline()
    cases = load_json(ROOT / "conformance/surfaces/frost-readability-cases.json")
    case_validator = validator_for("schemas/frost-surface-readability-case.schema.json")
    request_validator = validator_for("schemas/frost-surface-readability-request.schema.json")
    result_validator = validator_for("schemas/frost-surface-readability-result.schema.json")
    names = set()
    try:
        for case in cases:
            name = case["name"]
            if name in names or list(case_validator.iter_errors(case)):
                raise ValueError(f"duplicate or invalid case: {name}")
            names.add(name)
            request = apply_changes(base, case["changes"])
            valid = not list(request_validator.iter_errors(request))
            if valid != case["requestSchemaValid"]:
                raise ValueError(f"request schema disagrees with case: {name}")
            if "expected" in case and not valid:
                raise ValueError(f"successful request violates schema: {name}")
            source = json.dumps(request, ensure_ascii=False, allow_nan=False)
            completed = run_backend(command, source, arguments.timeout)
            if "errorContains" in case:
                if completed.returncode != 1 or completed.stdout or case["errorContains"] not in completed.stderr:
                    raise AssertionError(f"{name}: expected diagnostic failure; got {completed.returncode}, {completed.stderr[:200]!r}")
                continue
            if completed.returncode != 0 or completed.stderr:
                raise AssertionError(f"{name}: expected clean success; got {completed.returncode}, {completed.stderr[:200]!r}")
            actual = parse_json(completed.stdout, name)
            errors = list(result_validator.iter_errors(actual))
            if errors:
                raise AssertionError(f"{name}: result violates schema: {errors[0].message}")
            check_success(actual, request, case["expected"], binding)
            repeated = run_backend(command, source, arguments.timeout)
            if repeated.returncode != 0 or repeated.stderr or mismatch(parse_json(repeated.stdout, name), actual):
                raise AssertionError(f"{name}: repeated request changed its result")
        base_source = json.dumps(base)
        member = '"foregroundRole": "content.primary"'
        if base_source.count(member) != 1:
            raise ValueError("baseline foreground role is not unique")
        duplicate = base_source.replace(member, member + ', ' + member, 1)
        completed = run_backend(command, duplicate, arguments.timeout)
        if completed.returncode != 1 or completed.stdout or "duplicate JSON member" not in completed.stderr:
            raise AssertionError("duplicate request member was accepted")
    except (AssertionError, OSError, ValueError) as error:
        print(f"FAIL {error}", file=sys.stderr)
        return 1
    print(f"Frost surface readability backend passed {len(cases) + 1} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
