import argparse
import json
import math
import sys

from check_color_guard_backend import check_failure, check_success
from check_headless_backend import mismatch
from check_schemas import ROOT, apply_changes, check_case, load_json, validator_for


def check_delta_backend(
    label, case_schema, request_schema, result_schema, baseline_path, expected_path,
    cases_path, extra_failures=(), compare=mismatch,
):
    parser = argparse.ArgumentParser(description=f"Check a Resina {label} backend.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or arguments.timeout <= 0 or not math.isfinite(arguments.timeout):
        parser.error("provide a backend command and a positive finite timeout")
    case_validator = validator_for(case_schema)
    request_validator = validator_for(request_schema)
    result_validator = validator_for(result_schema)
    baseline = load_json(ROOT / baseline_path)
    expected = load_json(ROOT / expected_path)
    cases = load_json(ROOT / cases_path)
    names = set()
    try:
        for case in cases:
            name = case["name"]
            check_case(case_validator, name, case, True)
            if name in names:
                raise ValueError(f"duplicate {label} case: {name}")
            names.add(name)
            request = apply_changes(baseline, case["requestChanges"])
            check_case(request_validator, name, request, case["requestSchemaValid"])
            source = json.dumps(request, ensure_ascii=False, allow_nan=False)
            if "errorContains" in case:
                check_failure(command, source, arguments.timeout, name)
            else:
                result = case.get(
                    "expected", apply_changes(expected, case.get("expectedChanges", []))
                )
                check_success(
                    command, source, result, result_validator, arguments.timeout,
                    name, compare,
                )
        for name, source in extra_failures:
            check_failure(command, source, arguments.timeout, name)
    except (AssertionError, OSError, ValueError) as error:
        print(f"FAIL {error}", file=sys.stderr)
        return 1
    display_label = label[0].upper() + label[1:]
    print(f"{display_label} backend passed {len(cases) + len(extra_failures)} conformance cases")
    return 0
