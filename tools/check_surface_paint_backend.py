import argparse
import copy
import json
import math
import sys

from backend_source import duplicate_member_source, nonfinite_member_source
from opaque_request_source import opaque_request_failures
from surface_paint_source import positional_paint_request
from check_color_guard_backend import check_failure, check_success
from check_focus_ir_backend import focus_ir_mismatch
from check_extruded_contour_backend import contour_mismatch
from check_opaque_surface_backend import opaque_surface_mismatch
from check_headless_backend import mismatch
from check_schemas import ROOT, apply_changes, check_case, load_json, validator_for


def baseline():
    return {
        "schemaVersion": "0.1.0",
        "body": load_json(ROOT / "conformance/ir/opaque-surface-request.json"),
        "surroundingColor": {"colorSpace": "srgb", "components": [0, 0, 0], "alpha": 1},
    }


def expected_result(request):
    result = {"schemaVersion": "0.1.0", "body": load_json(ROOT / "conformance/ir/opaque-surface-expected.json")}
    states = [state for state in ("rest", "focused") if state in request["body"]["surface"]["states"]["states"]]
    result["body"]["states"]["states"] = states
    if "focused" in states:
        result["focus"] = load_json(ROOT / "conformance/ir/focus-ir-expected.json")
        result["focus"]["indicator"]["binding"]["states"]["states"] = states
    return result


def surface_paint_mismatch(actual, expected):
    if not isinstance(actual, dict) or actual.keys() != expected.keys():
        return "/"
    difference = opaque_surface_mismatch(actual["body"], expected["body"])
    if difference:
        return "/body" + difference
    if "focus" in actual:
        difference = focus_ir_mismatch(actual["focus"], expected["focus"])
        if difference:
            return "/focus" + difference
    return surface_paint_channel_mismatch(actual) or mismatch(actual["schemaVersion"], expected["schemaVersion"])


def surface_paint_channel_mismatch(result):
    if "focus" not in result:
        return None
    binding = result["focus"]["indicator"]["binding"]
    for field in ("states", "materialRole", "colorRole", "materialFamily", "form"):
        if mismatch(binding[field], result["body"][field]):
            return "/focus/indicator/binding/" + field
    if contour_mismatch(result["body"]["geometry"]["silhouette"], result["focus"]["geometry"]["silhouette"]):
        return "/focus/geometry/silhouette"
    return None


def case_request(base, case):
    request = apply_changes(base, case["requestChanges"])
    if case.get("omitSurroundingColor"):
        del request["surroundingColor"]
    return request


def main():
    parser = argparse.ArgumentParser(description="Check complete Resina surface body and navigation IR.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        parser.error("provide a backend command and a positive finite timeout")
    base = baseline()
    cases = load_json(ROOT / "conformance/ir/surface-paint-cases.json")
    case_validator = validator_for("schemas/surface-paint-case.schema.json")
    request_validator = validator_for("schemas/surface-paint-request.schema.json")
    result_validator = validator_for("schemas/surface-paint-ir.schema.json")
    names = set()
    try:
        for case in cases:
            name = case["name"]
            check_case(case_validator, name, case, True)
            if name in names:
                raise ValueError(f"duplicate surface paint case: {name}")
            names.add(name)
            request = case_request(base, case)
            check_case(request_validator, name, request, case["requestSchemaValid"])
            source = json.dumps(request, ensure_ascii=False, allow_nan=False)
            if "failure" in case:
                check_failure(command, source, arguments.timeout, name)
            else:
                result = expected_result(request)
                if ("focus" in result) != case["expectedFocus"]:
                    raise ValueError(f"{name}: expected focus disagrees with request")
                check_success(command, source, result, result_validator, arguments.timeout, name, surface_paint_mismatch)
        failed_focus = copy.deepcopy(base)
        failed_focus["body"]["surface"]["states"]["states"] = ["focused"]
        theme = json.loads(failed_focus["body"]["theme"]["themeSource"])
        theme["opaqueColorAssignments"]["roles"]["focus"] = "palette.black"
        failed_focus["body"]["theme"]["themeSource"] = json.dumps(theme)
        check_case(request_validator, "focus contrast failure request", failed_focus, True)
        failures = (
            ("focus failure publishes no body", json.dumps(failed_focus)),
            ("duplicate root version", duplicate_member_source(base, "/schemaVersion")),
            ("duplicate body state version", duplicate_member_source(base, "/body/surface/states/schemaVersion")),
            ("nonfinite body size", nonfinite_member_source(base, "/body/size/width")),
            ("unknown root member", json.dumps({**base, "unexpected": True})),
        ) + opaque_request_failures(base, "/body") + (
            ("positional surface paint request", positional_paint_request(base, "")),
        )
        for name, source in failures:
            check_failure(command, source, arguments.timeout, name)
    except (AssertionError, OSError, ValueError) as error:
        print(f"FAIL surface paint: {error}", file=sys.stderr)
        return 1
    print(f"Surface paint backend passed {len(cases) + len(failures)} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
