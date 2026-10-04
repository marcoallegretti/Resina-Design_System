import argparse
import copy
import json
import math
import sys

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_failure
from check_headless_backend import mismatch, run_backend
from check_schemas import ROOT, apply_changes, check_case, load_json, parse_json, validator_for
from check_command_motion_backend import equation_sample
from check_spring_trajectory_backend import trajectory_mismatch
from check_toggle_part_paint_backend import toggle_part_mismatch


def toggle_motion_mismatch(actual, request):
    if not isinstance(actual, dict):
        return "/"
    states = request["surface"]["body"]["surface"]["states"]["states"]
    phase = next((s for s in ("disabled", "pressed", "hover") if s in states), "rest")
    theme = parse_json(request["surface"]["body"]["theme"]["themeSource"], "motion theme")
    role = request["surface"]["body"]["surface"]["materialRole"].split(".")[1]
    family = theme["materialAssignments"]["control"][role]
    reduced = request["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]["reducedMotion"]
    policy = "reducedMotion" if reduced else "castImmediate" if family == "cast" else "spring"
    target = {"bodyMix": 0, "depthScale": 1} if phase == "rest" else request["interactionAppearance"]["profiles"][family][phase]
    if actual.get("target") != target:
        return "/target"
    for field, expected in (("schemaVersion", "0.1.0"), ("policy", policy), ("target", target)):
        if mismatch(actual.get(field), expected):
            return "/" + field
    response = {}
    for name, lower, upper in (("bodyMix", -1, 1), ("depthScale", 0, 1)):
        expected = equation_sample(request["channels"][name], target[name], request["time"], policy != "spring")
        difference = trajectory_mismatch(actual.get(name), expected)
        if difference:
            return "/" + name + difference
        value = actual[name]["state"]["position"]
        projection = "lowerBound" if value < lower else "upperBound" if value > upper else "none"
        if actual.get(name + "Projection") != projection:
            return "/" + name + "Projection"
        response[name] = min(upper, max(lower, value))
    return toggle_part_mismatch(actual["partPaint"], request, response)


def main():
    parser = argparse.ArgumentParser(description="Check headless sampled Toggle part paint and retained spring state.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command or not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("provide a backend command and a positive finite timeout")
    base = load_json(ROOT / "conformance/ir/toggle-part-motion-request.json")
    cases = load_json(ROOT / "conformance/ir/toggle-part-motion-cases.json")
    try:
        for case in cases:
            request = apply_changes(base, case["requestChanges"])
            check_case(validator_for("schemas/toggle-part-motion-case.schema.json"), case["name"], case, True)
            check_case(validator_for("schemas/toggle-part-motion-request.schema.json"), case["name"], request, case["requestSchemaValid"])
            source = json.dumps(request, allow_nan=False)
            if "errorContains" in case:
                check_failure(command, source, args.timeout, case["name"])
            else:
                outputs = []
                for _ in range(2):
                    result = run_backend(command, source, args.timeout)
                    if result.returncode != 0 or result.stderr:
                        raise AssertionError(f"{case['name']}: {result.returncode}: {result.stderr[:200]}")
                    actual = parse_json(result.stdout, case["name"])
                    check_case(validator_for("schemas/toggle-part-motion-ir.schema.json"), case["name"], actual, True)
                    difference = toggle_motion_mismatch(actual, request)
                    if difference:
                        raise AssertionError(f"{case['name']}: output differs at {difference}")
                    if actual["policy"] != case["expectedPolicy"]:
                        raise AssertionError(f"{case['name']}: policy differs from case")
                    outputs.append(actual)
                if outputs[0] != outputs[1]:
                    raise AssertionError(f"{case['name']}: nondeterministic output")
        for family in ("cast", "frost", "elastomer"):
            for part in ("track", "thumb"):
                for checked in (False, True):
                    for reduced in (False, True):
                        request = copy.deepcopy(base)
                        theme = parse_json(request["surface"]["body"]["theme"]["themeSource"], family)
                        theme["materialAssignments"]["control"]["interactive"] = family
                        request["surface"]["body"]["theme"]["themeSource"] = json.dumps(theme)
                        request["part"] = part
                        request["surface"]["body"]["surface"]["states"]["states"] = ["pressed", "focused"] + (["checked"] if checked else [])
                        request["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]["reducedMotion"] = reduced
                        result = run_backend(command, json.dumps(request), args.timeout)
                        if result.returncode or result.stderr:
                            raise AssertionError(f"{family} {part}: {result.stderr[:200]}")
                        actual = parse_json(result.stdout, part)
                        check_case(validator_for("schemas/toggle-part-motion-ir.schema.json"), part, actual, True)
                        difference = toggle_motion_mismatch(actual, request)
                        if difference:
                            raise AssertionError(f"{family} {part}: output differs at {difference}")
        for name, source in (
            ("duplicate channel position", duplicate_member_source(base, "/channels/bodyMix/initial/position")),
            ("duplicate root version", duplicate_member_source(base, "/schemaVersion")),
            ("nonfinite time", nonfinite_member_source(base, "/time")),
        ):
            check_failure(command, source, args.timeout, name)
    except (AssertionError, OSError, ValueError, KeyError) as error:
        print(f"FAIL toggle part motion: {error}", file=sys.stderr)
        return 1
    print(f"Toggle part motion backend passed {len(cases) + 24 + 3} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
