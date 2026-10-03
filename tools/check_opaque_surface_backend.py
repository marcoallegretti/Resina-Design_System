import argparse
import json
import math
import sys

from check_color_guard_backend import check_failure, check_success
from check_extruded_contour_backend import contour_mismatch
from check_headless_backend import mismatch
from check_key_light_backend import key_light_mismatch
from check_schemas import ROOT, apply_changes, check_case, load_json, validator_for


def opaque_surface_mismatch(actual, expected):
    if not isinstance(actual, dict):
        return "/"
    for name, compare in (("geometry", contour_mismatch), ("lighting", key_light_mismatch)):
        difference = compare(actual.get(name), expected[name])
        if difference:
            return f"/{name}" + (difference if difference != "/" else "")
    return mismatch(
        {**actual, "geometry": expected["geometry"], "lighting": expected["lighting"]},
        expected,
    )


def main():
    parser = argparse.ArgumentParser(description="Check a Resina opaque surface IR backend.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or arguments.timeout <= 0 or not math.isfinite(arguments.timeout):
        parser.error("provide a backend command and a positive finite timeout")
    case_schema = validator_for("schemas/opaque-surface-case.schema.json")
    request_schema = validator_for("schemas/opaque-surface-request.schema.json")
    result_schema = validator_for("schemas/opaque-surface-ir.schema.json")
    baseline = load_json(ROOT / "conformance/ir/opaque-surface-request.json")
    expected = load_json(ROOT / "conformance/ir/opaque-surface-expected.json")
    cases = load_json(ROOT / "conformance/ir/opaque-surface-cases.json")
    names = set()
    try:
        for case in cases:
            name = case["name"]
            check_case(case_schema, name, case, True)
            if name in names:
                raise ValueError(f"duplicate opaque surface case: {name}")
            names.add(name)
            request = apply_changes(baseline, case["requestChanges"])
            check_case(request_schema, name, request, case["requestSchemaValid"])
            source = json.dumps(request, ensure_ascii=False, allow_nan=False)
            if "errorContains" in case:
                check_failure(command, source, arguments.timeout, name)
            else:
                result = case.get(
                    "expected", apply_changes(expected, case.get("expectedChanges", []))
                )
                check_success(
                    command, source, result, result_schema, arguments.timeout,
                    name, opaque_surface_mismatch,
                )
        for name, source in (
            ("duplicate root version", '{"schemaVersion":"0.1.0","schemaVersion":"0.1.0"}'),
            ("duplicate nested band", '{"appearance":{"bands":{"cast":{"edgeWidth":1,"edgeWidth":2}}}}'),
            ("nonfinite surface dimension", '{"size":{"width":1e400}}'),
        ):
            check_failure(command, source, arguments.timeout, name)
    except (AssertionError, OSError, ValueError) as error:
        print(f"FAIL {error}", file=sys.stderr)
        return 1
    print(f"Opaque surface backend passed {len(cases) + 3} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
