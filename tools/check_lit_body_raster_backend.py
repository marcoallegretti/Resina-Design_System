"""Verify the public lit Command raster against independent lit control body arithmetic."""
import argparse
import copy
import json
import math
import subprocess

from check_command_paint_backend import admitting, command_body_source, fixture_color, fixture_theme, phase_of
from check_material_relief_raster_backend import decoded_png
from check_schemas import ROOT, apply_changes, load_json
from lit_body_arithmetic import contrast, pixels as lit_pixels

SCALES = (1, 1.25, 1.5, 2, 3)
SAMPLES = (1, 3)
MARGIN = 3
FOCUS_GAP = 2
FOCUS_STROKE = 2
FOCUS_CONTRAST = 3
ENVIRONMENT = "/surface/body/theme/environment"
STATES = "/surface/body/surface/states/states"


def rounded(width, height, radius):
    """Changes that bind the rounded shape to a literal radius on a body of the given size."""
    return [
        {"path": "/surface/body/size", "value": {"width": width, "height": height}},
        {"path": "/surface/body/surface/form/shape", "value": "rounded"},
        {"path": "/surface/body/appearance/shapeAssignments/profiles/rounded",
         "value": {"kind": "uniform", "radius": "radius.5"}},
        {"path": "/radius", "value": radius},
    ]


SCENES = (
    ("elastomer hover with ambient and interaction light", "elastomer",
     rounded(28, 24, 12) + [{"path": STATES, "value": ["hover"]}]),
    ("cast keyboard press with ambient light in right-to-left layout", "cast",
     [{"path": "/surface/body/size", "value": {"width": 30, "height": 22}},
      {"path": "/surface/body/surface/form/shape", "value": "organic"},
      {"path": "/surface/body/appearance/shapeAssignments/profiles/organic", "value": {"kind": "corners", "radii": {
          "topStart": "radius.4", "topEnd": "radius.3", "bottomEnd": "radius.4", "bottomStart": "radius.0"}}},
      {"path": STATES, "value": ["pressed"]},
      {"path": "/light/interaction", "value": None},
      {"path": "/surface/body/appearance/keyLight/direction", "value": {"x": -1, "y": -1}}]),
    ("frost rest square body without light", "frost",
     [{"path": "/surface/body/size", "value": {"width": 20, "height": 12}},
      {"path": STATES, "value": ["rest"]},
      {"path": "/light", "value": {"ambient": None, "interaction": None}},
      {"path": "/surface/body/postTreatmentBackdrop", "value": {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1}}]),
    ("elastomer disabled flat body with suppressed interaction", "elastomer",
     rounded(28, 24, 12) + [{"path": STATES, "value": ["disabled", "hover"]}]),
    ("cast hover under high contrast", "cast",
     rounded(28, 24, 12) + [{"path": STATES, "value": ["hover"]},
                            {"path": ENVIRONMENT + "/accessibilityPreferences/highContrast", "value": True}]),
    ("elastomer recessed press with a steep interaction", "elastomer",
     rounded(28, 24, 12) + [{"path": STATES, "value": ["pressed"]},
                            {"path": "/relief/commandBody", "value": {"kind": "recessed", "bodyThickness": 2, "amplitude": 1, "shoulderWidth": 2}},
                            {"path": "/light/interaction", "value": {"direction": {"x": -0.34, "y": -0.94}, "strength": 0.999}},
                            {"path": "/surface/body/appearance/keyLight/direction", "value": {"x": 1, "y": -0.5}}]),
    ("elastomer focused hover ring around a diagonal sweep", "elastomer",
     rounded(28, 24, 12) + [{"path": STATES, "value": ["hover", "focused"]},
                            {"path": "/surface/body/appearance/keyLight/direction", "value": {"x": 1, "y": -0.5}}]),
    ("cast focused rest ring around a square body", "cast",
     [{"path": "/surface/body/size", "value": {"width": 20, "height": 12}},
      {"path": STATES, "value": ["rest", "focused"]}]),
)


def scene_request(base, family, changes):
    request = admitting(copy.deepcopy(base))
    theme = fixture_theme(request)
    theme["materialAssignments"]["control"]["interactive"] = family
    radius = next((change["value"] for change in changes if change["path"] == "/radius"), None)
    if radius is not None:
        theme["tokens"]["radius"]["5"]["$value"]["value"] = radius
    request["surface"]["body"]["theme"]["themeSource"] = json.dumps(theme)
    return apply_changes(request, [change for change in changes if change["path"] != "/radius"])


def focus_ring(request):
    """The spec/27 ring color, gap and stroke: focus, else outline.strong, at 3:1 against the surround."""
    if "focused" not in request["surface"]["body"]["surface"]["states"]["states"]:
        return None
    surround = request["surface"]["surroundingColor"]["components"]
    for role in ("focus", "outline.strong"):
        color = fixture_color(request, role)
        if contrast(color, surround) >= FOCUS_CONTRAST:
            return color, FOCUS_GAP, FOCUS_STROKE
    raise ValueError("no focus color meets 3:1 against the surround")


def response_for(request):
    phase = phase_of(request["surface"]["body"]["surface"]["states"]["states"])
    if phase == "rest":
        return {"bodyMix": 0, "depthScale": 1}
    family = fixture_theme(request)["materialAssignments"]["control"]["interactive"]
    return request["commandAppearance"]["profiles"][family][phase]


def invoke(command, source, options):
    return subprocess.run([*command, *options], input=source, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          timeout=120, check=False)


def check_bounded_png(result, width, height, bounds):
    """Compares every pixel with its independent bounds: RGB within one step, alpha exact."""
    if len(bounds) != width * height:
        raise ValueError("independent pixels must cover the complete requested image")
    data = decoded_png(result, width, height)
    worst = 0
    for index, (lower, upper) in enumerate(bounds):
        actual = tuple(data[4 * index:4 * index + 4])
        if (upper[3] == 0 or actual[3] == 0) and actual != (0, 0, 0, 0):
            raise ValueError(f"pixel {index}: transparent pixels must be clear")
        if not lower[3] <= actual[3] <= upper[3]:
            raise ValueError(f"pixel {index}: coverage {actual[3]} is outside [{lower[3]}, {upper[3]}]")
        if actual[3] == 0:
            continue
        difference = max(max(lower[channel] - actual[channel], actual[channel] - upper[channel], 0)
                         for channel in range(3))
        worst = max(worst, difference)
        if difference > 1:
            raise ValueError(f"pixel {index}: color {actual} is outside {lower}..{upper}")
    return worst


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("backend", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.backend[1:] if args.backend[:1] == ["--"] else args.backend
    if not command:
        parser.error("supply a backend command after --")
    base = load_json(ROOT / "conformance/ir/command-paint-request.json")
    checked, pixel_count, worst = 0, 0, 0
    for name, family, changes in SCENES:
        request = scene_request(base, family, changes)
        source = command_body_source(request, response_for(request))
        size = request["surface"]["body"]["size"]
        source_bytes = json.dumps(request).encode("utf-8")
        ring = focus_ring(request)
        margin = MARGIN if ring is None else MARGIN + FOCUS_GAP + FOCUS_STROKE
        for scale in SCALES:
            for samples in SAMPLES:
                width = math.ceil((size["width"] + 2 * margin) * scale)
                height = math.ceil((size["height"] + 2 * margin) * scale)
                options = ["-", str(width), str(height), str(-margin), str(-margin), str(scale), str(samples)]
                bounds = lit_pixels(source, width, height, (-margin, -margin), scale, samples, ring=ring)
                if ring is not None and not any(lower == upper and lower[:3] == tuple(
                        round(255 * c) for c in ring[0]) and lower[3] == 255 for lower, upper in bounds):
                    raise ValueError(f"{name}: the independent ring covers no complete pixel")
                result = invoke(command, source_bytes, options)
                try:
                    difference = check_bounded_png(result, width, height, bounds)
                except ValueError as error:
                    raise ValueError(f"{name} at scale {scale} with {samples} samples: {error}") from error
                repeated = invoke(command, source_bytes, options)
                if repeated.returncode != 0 or repeated.stderr or repeated.stdout != result.stdout:
                    raise ValueError(f"{name}/{scale}/{samples}: output is not deterministic")
                worst = max(worst, difference)
                checked += 1
                pixel_count += width * height
    failing = apply_changes(base, [{"path": "/relief/commandBody/amplitude", "value": 1.5}])
    for name, source in (("invalid relief", json.dumps(failing).encode("utf-8")),
                         ("oversized", b" " * 1_048_577), ("invalid UTF-8", b"\xff"), ("malformed", b"{")):
        result = invoke(command, source, ["-", "1", "1", "0", "0", "1", "1"])
        if result.returncode != 1 or result.stdout or not result.stderr:
            raise ValueError(f"{name}: invalid input must fail atomically")
        checked += 1
    for name, options in (("zero width", ["-", "0", "1", "0", "0", "1", "1"]),
                          ("nonfinite scale", ["-", "1", "1", "0", "0", "nan", "1"]),
                          ("invalid samples", ["-", "1", "1", "0", "0", "1", "0"]),
                          ("resource limit", ["-", "4294967295", "1", "0", "0", "1", "1"])):
        result = invoke(command, json.dumps(base).encode("utf-8"), options)
        if result.returncode != 1 or result.stdout or not result.stderr:
            raise ValueError(f"{name}: invalid viewport must fail atomically")
        checked += 1
    usage = invoke(command, b"", [])
    if usage.returncode != 2 or usage.stdout or not usage.stderr:
        raise ValueError("missing arguments must produce status 2 and usage on stderr")
    checked += 1
    print(f"Lit body raster passed {checked} cases and {pixel_count} independent pixels; "
          f"worst RGB difference {worst}, exact alpha")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.TimeoutExpired) as error:
        raise SystemExit(f"FAIL: {error}") from error
