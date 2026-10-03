import argparse
import json
import math
import sys

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_failure, check_success
from check_headless_backend import run_backend
from check_schemas import ROOT, apply_changes, check_case, load_json, validator_for


def baseline():
    return load_json(ROOT / "conformance/interaction/hit-region-request.json")


def cases():
    base = baseline()
    return [
        {**case, "request": apply_changes(base, case["requestChanges"])}
        for case in load_json(ROOT / "conformance/interaction/hit-region-cases.json")
    ]


def hit_region_mismatch(actual, expected):
    return None if actual == expected else "/"


def main():
    parser = argparse.ArgumentParser(description="Check portable Resina hit-region placement.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        parser.error("provide a backend command and a positive finite timeout")
    base = baseline()
    missing = dict(base)
    del missing["occupiedRegions"]
    failures = (
        ("duplicate nested bounds", duplicate_member_source(base, "/visualBounds/x")),
        ("nonfinite extent", nonfinite_member_source(base, "/visualBounds/width")),
        ("missing neighbors", json.dumps(missing)),
        ("unknown root", json.dumps({**base, "unexpected": True})),
    )
    case_validator = validator_for("schemas/hit-region-case.schema.json")
    request_validator = validator_for("schemas/hit-region-request.schema.json")
    result_validator = validator_for("schemas/hit-region-ir.schema.json")
    public_cases = cases()
    names = set()
    try:
        for case in public_cases:
            name = case["name"]
            check_case(case_validator, name, {key: value for key, value in case.items() if key != "request"}, True)
            if name in names:
                raise ValueError(f"duplicate hit region case: {name}")
            names.add(name)
            check_case(request_validator, name, case["request"], case["requestSchemaValid"])
            source = json.dumps(case["request"], ensure_ascii=False, allow_nan=False)
            if "expected" in case:
                check_success(command, source, case["expected"], result_validator, arguments.timeout, name, hit_region_mismatch)
            else:
                completed = run_backend(command, source, arguments.timeout)
                if completed.returncode != 1 or completed.stdout or case["errorContains"] not in completed.stderr:
                    raise AssertionError(f"{name}: expected diagnostic failure: {completed.returncode}, {completed.stderr[:200]!r}")
        check_failure(command, duplicate_member_source(base, "/schemaVersion"), arguments.timeout, "duplicate root version")
        for name, source in failures:
            check_failure(command, source, arguments.timeout, name)
    except (AssertionError, OSError, ValueError) as error:
        print(f"FAIL hit region: {error}", file=sys.stderr)
        return 1
    print(f"Hit region backend passed {len(public_cases) + len(failures) + 1} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
