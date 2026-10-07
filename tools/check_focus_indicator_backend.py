import argparse
import copy
import json
import math
import sys

from check_color_guard_backend import check_failure
from check_headless_backend import mismatch, run_backend
from check_schemas import ROOT, apply_changes, load_json, parse_json, validator_for


def baseline():
    binding = load_json(ROOT / "conformance/surfaces/binding-vectors.json")[0]
    request = {
        "schemaVersion": "0.1.0",
        "scenario": {
            "schemaVersion": "0.4.0",
            "resolution": load_json(ROOT / "conformance/headless/valid-request.json"),
            "surface": copy.deepcopy(binding["document"]),
        },
        "surroundingColor": {
            "colorSpace": "srgb",
            "components": [1, 1, 1],
            "alpha": 1,
        },
    }
    return request, binding["expected"]


def luminance(channels):
    linear = [
        channel / 12.92 if channel <= 0.04045 else ((channel + 0.055) / 1.055) ** 2.4
        for channel in channels
    ]
    return sum(channel * weight for channel, weight in zip(linear, (0.2126, 0.7152, 0.0722)))


def contrast(first, second):
    lighter, darker = sorted((luminance(first), luminance(second)), reverse=True)
    return (lighter + 0.05) / (darker + 0.05)


def check_success(actual, request, expected, expected_binding):
    binding = copy.deepcopy(expected_binding)
    binding["states"]["states"] = expected.get("states", binding["states"]["states"])
    if mismatch(actual["binding"], binding):
        raise AssertionError("binding differs from the surface conformance result")
    for field, value in (
        ("colorRole", expected["colorRole"]),
        ("fallbackApplied", expected["fallbackApplied"]),
        ("strokeWidth", 2),
        ("gap", 2),
    ):
        if actual[field] != value:
            raise AssertionError(f"{field}: expected {value!r}, got {actual[field]!r}")
    roles = request["scenario"]["resolution"]["opaqueColorAssignments"]["roles"]
    path = roles[actual["colorRole"]]
    palette = request["scenario"]["resolution"]["tokens"]["palette"]
    token = palette[path.split(".", 1)[1]]["$value"]
    color = {"colorSpace": "srgb", "components": token["components"], "alpha": 1}
    if mismatch(actual["color"], color):
        raise AssertionError("indicator color differs from its authored opaque assignment")
    ratio = contrast(color["components"], request["surroundingColor"]["components"])
    if abs(actual["contrastRatio"] - ratio) > 1e-12 or ratio < 3:
        raise AssertionError("indicator contrast ratio differs from the independent calculation")


def main():
    parser = argparse.ArgumentParser(description="Check a Resina focus indicator backend.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or arguments.timeout <= 0 or not math.isfinite(arguments.timeout):
        parser.error("provide a backend command and a positive finite timeout")

    base, binding = baseline()
    cases = load_json(ROOT / "conformance/states/focus-indicator-cases.json")
    case_validator = validator_for("schemas/focus-indicator-case.schema.json")
    request_validator = validator_for("schemas/focus-indicator-request.schema.json")
    result_validator = validator_for("schemas/focus-indicator-result.schema.json")
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
            if "failure" in case:
                check_failure(command, source, arguments.timeout, name)
                continue
            completed = run_backend(command, source, arguments.timeout)
            if completed.returncode != 0 or completed.stderr:
                raise AssertionError(
                    f"{name}: expected clean success; got {completed.returncode}, {completed.stderr[:200]!r}"
                )
            actual = parse_json(completed.stdout, name)
            errors = list(result_validator.iter_errors(actual))
            if errors:
                raise AssertionError(f"{name}: result violates schema: {errors[0].message}")
            check_success(actual, request, case["expected"], binding)
            repeated = run_backend(command, source, arguments.timeout)
            if (
                repeated.returncode != 0
                or repeated.stderr
                or mismatch(parse_json(repeated.stdout, name), actual)
            ):
                raise AssertionError(f"{name}: repeated request changed its result")
        base_source = json.dumps(base)
        member = '"surroundingColor":'
        if base_source.count(member) != 1:
            raise ValueError("baseline surrounding color is not unique")
        duplicate_member = ' {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1}, '
        duplicate = base_source.replace(member, member + duplicate_member + member, 1)
        check_failure(command, duplicate, arguments.timeout, "duplicate request member")
    except (AssertionError, OSError, ValueError) as error:
        print(f"FAIL {error}", file=sys.stderr)
        return 1
    print(f"Focus indicator backend passed {len(cases) + 1} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
