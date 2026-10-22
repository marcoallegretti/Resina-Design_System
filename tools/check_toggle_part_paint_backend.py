import argparse
import copy
import json
import math
import sys

from backend_source import duplicate_member_source, negative_underflow_member_source, nonfinite_member_source
from opaque_request_source import opaque_request_failures
from surface_paint_source import positional_paint_request
from control_paint_source import positional_control_request
from check_color_guard_backend import check_failure
from check_headless_backend import mismatch, run_backend
from check_schemas import ROOT, apply_changes, check_case, load_json, parse_json, validator_for
from check_surface_paint_backend import surface_paint_channel_mismatch
from check_command_paint_backend import (
    admitting, contrast_ratio, fixture_color, fixture_depth, imperceptible_phase, lit_face_mismatch, mix_body,
    phase_of, without_light,
)

RELIEF_PARTS = {"track": "toggleTrack", "thumb": "toggleThumb"}


def selected_role(request):
    states = request["surface"]["body"]["surface"]["states"]["states"]
    return request["checkedColorRole"] if "checked" in states else request["surface"]["body"]["surface"]["colorRole"]


def toggle_part_mismatch(actual, request, sampled_response=None):
    states = request["surface"]["body"]["surface"]["states"]["states"]
    ordered = [state for state in ("rest", "hover", "focused", "pressed", "checked", "disabled") if state in states]
    phase = phase_of(states)
    checked = "checked" in states
    if actual["schemaVersion"] != "0.2.0" or actual["paint"]["schemaVersion"] != "0.1.0":
        return "/schemaVersion"
    if actual["part"] != request["part"] or actual["checked"] != checked:
        return "/part-or-checked"
    body = actual["paint"]["body"]
    family = body["materialFamily"]
    theme = parse_json(request["surface"]["body"]["theme"]["themeSource"], "toggle fixture theme")
    role = request["surface"]["body"]["surface"]["materialRole"].split(".")[1]
    if family != theme["materialAssignments"]["control"][role]:
        return "/paint/body/materialFamily"
    response = {"bodyMix": 0, "depthScale": 1} if phase == "rest" else request["interactionAppearance"]["profiles"][family][phase]
    if sampled_response is not None:
        response = sampled_response
    if actual["phase"] != phase:
        return "/phase"
    if mismatch(actual["response"], response):
        return "/response"
    if body["states"]["states"] != ordered:
        return "/paint/body/states"
    expected_role = selected_role(request)
    if body["colorRole"] != expected_role:
        return "/paint/body/colorRole"
    for field in ("materialRole", "form"):
        if mismatch(body[field], request["surface"]["body"]["surface"][field]):
            return "/paint/body/" + field
    face = body["face"]
    mix = response["bodyMix"]
    role_colors = [fixture_color(request, role) for role in
                   (request["surface"]["body"]["surface"]["colorRole"], request["checkedColorRole"])]
    rest_color = role_colors[1] if checked else role_colors[0]
    color = mix_body(rest_color, mix)
    if mismatch(face["pigment"]["body"], {"colorSpace": "srgb", "components": color, "alpha": 1}):
        return "/paint/body/face/pigment/body"
    guarded = fixture_color(request, request["surface"]["body"]["foregroundRole"])
    if face["foreground"] != {"colorSpace": "srgb", "components": guarded, "alpha": 1}:
        return "/paint/body/face/foreground"
    if not math.isclose(face["contentContrastRatio"], contrast_ratio(guarded, color), rel_tol=1e-12, abs_tol=1e-12):
        return "/paint/body/face/contentContrastRatio"
    offset = body["lighting"]["sideOffset"]
    semantic_depth = fixture_depth(request)
    depth = semantic_depth * response["depthScale"]
    if not math.isclose(math.hypot(offset["x"], offset["y"]), depth, rel_tol=1e-12, abs_tol=1e-12):
        return "/paint/body/lighting/sideOffset"
    profile = request["interactionAppearance"]["profiles"][family]
    if sampled_response is None and any(imperceptible_phase(role_color, semantic_depth, profile)
                                        for role_color in role_colors):
        return "/interactionAppearance (imperceptible authored response)"
    if ("focus" in actual["paint"]) != (request["part"] == "track" and "focused" in states):
        return "/paint/focus"
    difference = lit_face_mismatch(body, request, response, phase, RELIEF_PARTS[request["part"]], expected_role)
    if difference:
        return difference
    return surface_paint_channel_mismatch(actual["paint"])


def check_success(command, request, timeout, name, source=None):
    """Resolves one request, or its equivalent source text, twice and checks it independently."""
    outputs = []
    for _ in range(2):
        completed = run_backend(command, source or json.dumps(request, allow_nan=False), timeout)
        if completed.returncode != 0 or completed.stderr:
            raise AssertionError(f"{name}: {completed.returncode}: {completed.stderr[:200]}")
        actual = parse_json(completed.stdout, name)
        check_case(validator_for("schemas/toggle-part-paint-ir.schema.json"), name, actual, True)
        difference = toggle_part_mismatch(actual, request)
        if difference:
            raise AssertionError(f"{name}: output differs at {difference}")
        outputs.append(actual)
    if outputs[0] != outputs[1]:
        raise AssertionError(f"{name}: nondeterministic output")
    return outputs[0]


def check_light_differential(command, request, timeout, name):
    """In an admitting environment, light channels change nothing but face.light."""
    lit = check_success(command, admitting(copy.deepcopy(request)), timeout, name + " admitted")
    unlit_request = admitting(copy.deepcopy(request))
    unlit_request["light"] = {"ambient": None, "interaction": None}
    unlit = check_success(command, unlit_request, timeout, name + " admitted without light")
    if without_light(lit) != without_light(unlit):
        raise AssertionError(f"{name}: light changed more than face.light")


def part_requests(base):
    for family in ("cast", "frost", "elastomer"):
        for part in ("track", "thumb"):
            for states in (["rest"], ["rest", "checked"], ["hover", "focused"], ["focused", "pressed", "checked"],
                           ["hover", "focused", "checked", "disabled"]):
                request = copy.deepcopy(base)
                request["part"] = part
                theme = parse_json(request["surface"]["body"]["theme"]["themeSource"], family)
                theme["materialAssignments"]["control"]["interactive"] = family
                request["surface"]["body"]["theme"]["themeSource"] = json.dumps(theme)
                request["surface"]["body"]["surface"]["states"]["states"] = states
                if family == "frost":
                    request["surface"]["body"]["postTreatmentBackdrop"] = {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1}
                yield f"{family} {part} {states}", request


def coefficient_sources(command, base, timeout):
    """Negative-zero channel coefficient sources fail even where policy or phase suppresses them."""
    checked = 0
    environment = "/surface/body/theme/environment"
    for label, changes in (
        ("rest", [{"path": "/surface/body/surface/states/states", "value": ["rest"]}]),
        ("disabled", [{"path": "/surface/body/surface/states/states", "value": ["disabled"]}]),
        ("high contrast", [{"path": environment + "/accessibilityPreferences/highContrast", "value": True},
                           {"path": "/surface/body/surface/states/states", "value": ["hover"]}]),
    ):
        request = apply_changes(admitting(copy.deepcopy(base)), changes)
        for pointer in ("/light/ambient", "/light/interaction/strength"):
            source = negative_underflow_member_source(request, pointer)
            check_failure(command, source, timeout, f"{label} {pointer} negative zero source")
            zero = apply_changes(request, [{"path": pointer, "value": 0}])
            for literal in ("-0", "-0e-400", "1e-400"):
                check_success(command, zero, timeout, f"{label} {pointer} {literal}",
                              source=source.replace("-1e-400", literal, 1))
            checked += 4
    return checked


def main():
    parser = argparse.ArgumentParser(description="Check lit toggle part paint through its public protocol.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        parser.error("provide a backend command and a positive finite timeout")
    base = load_json(ROOT / "conformance/ir/toggle-part-paint-request.json")
    cases = load_json(ROOT / "conformance/ir/toggle-part-paint-cases.json")
    checked = 0
    try:
        names = set()
        for case in cases:
            name = case["name"]
            if name in names:
                raise ValueError(f"duplicate toggle part paint case: {name}")
            names.add(name)
            check_case(validator_for("schemas/toggle-part-paint-case.schema.json"), name, case, True)
            request = apply_changes(base, case["requestChanges"])
            check_case(validator_for("schemas/toggle-part-paint-request.schema.json"), name, request, case["requestSchemaValid"])
            if "failure" in case:
                check_failure(command, json.dumps(request), arguments.timeout, name)
            else:
                states = request["surface"]["body"]["surface"]["states"]["states"]
                if phase_of(states) != case["expectedPhase"]:
                    raise AssertionError(f"{name}: expectedPhase contradicts request states")
                check_success(command, request, arguments.timeout, name)
            checked += 1
        for name, request in part_requests(base):
            check_success(command, request, arguments.timeout, name)
            check_light_differential(command, request, arguments.timeout, name)
            checked += 3
        checked += coefficient_sources(command, base, arguments.timeout)
        failures = (
            ("duplicate response", duplicate_member_source(base, "/interactionAppearance/profiles/cast/hover/bodyMix")),
            ("nonfinite response", nonfinite_member_source(base, "/interactionAppearance/profiles/frost/pressed/depthScale")),
            ("duplicate light channel", duplicate_member_source(base, "/light/ambient")),
            ("nonfinite light channel", nonfinite_member_source(base, "/light/interaction/strength")),
            ("unknown root", json.dumps({**base, "unexpected": True})),
        ) + opaque_request_failures(base, "/surface/body") + (
            ("positional surface paint request", positional_paint_request(base, "/surface")),
            ("positional toggle-part-paint request", positional_control_request(
                base, ("schemaVersion", "part", "surface", "checkedColorRole", "interactionAppearance", "light", "relief"))),
            ("positional relief", json.dumps(dict(base, relief=list(base["relief"].values())))),
        )
        for name, source in failures:
            check_failure(command, source, arguments.timeout, name)
        checked += len(failures)
    except (AssertionError, KeyError, TypeError, OSError, ValueError) as error:
        print(f"FAIL toggle part paint: {error}", file=sys.stderr)
        return 1
    print(f"Toggle part paint backend passed {checked} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
