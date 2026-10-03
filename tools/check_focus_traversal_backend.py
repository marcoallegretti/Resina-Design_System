import argparse
import json
import math
import sys

from backend_source import duplicate_member_source
from check_color_guard_backend import check_failure, check_success
from check_headless_backend import run_backend
from check_schemas import ROOT, check_case, load_json, validator_for


def cases():
    return load_json(ROOT / "conformance/interaction/focus-traversal-cases.json")


def focus_traversal_mismatch(actual, expected):
    return None if actual == expected else "/"


def main():
    parser = argparse.ArgumentParser(description="Check portable Resina sequential focus traversal.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        parser.error("provide a backend command and a positive finite timeout")
    case_validator = validator_for("schemas/focus-traversal-case.schema.json")
    request_validator = validator_for("schemas/focus-traversal-request.schema.json")
    result_validator = validator_for("schemas/focus-traversal-result.schema.json")
    public_cases = cases()
    names = set()
    try:
        if not public_cases:
            raise ValueError("focus traversal cases must not be empty")
        for case in public_cases:
            name = case["name"]
            check_case(case_validator, name, case, True)
            if name in names:
                raise ValueError(f"duplicate focus traversal case: {name}")
            names.add(name)
            check_case(request_validator, name, case["request"], case["requestSchemaValid"])
            source = json.dumps(case["request"], ensure_ascii=False, allow_nan=False)
            if "expected" in case:
                check_success(command, source, case["expected"], result_validator, arguments.timeout, name, focus_traversal_mismatch)
            else:
                completed = run_backend(command, source, arguments.timeout)
                if completed.returncode != 1 or completed.stdout or case["errorContains"] not in completed.stderr:
                    raise AssertionError(f"{name}: expected diagnostic failure: {completed.returncode}, {completed.stderr[:200]!r}")
        base = public_cases[0]["request"]
        failures = (
            ("duplicate root current", duplicate_member_source(base, "/current")),
            ("duplicate target eligibility", duplicate_member_source(base, "/targets/0/eligible")),
            ("unknown root", json.dumps({**base, "unexpected": True})),
        )
        for name, source in failures:
            check_failure(command, source, arguments.timeout, name)
    except (AssertionError, OSError, ValueError) as error:
        print(f"FAIL focus traversal: {error}", file=sys.stderr)
        return 1
    print(f"Focus traversal backend passed {len(public_cases) + len(failures)} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
