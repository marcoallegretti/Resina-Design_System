import argparse
import json
import math
import subprocess
import sys

from check_schemas import ROOT, apply_changes, load_json, parse_json, validator_for


def mismatch(actual, expected, pointer=""):
    if isinstance(expected, dict):
        if not isinstance(actual, dict) or actual.keys() != expected.keys():
            return pointer or "/"
        for name, value in expected.items():
            escaped = name.replace("~", "~0").replace("/", "~1")
            found = mismatch(actual[name], value, f"{pointer}/{escaped}")
            if found:
                return found
        return None
    if isinstance(expected, list):
        if not isinstance(actual, list) or len(actual) != len(expected):
            return pointer or "/"
        for index, value in enumerate(expected):
            found = mismatch(actual[index], value, f"{pointer}/{index}")
            if found:
                return found
        return None
    if isinstance(expected, bool) or isinstance(actual, bool):
        return None if actual is expected else pointer or "/"
    if isinstance(expected, (int, float)) and isinstance(actual, (int, float)):
        return None if math.isclose(actual, expected, rel_tol=0, abs_tol=1e-12) else pointer or "/"
    return None if actual == expected else pointer or "/"


def run_backend(command, source, timeout):
    try:
        return subprocess.run(
            command,
            input=source,
            text=True,
            encoding="utf-8",
            capture_output=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise AssertionError(f"backend exceeded {timeout} seconds") from error


def check_case(command, case, request, expected, base_source, result_validator, timeout):
    if "sourceText" in case:
        source = case["sourceText"]
    elif "sourceReplace" in case:
        edit = case["sourceReplace"]
        if base_source.count(edit["find"]) != 1:
            raise ValueError(f"source replacement must match exactly once: {edit['find']!r}")
        source = base_source.replace(edit["find"], edit["with"], 1)
    else:
        source = json.dumps(apply_changes(request, case.get("requestChanges", [])), ensure_ascii=False)
    completed = run_backend(command, source, timeout)

    if case["outcome"] == "invalid":
        if completed.returncode != 1 or completed.stdout or not completed.stderr.strip():
            raise AssertionError(
                f"invalid source must exit 1 with empty stdout and a diagnostic; "
                f"got exit {completed.returncode}, stdout {completed.stdout[:200]!r}, "
                f"stderr {completed.stderr[:200]!r}"
            )
        return

    if completed.returncode != 0 or completed.stderr:
        raise AssertionError(
            f"valid source must exit 0 without stderr; got exit {completed.returncode}, "
            f"stderr {completed.stderr[:200]!r}"
        )
    try:
        actual = parse_json(completed.stdout, f"backend output for {case['name']}")
    except ValueError as error:
        raise AssertionError(f"backend did not emit one strict JSON document: {error}") from error
    errors = list(result_validator.iter_errors(actual))
    if errors:
        raise AssertionError(f"output violates the result schema: {errors[0].message}")
    wanted = apply_changes(expected, case.get("expectedChanges", []))
    differing = mismatch(actual, wanted)
    if differing:
        raise AssertionError(f"output differs from the conformance result at {differing}")
    repeated = run_backend(command, source, timeout)
    if repeated.returncode != 0 or repeated.stderr:
        raise AssertionError("repeating a valid request changed its exit status or emitted a diagnostic")
    try:
        repeated_output = parse_json(repeated.stdout, f"repeat output for {case['name']}")
    except ValueError as error:
        raise AssertionError(f"repeat output is not one strict JSON document: {error}") from error
    differing = mismatch(repeated_output, actual)
    if differing:
        raise AssertionError(f"repeating the same request changed the result at {differing}")


def main():
    parser = argparse.ArgumentParser(
        description="Check a Resina headless backend using the public stdin/stdout protocol."
    )
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or arguments.timeout <= 0 or not math.isfinite(arguments.timeout):
        parser.error("provide a backend command and a positive finite timeout")

    request = load_json(ROOT / "conformance/headless/valid-request.json")
    base_source = (ROOT / "conformance/headless/valid-request.json").read_text(encoding="utf-8")
    expected = load_json(ROOT / "conformance/headless/expected-resolution.json")
    cases = load_json(ROOT / "conformance/headless/backend-cases.json")
    case_validator = validator_for("schemas/headless-conformance-case.schema.json")
    result_validator = validator_for("schemas/headless-result.schema.json")
    names = set()
    for case in cases:
        errors = list(case_validator.iter_errors(case))
        if errors:
            raise ValueError(f"invalid backend case: {errors[0].message}")
        name = case["name"]
        if name in names:
            raise ValueError(f"duplicate backend case name: {name}")
        names.add(name)
        try:
            check_case(command, case, request, expected, base_source, result_validator, arguments.timeout)
        except (AssertionError, OSError, ValueError) as error:
            print(f"FAIL {name}: {error}", file=sys.stderr)
            return 1
    print(f"Headless backend passed {len(cases)} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
