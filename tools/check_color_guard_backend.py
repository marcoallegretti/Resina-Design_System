import argparse
import json
import math
import sys

from backend_source import duplicate_member_source
from check_headless_backend import mismatch, run_backend
from check_schemas import ROOT, load_json, parse_json, validator_for


def check_failure(command, source, timeout, name):
    completed = run_backend(command, source, timeout)
    if completed.returncode != 1 or completed.stdout or not completed.stderr.strip():
        raise AssertionError(
            f"{name}: failure must exit 1 with empty stdout and a diagnostic; "
            f"got exit {completed.returncode}, stdout {completed.stdout[:200]!r}, "
            f"stderr {completed.stderr[:200]!r}"
        )


def check_success(command, source, expected, result_validator, timeout, name, compare=mismatch):
    previous = None
    for attempt in range(2):
        completed = run_backend(command, source, timeout)
        if completed.returncode != 0 or completed.stderr:
            raise AssertionError(
                f"{name}: attempt {attempt + 1} failed with exit {completed.returncode}, "
                f"stderr {completed.stderr[:200]!r}"
            )
        actual = parse_json(completed.stdout, f"{name} backend output")
        errors = list(result_validator.iter_errors(actual))
        if errors:
            raise AssertionError(f"{name}: output violates result schema: {errors[0].message}")
        differing = compare(actual, expected)
        if differing:
            raise AssertionError(f"{name}: output differs at {differing}")
        if attempt and actual != previous:
            raise AssertionError(f"{name}: repeated request produced different results")
        previous = actual


def check_backend(
    label, case_schema, request_schema, result_schema, vectors,
    extra_failures=(), compare=mismatch, extra_successes=(), compare_request=None,
):
    parser = argparse.ArgumentParser(
        description=f"Check a Resina {label} backend through the public command protocol."
    )
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
    cases = load_json(ROOT / vectors)
    names = set()
    try:
        for case in cases:
            name = case.get("name")
            errors = list(case_validator.iter_errors(case))
            if errors:
                raise ValueError(f"{name}: invalid case: {errors[0].message}")
            if name in names:
                raise ValueError(f"duplicate case name: {name}")
            names.add(name)
            request_errors = list(request_validator.iter_errors(case["request"]))
            if bool(request_errors) == case["requestSchemaValid"]:
                detail = request_errors[0].message if request_errors else "unexpectedly valid"
                raise ValueError(f"{name}: request schema: {detail}")
            source = json.dumps(case["request"], ensure_ascii=False, allow_nan=False)
            if "expected" in case:
                comparison = compare if compare_request is None else (
                    lambda actual, expected: compare_request(actual, expected, case["request"]))
                check_success(
                    command, source, case["expected"], result_validator,
                    arguments.timeout, name, comparison,
                )
            else:
                check_failure(command, source, arguments.timeout, name)
        baseline = next((case["request"] for case in cases if "expected" in case), None)
        if baseline is None:
            raise ValueError(f"{label} conformance requires a successful request")
        check_failure(
            command,
            duplicate_member_source(baseline, "/schemaVersion"),
            arguments.timeout,
            "duplicate request member",
        )
        for name, source in extra_failures:
            check_failure(command, source, arguments.timeout, name)
        for name, source, expected in extra_successes:
            check_success(command, source, expected, result_validator,
                          arguments.timeout, name, compare)
    except (AssertionError, OSError, ValueError) as error:
        print(f"FAIL {error}", file=sys.stderr)
        return 1
    print(f"{label.capitalize()} backend passed {len(cases) + 1 + len(extra_failures) + len(extra_successes)} conformance cases")
    return 0
