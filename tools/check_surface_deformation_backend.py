import argparse
import json
import math
import sys

from backend_source import duplicate_member_source, negative_underflow_member_source, nonfinite_member_source
from check_color_guard_backend import check_failure, check_success
from check_headless_backend import mismatch
from check_schemas import ROOT, apply_changes, check_case, load_json, validator_for
from check_spring_trajectory_backend import trajectory_mismatch
import surface_deformation_arithmetic as arithmetic

KINDS = {
    "deformation": ("widthScale", "heightScale"),
    "emergence": ("progress", "opacity"),
}
ROOT_MEMBERS = {
    "deformation": ("schemaVersion", "family", "reducedMotion", "time", "outline", "content", "sweepOffset",
                    "bounds", "channels"),
    "emergence": ("schemaVersion", "family", "reducedMotion", "time", "emerged", "from", "to", "sweepOffset",
                  "channels"),
}
SOURCE_NUMBERS = {
    "deformation": (
        "/time", "/bounds/widthScale/upper", "/channels/heightScale/target", "/sweepOffset/x",
        "/content/width", "/outline/size/height", "/channels/widthScale/dynamics/mass",
        "/channels/heightScale/initial/velocity", "/outline/minimumInset",
    ),
    "emergence": (
        "/time", "/from/origin/x", "/to/outline/size/width", "/to/content/height", "/sweepOffset/y",
        "/channels/opacity/dynamics/damping", "/channels/progress/initial/position",
    ),
}
DUPLICATES = {
    "deformation": (
        "/time", "/outline/size/width", "/outline/radii/topStart/x", "/bounds/heightScale/lower",
        "/channels/widthScale/target", "/channels/heightScale/dynamics/stiffness",
        "/channels/widthScale/initial/position",
    ),
    "emergence": (
        "/to/identity", "/from/outline/size/width", "/to/origin/y", "/to/content/x",
        "/channels/opacity/dynamics/mass", "/channels/progress/initial/velocity",
    ),
}
UNDERFLOWS = {
    "deformation": ("/bounds/widthScale/lower", "/outline/radii/topStart/x", "/outline/minimumInset"),
    "emergence": ("/from/outline/radii/topEnd/y", "/to/outline/minimumInset"),
}


def baseline(kind):
    return load_json(ROOT / f"conformance/motion/surface-{kind}-request.json")


def cases(kind):
    return load_json(ROOT / f"conformance/motion/surface-{kind}-cases.json")


def case_request(base, case):
    return apply_changes(base, case["requestChanges"])


def oracle(kind):
    return getattr(arithmetic, kind)


def expected_result(kind, request):
    return oracle(kind)(request)


def geometry_close(actual, expected):
    return math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12)


def motion_mismatch(kind, request, actual, pointer=""):
    if not isinstance(actual, dict):
        return "/"
    try:
        reference = expected_result(kind, request)
        accepted = {name: actual[name] for name in KINDS[kind]}
        for name in KINDS[kind]:
            difference = trajectory_mismatch(actual[name], reference[name])
            if difference:
                return f"/{name}{difference}"
        expected = oracle(kind)(request, accepted)
    except (KeyError, TypeError):
        return "/"
    except arithmetic.Reject as error:
        return f"rejected by the oracle: {error}"
    return compare(actual, expected, KINDS[kind])


def compare(actual, expected, samples, pointer=""):
    if isinstance(expected, dict):
        if not isinstance(actual, dict) or actual.keys() != expected.keys():
            return pointer or "/"
        for name, value in expected.items():
            if not pointer and name in samples:
                continue
            found = compare(actual[name], value, samples, f"{pointer}/{name}")
            if found:
                return found
        return None
    if expected is None or isinstance(expected, (str, bool)) or actual is None:
        return None if actual == expected and type(actual) is type(expected) else pointer or "/"
    if isinstance(expected, (int, float)) and isinstance(actual, (int, float)) and not isinstance(actual, bool):
        return None if geometry_close(actual, expected) else pointer or "/"
    return pointer or "/"


def source_failures(kind, base):
    failures = [
        ("duplicate root version", duplicate_member_source(base, "/schemaVersion")),
        ("positional request", json.dumps([base[member] for member in ROOT_MEMBERS[kind]])),
        ("unknown root member", json.dumps({**base, "unexpected": True})),
        ("null request", "null"),
        ("empty array request", "[]"),
    ]
    failures += [(f"duplicate {pointer}", duplicate_member_source(base, pointer)) for pointer in DUPLICATES[kind]]
    failures += [(f"nonfinite {pointer}", nonfinite_member_source(base, pointer)) for pointer in SOURCE_NUMBERS[kind]]
    failures += [(f"underflow {pointer}", negative_underflow_member_source(base, pointer)) for pointer in UNDERFLOWS[kind]]
    return failures


def check_kind(kind, command, timeout):
    base = baseline(kind)
    case_validator = validator_for(f"schemas/surface-{kind}-case.schema.json")
    request_validator = validator_for(f"schemas/surface-{kind}-request.schema.json")
    result_validator = validator_for(f"schemas/surface-{kind}-result.schema.json")
    names = set()
    public_cases = cases(kind)
    for case in public_cases:
        name = f"{kind}: {case['name']}"
        check_case(case_validator, name, case, True)
        if name in names:
            raise ValueError(f"duplicate case {name}")
        names.add(name)
        request = case_request(base, case)
        check_case(request_validator, name, request, case["requestSchemaValid"])
        source = json.dumps(request, ensure_ascii=False, allow_nan=False)
        if "errorContains" in case:
            if case["requestSchemaValid"]:
                try:
                    expected_result(kind, request)
                except arithmetic.Reject as error:
                    if case["errorContains"].lower() not in str(error).lower():
                        raise AssertionError(f"{name}: oracle failed for the wrong reason: {error}")
                else:
                    raise AssertionError(f"{name}: oracle accepted a negative case")
            check_failure(command, source, timeout, name)
        else:
            expected = expected_result(kind, request)
            if expected["policy"] != case["expectedPolicy"]:
                raise AssertionError(f"{name}: expectedPolicy disagrees with the oracle")
            check_case(result_validator, name, expected, True)
            check_success(
                command, source, expected, result_validator, timeout, name,
                lambda actual, _expected, request=request: motion_mismatch(kind, request, actual),
            )
    failures = source_failures(kind, base)
    for name, source in failures:
        check_failure(command, source, timeout, f"{kind}: {name}")
    return len(public_cases) + len(failures)


def main():
    parser = argparse.ArgumentParser(description="Check Resina bounded deformation and emergence samples.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("--emergence", required=True, help="the resina-surface-emergence command")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        parser.error("provide a deformation backend command and a positive finite timeout")
    try:
        checked = check_kind("deformation", command, arguments.timeout)
        checked += check_kind("emergence", [arguments.emergence, "-"], arguments.timeout)
    except (AssertionError, KeyError, OSError, ValueError) as error:
        print(f"FAIL surface deformation: {error}", file=sys.stderr)
        return 1
    print(f"Surface deformation and emergence backend passed {checked} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
