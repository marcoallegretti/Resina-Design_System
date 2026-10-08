import argparse
import json
import math
import sys

from backend_source import duplicate_member_source, nonfinite_member_source
from opaque_request_source import opaque_request_failures
from check_color_guard_backend import check_failure
from check_command_paint_backend import command_mismatch
from check_headless_backend import mismatch, run_backend
from check_schemas import ROOT, apply_changes, check_case, load_json, parse_json, validator_for
from check_spring_trajectory_backend import trajectory_mismatch


def equation_sample(channel, target, time, immediate):
    if immediate:
        position, velocity, settled = target, 0.0, True
    else:
        dynamics = channel["dynamics"]
        omega = math.sqrt(dynamics["stiffness"] / dynamics["mass"])
        zeta = dynamics["damping"] / (2 * math.sqrt(dynamics["mass"] * dynamics["stiffness"]))
        u = omega * time
        y0 = channel["initial"]["position"] - target
        q = channel["initial"]["velocity"] / omega
        if zeta < 1:
            beta = math.sqrt(1 - zeta * zeta)
            cosine, sine = math.cos(beta * u), math.sin(beta * u) / beta
            decay = math.exp(-zeta * u)
            y = decay * (y0 * cosine + (q + zeta * y0) * sine)
            w = decay * (q * cosine - (y0 + zeta * q) * sine)
        elif zeta == 1:
            decay = math.exp(-u)
            y = decay * (y0 + (q + y0) * u)
            w = decay * (q - (q + y0) * u)
        else:
            beta = math.sqrt(zeta * zeta - 1)
            slow, fast = -zeta + beta, -zeta - beta
            a = (q - fast * y0) / (slow - fast)
            b = y0 - a
            y = a * math.exp(slow * u) + b * math.exp(fast * u)
            w = slow * a * math.exp(slow * u) + fast * b * math.exp(fast * u)
        radius = math.hypot(y, w)
        settled = radius <= dynamics["positionThreshold"] and omega * radius <= dynamics["velocityThreshold"]
        position = target if settled else channel["initial"]["position"] if time == 0 else target + y
        velocity = 0.0 if settled else channel["initial"]["velocity"] if time == 0 else omega * w
    return {"schemaVersion": "0.1.0", "representation": "immediate" if immediate else "spring",
            "target": target, "state": {"position": position, "velocity": velocity}, "settled": settled}


def motion_mismatch(actual, request):
    if not isinstance(actual, dict):
        return "/"
    states = request["surface"]["body"]["surface"]["states"]["states"]
    phase = next((s for s in ("disabled", "pressed", "hover") if s in states), "rest")
    theme = parse_json(request["surface"]["body"]["theme"]["themeSource"], "motion theme")
    role = request["surface"]["body"]["surface"]["materialRole"].split(".")[1]
    family = theme["materialAssignments"]["control"][role]
    reduced = request["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]["reducedMotion"]
    policy = "reducedMotion" if reduced else "castImmediate" if family == "cast" else "spring"
    target = {"bodyMix": 0, "depthScale": 1} if phase == "rest" else request["commandAppearance"]["profiles"][family][phase]
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
    return command_mismatch(actual["command"], request, response)


def main():
    parser = argparse.ArgumentParser(description="Check headless animated command paint and retained spring state.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command or not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("provide a backend command and a positive finite timeout")
    base = load_json(ROOT / "conformance/ir/command-motion-request.json")
    cases = load_json(ROOT / "conformance/ir/command-motion-cases.json")
    try:
        for case in cases:
            request = apply_changes(base, case["requestChanges"])
            check_case(validator_for("schemas/command-motion-case.schema.json"), case["name"], case, True)
            check_case(validator_for("schemas/command-motion-request.schema.json"), case["name"], request, case["requestSchemaValid"])
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
                    check_case(validator_for("schemas/command-motion-ir.schema.json"), case["name"], actual, True)
                    difference = motion_mismatch(actual, request)
                    if difference:
                        raise AssertionError(f"{case['name']}: output differs at {difference}")
                    if actual["policy"] != case["expectedPolicy"]:
                        raise AssertionError(f"{case['name']}: policy differs from case")
                    outputs.append(actual)
                if outputs[0] != outputs[1]:
                    raise AssertionError(f"{case['name']}: nondeterministic output")
        failures = (
            ("duplicate channel position", duplicate_member_source(base, "/channels/bodyMix/initial/position")),
            ("duplicate root version", duplicate_member_source(base, "/schemaVersion")),
            ("nonfinite time", nonfinite_member_source(base, "/time")),
        ) + opaque_request_failures(base, "/surface/body")
        for name, source in failures:
            check_failure(command, source, args.timeout, name)
    except (AssertionError, OSError, ValueError, KeyError) as error:
        print(f"FAIL command motion: {error}", file=sys.stderr)
        return 1
    print(f"Command motion backend passed {len(cases) + len(failures)} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
