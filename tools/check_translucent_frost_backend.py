import argparse
import copy
import json
import math
import sys

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_failure, check_success
from check_extruded_contour_backend import contour_mismatch
from check_focus_ir_backend import focus_ir_mismatch
from check_headless_backend import mismatch, run_backend
from check_key_light_backend import key_light_mismatch
from check_opaque_surface_backend import opaque_surface_mismatch
from check_schemas import ROOT, apply_changes, check_case, load_json, parse_json, validator_for
from opaque_request_source import pigment_coefficient_failures
from translucent_frost_arithmetic import expected_result as oracle_result
from translucent_frost_arithmetic import remove_members

BASELINE = "conformance/ir/translucent-frost-request.json"
CASES = "conformance/ir/translucent-frost-cases.json"


def baseline():
    return load_json(ROOT / BASELINE)


def cases():
    return load_json(ROOT / CASES)


def case_request(base, case):
    request = apply_changes(base, case["requestChanges"])
    return remove_members(request, case.get("omitMembers", []))


def expected_result(request):
    return oracle_result(request)


def translucent_mismatch(actual, expected):
    if not isinstance(actual, dict) or actual.keys() != expected.keys():
        return "/"
    if expected["appearance"] == "opaque":
        paint, reference = actual["paint"], expected["paint"]
        if not isinstance(paint, dict) or paint.keys() != reference.keys():
            return "/paint"
        difference = opaque_surface_mismatch(paint["body"], reference["body"])
        if difference:
            return "/paint/body" + difference
        if "focus" in reference:
            difference = focus_ir_mismatch(paint["focus"], reference["focus"])
            if difference:
                return "/paint/focus" + difference
        stripped = {key: value for key, value in actual.items() if key != "paint"}
        base = {key: value for key, value in expected.items() if key != "paint"}
        return mismatch(stripped, base)
    body, reference = actual["body"], expected["body"]
    if not isinstance(body, dict) or body.keys() != reference.keys():
        return "/body"
    difference = contour_mismatch(body["geometry"], reference["geometry"])
    if difference:
        return "/body/geometry" + (difference if difference != "/" else "")
    difference = key_light_mismatch(body["lighting"], reference["lighting"])
    if difference:
        return "/body/lighting" + (difference if difference != "/" else "")
    if "focus" in expected:
        difference = focus_ir_mismatch(actual["focus"], expected["focus"])
        if difference:
            return "/focus" + difference

    def rest(document):
        return {
            key: ({k: v for k, v in value.items() if k not in ("geometry", "lighting")}
                  if key == "body" else value)
            for key, value in document.items() if key != "focus"
        }

    return mismatch(rest(actual), rest(expected))


RETAINED_FROM_ACTUAL = ("frostRepresentation", "frostPortableBody")


def point(level):
    color = {"colorSpace": "srgb", "components": [level] * 3, "alpha": 1}
    return {"lower": color, "upper": dict(color)}


def surface_paint_request(request):
    body = {
        key: copy.deepcopy(request[key])
        for key in (
            "theme", "surface", "size", "appearance", "foregroundRole",
            "minimumContentContrast", "minimumEdgeContrast",
        )
    }
    body["schemaVersion"] = "0.1.0"
    body["postTreatmentBackdrop"] = request["backdrop"]["lower"]
    body["adjacentColor"] = request["adjacentRanges"][0]["lower"]
    paint = {"schemaVersion": "0.1.0", "body": body}
    if "surroundingRanges" in request:
        paint["surroundingColor"] = request["surroundingRanges"][0]["lower"]
    return paint


def check_opaque_branch_is_surface_paint(base, command, surface_paint, timeout):
    for reason, level, high_contrast in (("selection", 0.9, True), ("legibility", 0.1, False)):
        for states in (["rest"], ["focused"], ["rest", "focused"]):
            name = f"opaque branch {reason} {'+'.join(states)}"
            request = copy.deepcopy(base)
            request["theme"]["environment"]["accessibilityPreferences"]["highContrast"] = high_contrast
            request["surface"]["states"]["states"] = states
            request["backdrop"] = point(level)
            request["adjacentRanges"] = [point(0.95)]
            request["surroundingRanges"] = [point(0.95)]
            if states == ["rest"]:
                del request["surroundingRanges"]
            source = json.dumps(request, allow_nan=False)
            results = []
            for backend, payload in (
                (command, source),
                (surface_paint, json.dumps(surface_paint_request(request))),
            ):
                completed = run_backend(backend, payload, timeout)
                if completed.returncode != 0 or completed.stderr:
                    raise AssertionError(f"{name}: backend failed with exit {completed.returncode}")
                results.append(parse_json(completed.stdout, name))
            actual, reference = results
            if actual["fallbackReason"] != reason or actual["appearance"] != "opaque":
                raise AssertionError(f"{name}: expected the {reason} opaque branch")
            paint = copy.deepcopy(actual["paint"])
            if "focus" in paint:
                binding = paint["focus"]["indicator"]["binding"]
                if (binding["frostRepresentation"] != "opaqueDimensional"
                        or binding["frostPortableBody"] != binding["opaqueColorFallback"]):
                    raise AssertionError(f"{name}: the ring does not retain the opaque representation")
                for result in (paint, reference):
                    for member in RETAINED_FROM_ACTUAL:
                        result["focus"]["indicator"]["binding"].pop(member)
            difference = mismatch(paint, reference)
            if difference:
                raise AssertionError(f"{name}: paint differs from surface paint at {difference}")


def source_failures(base):
    object_role = copy.deepcopy(base)
    object_role["foregroundRole"] = {base["foregroundRole"]: None}
    fields = list(base)
    return (
        ("duplicate root version", duplicate_member_source(base, "/schemaVersion")),
        ("duplicate diffusion radius", duplicate_member_source(base, "/diffusion/radius")),
        ("duplicate backdrop bound", duplicate_member_source(base, "/backdrop/lower/alpha")),
        ("duplicate adjacent bound", duplicate_member_source(base, "/adjacentRanges/0/upper/alpha")),
        ("nonfinite diffusion radius", nonfinite_member_source(base, "/diffusion/radius")),
        ("nonfinite backdrop alpha", nonfinite_member_source(base, "/backdrop/upper/alpha")),
        ("nonfinite edge threshold", nonfinite_member_source(base, "/minimumEdgeContrast")),
        ("nonfinite surface dimension", nonfinite_member_source(base, "/size/width")),
        ("unknown root member", json.dumps({**base, "unexpected": True})),
        ("object-encoded foreground role", json.dumps(object_role)),
        ("positional request", json.dumps([base[field] for field in fields])),
        ("null request", "null"),
        ("empty array request", "[]"),
    ) + pigment_coefficient_failures(base, "/appearance/pigmentProfiles")


def main():
    parser = argparse.ArgumentParser(description="Check Resina translucent Frost surface IR.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("--surface-paint", required=True,
                        help="the resina-surface-paint command the opaque branch is compared with")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        parser.error("provide a backend command and a positive finite timeout")

    base = baseline()
    public_cases = cases()
    case_validator = validator_for("schemas/translucent-frost-case.schema.json")
    request_validator = validator_for("schemas/translucent-frost-request.schema.json")
    result_validator = validator_for("schemas/translucent-frost-ir.schema.json")
    names = set()
    try:
        for case in public_cases:
            name = case["name"]
            check_case(case_validator, name, case, True)
            if name in names:
                raise ValueError(f"duplicate translucent Frost case: {name}")
            names.add(name)
            request = case_request(base, case)
            check_case(request_validator, name, request, case["requestSchemaValid"])
            source = json.dumps(request, ensure_ascii=False, allow_nan=False)
            if "errorContains" in case:
                if case["requestSchemaValid"]:
                    try:
                        oracle_result(request)
                    except ValueError as error:
                        if case["errorContains"] not in str(error):
                            raise AssertionError(f"{name}: oracle failed for the wrong reason: {error}")
                    else:
                        raise AssertionError(f"{name}: oracle accepted a negative case")
                check_failure(command, source, arguments.timeout, name)
            else:
                expected = oracle_result(request)
                for field, key in (
                    ("expectedAppearance", "appearance"),
                    ("expectedRepresentation", "frostRepresentation"),
                ):
                    if case[field] != expected[key]:
                        raise AssertionError(f"{name}: {field} disagrees with the oracle")
                if case.get("expectedFallbackReason") != expected.get("fallbackReason"):
                    raise AssertionError(f"{name}: expectedFallbackReason disagrees with the oracle")
                if case["expectedFocus"] != (
                    "focus" in expected or "focus" in expected.get("paint", {})
                ):
                    raise AssertionError(f"{name}: expectedFocus disagrees with the oracle")
                check_case(result_validator, name, expected, True)
                check_success(
                    command, source, expected, result_validator,
                    arguments.timeout, name, translucent_mismatch,
                )
        check_opaque_branch_is_surface_paint(
            base, command, [arguments.surface_paint, "-"], arguments.timeout
        )
        failures = source_failures(base)
        for name, source in failures:
            check_failure(command, source, arguments.timeout, name)
    except (AssertionError, KeyError, OSError, ValueError) as error:
        print(f"FAIL translucent Frost: {error}", file=sys.stderr)
        return 1
    print(f"Translucent Frost backend passed {len(public_cases) + len(failures)} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
