import argparse
import copy
import json
import math
import sys

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_failure
from check_headless_backend import mismatch, run_backend
from check_schemas import ROOT, apply_changes, check_case, load_json, parse_json, validator_for
from check_surface_paint_backend import surface_paint_channel_mismatch


def command_mismatch(actual, request, response=None):
    states = request["surface"]["body"]["surface"]["states"]["states"]
    ordered = [state for state in ("rest", "hover", "focused", "pressed", "disabled") if state in states]
    phase = next((state for state in ("disabled", "pressed", "hover") if state in states), "rest")
    body = actual["paint"]["body"]
    family = body["materialFamily"]
    theme = parse_json(request["surface"]["body"]["theme"]["themeSource"], "command fixture theme")
    role = request["surface"]["body"]["surface"]["materialRole"].split(".")[1]
    if family != theme["materialAssignments"]["control"][role]:
        return "/paint/body/materialFamily"
    if response is None:
        response = {"bodyMix": 0, "depthScale": 1} if phase == "rest" else request["commandAppearance"]["profiles"][family][phase]
    if actual["phase"] != phase:
        return "/phase"
    if mismatch(actual["response"], response):
        return "/response"
    if body["states"]["states"] != ordered:
        return "/paint/body/states"
    for field in ("materialRole", "colorRole", "form"):
        if mismatch(body[field], request["surface"]["body"]["surface"][field]):
            return "/paint/body/" + field
    if body["foreground"] != {"colorSpace": "srgb", "components": [0, 0, 0], "alpha": 1}:
        return "/paint/body/foreground"
    # Public arithmetic fixture: an opaque (0.8, 0.7, 0.6) body with black foreground.
    mix = response["bodyMix"]
    color = [c * (1 + mix) if mix < 0 else c + (1 - c) * mix for c in (0.8, 0.7, 0.6)]
    if mismatch(body["pigment"]["body"], {"colorSpace": "srgb", "components": color, "alpha": 1}):
        return "/paint/body/pigment/body"
    linear = [c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4 for c in color]
    ratio = (sum(c * w for c, w in zip(linear, (0.2126, 0.7152, 0.0722))) + 0.05) / 0.05
    if not math.isclose(body["contentContrastRatio"], ratio, rel_tol=1e-12, abs_tol=1e-12):
        return "/paint/body/contentContrastRatio"
    offset = body["lighting"]["sideOffset"]
    depth = 2 * response["depthScale"]
    if not math.isclose(math.hypot(offset["x"], offset["y"]), depth, rel_tol=1e-12, abs_tol=1e-12):
        return "/paint/body/lighting/sideOffset"
    if ("focus" in actual["paint"]) != ("focused" in states):
        return "/paint/focus"
    return surface_paint_channel_mismatch(actual["paint"])


def check_success(command, request, timeout, name):
    outputs = []
    for _ in range(2):
        completed = run_backend(command, json.dumps(request, allow_nan=False), timeout)
        if completed.returncode != 0 or completed.stderr:
            raise AssertionError(f"{name}: {completed.returncode}: {completed.stderr[:200]}")
        actual = parse_json(completed.stdout, name)
        check_case(validator_for("schemas/command-paint-ir.schema.json"), name, actual, True)
        difference = command_mismatch(actual, request)
        if difference:
            raise AssertionError(f"{name}: output differs at {difference}")
        outputs.append(actual)
    if outputs[0] != outputs[1]:
        raise AssertionError(f"{name}: nondeterministic output")


def main():
    parser = argparse.ArgumentParser(description="Check opaque command paint through its public protocol.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        parser.error("provide a backend command and a positive finite timeout")
    base = load_json(ROOT / "conformance/ir/command-paint-request.json")
    cases = load_json(ROOT / "conformance/ir/command-paint-cases.json")
    try:
        names = set()
        for case in cases:
            name = case["name"]
            if name in names:
                raise ValueError(f"duplicate command paint case: {name}")
            names.add(name)
            check_case(validator_for("schemas/command-paint-case.schema.json"), name, case, True)
            request = apply_changes(base, case["requestChanges"])
            check_case(validator_for("schemas/command-paint-request.schema.json"), name, request, case["requestSchemaValid"])
            if "failure" in case:
                check_failure(command, json.dumps(request), arguments.timeout, name)
            else:
                check_success(command, request, arguments.timeout, name)
        states = ("rest", "hover", "focused", "pressed", "disabled")
        for family in ("cast", "frost", "elastomer"):
            for bits in range(1, 32):
                request = copy.deepcopy(base)
                theme = parse_json(request["surface"]["body"]["theme"]["themeSource"], family)
                theme["materialAssignments"]["control"]["interactive"] = family
                request["surface"]["body"]["theme"]["themeSource"] = json.dumps(theme)
                request["surface"]["body"]["surface"]["states"]["states"] = [s for i, s in enumerate(states) if bits & (1 << i)]
                if family == "frost":
                    request["surface"]["body"]["postTreatmentBackdrop"] = {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1}
                check_success(command, request, arguments.timeout, f"{family} states {bits}")
        failures = (
            ("duplicate response", duplicate_member_source(base, "/commandAppearance/profiles/cast/hover/bodyMix")),
            ("nonfinite response", nonfinite_member_source(base, "/commandAppearance/profiles/frost/pressed/depthScale")),
            ("unknown root", json.dumps({**base, "unexpected": True})),
        )
        for name, source in failures:
            check_failure(command, source, arguments.timeout, name)
    except (AssertionError, KeyError, TypeError, OSError, ValueError) as error:
        print(f"FAIL command paint: {error}", file=sys.stderr)
        return 1
    print(f"Command paint backend passed {len(cases) + 93 + len(failures)} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
