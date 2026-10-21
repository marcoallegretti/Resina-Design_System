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
from lit_body_arithmetic import Body, body_invariants
from material_light_arithmetic import expected_result

MINIMUM_STATE_COLOR_DIFFERENCE = 0.04
MINIMUM_STATE_DEPTH_CHANGE = 1
DEPTH_TOLERANCE = 1e-12
LOGICAL_CORNERS = ("topStart", "topEnd", "bottomEnd", "bottomStart")


def oklab(components):
    # Ottosson's linear sRGB to LMS and LMS' to Oklab matrices, independent of the reference's XYZ path.
    linear = [c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4 for c in components]
    lms = [sum(w * c for w, c in zip(row, linear)) ** (1 / 3) for row in (
        (0.4122214708, 0.5363325363, 0.0514459929),
        (0.2119034982, 0.6806995451, 0.1073969566),
        (0.0883024619, 0.2817188376, 0.6299787005))]
    return [sum(w * c for w, c in zip(row, lms)) for row in (
        (0.2104542553, 0.7936177850, -0.0040720468),
        (1.9779984951, -2.4285922050, 0.4505937099),
        (0.0259040371, 0.7827717662, -0.8086757660))]


def mix_body(components, mix):
    return [c * (1 + mix) if mix < 0 else c + (1 - c) * mix for c in components]


def perceivable(rest_color, color, rest_depth, depth):
    return (math.dist(oklab(rest_color), oklab(color)) >= MINIMUM_STATE_COLOR_DIFFERENCE
            or abs(depth - rest_depth) >= MINIMUM_STATE_DEPTH_CHANGE - DEPTH_TOLERANCE)


DISABLED_LABEL_CONTRAST = 3
HIGH_CONTRAST_DISABLED_LABEL_CONTRAST = 4.5


def disabled_label_bound(environment):
    return HIGH_CONTRAST_DISABLED_LABEL_CONTRAST if environment["accessibilityPreferences"]["highContrast"] \
        else DISABLED_LABEL_CONTRAST


def decode(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def relative_luminance(components):
    return sum(decode(c) * w for c, w in zip(components, (0.2126, 0.7152, 0.0722)))


def contrast_ratio(first, second):
    high, low = sorted((relative_luminance(first), relative_luminance(second)), reverse=True)
    return (high + 0.05) / (low + 0.05)


def disabled_label_mismatch(label, background, undimmed, bound):
    """Checks a published disabled label against the linear-light dimming law independently."""
    ratio = contrast_ratio(label, background)
    if contrast_ratio(undimmed, background) <= bound + 1e-9:
        return None if label == list(undimmed) else "undimmed label changed"
    # Independent rounding may land within the 1e-12 conformance tolerance of the bound.
    if not bound - 1e-12 <= ratio < bound + 1e-9:
        return f"disabled label contrast {ratio} is not the {bound}:1 bound"
    start, end, actual = ([decode(c) for c in color] for color in (undimmed, background, label))
    fractions = [(a - s) / (e - s) for a, s, e in zip(actual, start, end) if abs(e - s) > 1e-6]
    if not fractions or max(fractions) - min(fractions) > 1e-9 or not 0 <= fractions[0] <= 1:
        return "disabled label is not a linear-light mix toward its background"
    return None


def fixture_theme(request):
    return parse_json(request["surface"]["body"]["theme"]["themeSource"], "command fixture theme")


def fixture_token(theme, path):
    group, name = path.split(".")
    return theme["tokens"][group][name]["$value"]


def fixture_color(request, role):
    """Resolves an opaque role in the public arithmetic fixture, whose palette is literal sRGB."""
    theme = fixture_theme(request)
    return fixture_token(theme, theme["opaqueColorAssignments"]["roles"][role])["components"]


def fixture_depth(request):
    """Resolves the semantic elevation depth of the public arithmetic fixture in logical px."""
    body = request["surface"]["body"]
    path = body["appearance"]["depthAssignments"]["roles"][body["surface"]["form"]["elevation"]]
    return fixture_token(fixture_theme(request), path)["value"]


def fixture_radii(request):
    """Resolves the logical Tier 0 shape fallback radii of the public arithmetic fixture."""
    body = request["surface"]["body"]
    profile = body["appearance"]["shapeAssignments"]["profiles"][body["surface"]["form"]["shape"]]
    theme = fixture_theme(request)
    if profile["kind"] == "capsule":
        values = dict.fromkeys(LOGICAL_CORNERS, min(body["size"]["width"], body["size"]["height"]) / 2)
    elif profile["kind"] == "uniform":
        values = dict.fromkeys(LOGICAL_CORNERS, fixture_token(theme, profile["radius"])["value"])
    else:
        values = {name: fixture_token(theme, profile["radii"][name])["value"] for name in LOGICAL_CORNERS}
    return {name: {"x": value, "y": value} for name, value in values.items()}


def phase_of(states):
    return next((state for state in ("disabled", "pressed", "hover") if state in states), "rest")


def command_body_source(request, response):
    """The lit control body source of one fixture command request, derived without the reference."""
    body = request["surface"]["body"]
    theme = fixture_theme(request)
    family = theme["materialAssignments"]["control"][body["surface"]["materialRole"].split(".")[1]]

    def color(components):
        return {"colorSpace": "srgb", "components": list(components), "alpha": 1}

    identity = {
        "materialRole": body["surface"]["materialRole"], "colorRole": body["surface"]["colorRole"],
        "materialFamily": family, "form": body["surface"]["form"],
        "states": body["surface"]["states"], "foregroundRole": body["foregroundRole"],
        "contentFallbackApplied": False,
    }
    if family == "frost":
        identity["frostRepresentation"] = "opaqueDimensional"
    return {
        "identity": identity,
        "size": body["size"],
        "radii": fixture_radii(request),
        "relief": request["relief"]["commandBody"],
        "body": color(mix_body(fixture_color(request, body["surface"]["colorRole"]), response["bodyMix"])),
        "pigmentProfiles": body["appearance"]["pigmentProfiles"],
        "keyLight": body["appearance"]["keyLight"],
        "light": request["light"],
        "environment": body["theme"]["environment"],
        "edge": {
            "schemaVersion": "0.1.0",
            "outline": color(fixture_color(request, "outline")),
            "outlineStrong": color(fixture_color(request, "outline.strong")),
            "adjacentColor": body["adjacentColor"],
            "minimumContrast": body["minimumEdgeContrast"],
        },
        "edgeWidth": body["appearance"]["bands"][family]["edgeWidth"],
        "foreground": color(fixture_color(request, body["foregroundRole"])),
        "minimumContentContrast": body["minimumContentContrast"],
        "depth": fixture_depth(request) * response["depthScale"],
        "points": [],
    }


def expected_light(source, phase):
    light = expected_result({
        "pigment": {"materialFamily": source["identity"]["materialFamily"],
                    "profiles": source["pigmentProfiles"], "body": source["body"]},
        "light": {"keyLight": source["keyLight"], **source["light"]},
        "environment": source["environment"],
        "enabled": True,
        "interactionActive": phase in ("hover", "pressed"),
        "normals": [],
    })
    return {name: light[name] for name in ("keyDirection", "ambient", "interaction")}


def imperceptible_phase(rest_color, rest_depth, profile):
    responses = {"rest": {"bodyMix": 0, "depthScale": 1}, **profile}
    for phase, reference in (("hover", "rest"), ("pressed", "rest"), ("disabled", "rest"),
                             ("disabled", "pressed")):
        first, second = responses[reference], responses[phase]
        if not perceivable(mix_body(rest_color, first["bodyMix"]), mix_body(rest_color, second["bodyMix"]),
                           rest_depth * first["depthScale"], rest_depth * second["depthScale"]):
            return phase if reference == "rest" else phase + " from " + reference
    return None


def lit_face_mismatch(body, request, response, phase):
    """Compares the published lit body with independent spec/88 arithmetic for this request."""
    source = command_body_source(request, response)
    oracle = Body(source)
    face = body["face"]
    if mismatch(face["light"], expected_light(source, phase)):
        return "/paint/body/face/light"
    if mismatch(face["pigment"], oracle.pigment):
        return "/paint/body/face/pigment"
    if face["edge"]["color"] != oracle.edge or face["edgeWidth"] != source["edgeWidth"]:
        return "/paint/body/face/edge"
    if face["surface"]["profile"] != request["relief"]["commandBody"]:
        return "/paint/body/face/surface/profile"
    if face["surface"]["outline"]["size"] != source["size"]:
        return "/paint/body/face/surface/outline/size"
    if mismatch(face["contentBounds"], oracle.content_bounds):
        return "/paint/body/face/contentBounds"
    if mismatch(face["colorRanges"], oracle.color_ranges()):
        return "/paint/body/face/colorRanges"
    if mismatch(body["depth"], source["depth"]):
        return "/paint/body/depth"
    invariant = body_invariants(body)
    if invariant:
        return "/paint/body (" + invariant + ")"
    return None


def command_mismatch(actual, request, response=None):
    states = request["surface"]["body"]["surface"]["states"]["states"]
    ordered = [state for state in ("rest", "hover", "focused", "pressed", "disabled") if state in states]
    phase = phase_of(states)
    body = actual["paint"]["body"]
    family = body["materialFamily"]
    theme = fixture_theme(request)
    role = request["surface"]["body"]["surface"]["materialRole"].split(".")[1]
    if family != theme["materialAssignments"]["control"][role]:
        return "/paint/body/materialFamily"
    authored = response is None
    if authored:
        response = {"bodyMix": 0, "depthScale": 1} if phase == "rest" else request["commandAppearance"]["profiles"][family][phase]
    if actual["schemaVersion"] != "0.2.0" or actual["paint"]["schemaVersion"] != "0.1.0":
        return "/schemaVersion"
    if actual["phase"] != phase:
        return "/phase"
    if mismatch(actual["response"], response):
        return "/response"
    if body["states"]["states"] != ordered:
        return "/paint/body/states"
    for field in ("materialRole", "colorRole", "form"):
        if mismatch(body[field], request["surface"]["body"]["surface"][field]):
            return "/paint/body/" + field
    rest_color = fixture_color(request, request["surface"]["body"]["surface"]["colorRole"])
    guarded = fixture_color(request, request["surface"]["body"]["foregroundRole"])
    color = mix_body(rest_color, response["bodyMix"])
    face = body["face"]
    if mismatch(face["pigment"]["body"], {"colorSpace": "srgb", "components": color, "alpha": 1}):
        return "/paint/body/face/pigment/body"
    foreground = face["foreground"]
    if foreground["colorSpace"] != "srgb" or foreground["alpha"] != 1:
        return "/paint/body/face/foreground"
    if phase == "disabled":
        if disabled_label_mismatch(foreground["components"], color, guarded,
                                   disabled_label_bound(request["surface"]["body"]["theme"]["environment"])):
            return "/paint/body/face/foreground (disabled label)"
    elif foreground["components"] != guarded:
        return "/paint/body/face/foreground"
    ratio = contrast_ratio(foreground["components"], color)
    if not math.isclose(face["contentContrastRatio"], ratio, rel_tol=1e-12, abs_tol=1e-12):
        return "/paint/body/face/contentContrastRatio"
    offset = body["lighting"]["sideOffset"]
    rest_depth = fixture_depth(request)
    depth = rest_depth * response["depthScale"]
    if not math.isclose(math.hypot(offset["x"], offset["y"]), depth, rel_tol=1e-12, abs_tol=1e-12):
        return "/paint/body/lighting/sideOffset"
    if authored and imperceptible_phase(rest_color, rest_depth, request["commandAppearance"]["profiles"][family]):
        return "/commandAppearance (imperceptible authored response)"
    if ("focus" in actual["paint"]) != ("focused" in states):
        return "/paint/focus"
    difference = lit_face_mismatch(body, request, response, phase)
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
        check_case(validator_for("schemas/command-paint-ir.schema.json"), name, actual, True)
        difference = command_mismatch(actual, request)
        if difference:
            raise AssertionError(f"{name}: output differs at {difference}")
        outputs.append(actual)
    if outputs[0] != outputs[1]:
        raise AssertionError(f"{name}: nondeterministic output")
    return outputs[0]


def admitting(request):
    """The fixture environment with every interaction-light condition admitted."""
    environment = request["surface"]["body"]["theme"]["environment"]
    environment["accessibilityPreferences"].update(highContrast=False, reducedMotion=False)
    environment["rendererCapabilities"]["dynamicLighting"] = True
    environment["qualityPolicy"] = "balanced"
    return request


def without_light(result):
    changed = copy.deepcopy(result)
    del changed["paint"]["body"]["face"]["light"]
    return changed


def check_light_differential(command, request, timeout, name):
    """In an admitting environment, light channels change nothing but face.light."""
    lit = check_success(command, admitting(copy.deepcopy(request)), timeout, name + " admitted")
    unlit_request = admitting(copy.deepcopy(request))
    unlit_request["light"] = {"ambient": None, "interaction": None}
    unlit = check_success(command, unlit_request, timeout, name + " admitted without light")
    if without_light(lit) != without_light(unlit):
        raise AssertionError(f"{name}: light changed more than face.light")


def state_requests(base):
    states = ("rest", "hover", "focused", "pressed", "disabled")
    for family in ("cast", "frost", "elastomer"):
        for bits in range(1, 32):
            request = copy.deepcopy(base)
            theme = fixture_theme(request)
            theme["materialAssignments"]["control"]["interactive"] = family
            request["surface"]["body"]["theme"]["themeSource"] = json.dumps(theme)
            request["surface"]["body"]["surface"]["states"]["states"] = [s for i, s in enumerate(states) if bits & (1 << i)]
            if family == "frost":
                request["surface"]["body"]["postTreatmentBackdrop"] = {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1}
            yield f"{family} states {bits}", request


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
    parser = argparse.ArgumentParser(description="Check lit command paint through its public protocol.")
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
    checked = 0
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
            checked += 1
        for name, request in state_requests(base):
            check_success(command, request, arguments.timeout, name)
            check_light_differential(command, request, arguments.timeout, name)
            checked += 3
        checked += coefficient_sources(command, base, arguments.timeout)
        failures = (
            ("duplicate response", duplicate_member_source(base, "/commandAppearance/profiles/cast/hover/bodyMix")),
            ("nonfinite response", nonfinite_member_source(base, "/commandAppearance/profiles/frost/pressed/depthScale")),
            ("duplicate light channel", duplicate_member_source(base, "/light/ambient")),
            ("nonfinite light channel", nonfinite_member_source(base, "/light/interaction/strength")),
            ("unknown root", json.dumps({**base, "unexpected": True})),
        ) + opaque_request_failures(base, "/surface/body") + (
            ("positional surface paint request", positional_paint_request(base, "/surface")),
            ("positional command-paint request", positional_control_request(
                base, ("schemaVersion", "surface", "commandAppearance", "light", "relief"))),
            ("positional relief", json.dumps(dict(base, relief=list(base["relief"].values())))),
        )
        for name, source in failures:
            check_failure(command, source, arguments.timeout, name)
        checked += len(failures)
    except (AssertionError, KeyError, TypeError, OSError, ValueError) as error:
        print(f"FAIL command paint: {error}", file=sys.stderr)
        return 1
    print(f"Command paint backend passed {checked} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
