"""Verify the lit Command and Toggle bodies drawn by released Quickshell against the independent lit control body arithmetic.

Every lit control scene is prepared from the authored themes and definitions, drawn through the Qt runtime views in
released Quickshell, captured from the window and compared pixel by pixel with bounds from the independent arithmetic.
The Command is drawn with a label that has no ink, because a scene draws no text, so every pixel of every body is compared.
"""
import copy
import itertools
import json
import math
import os
import sys
from concurrent.futures import ProcessPoolExecutor, as_completed
from concurrent.futures.process import BrokenProcessPool

from PIL import Image

from check_qt_command_snapshot_runtime import request_for as command_request
from check_qt_toggle_control_runtime import request_for as toggle_request
from check_quickshell_command_updates import run_updates, updates_parser
import conformance_font
from check_schemas import load_json
from lit_control_scenes import MANIFEST, expected_pixels, prepare
from material_scenarios import backend_document

PROBE = "lit-body-probe.qml"
VIEW = 50
SAMPLES = 4
TOLERANCE = 1
CONTROL_SHIFT = 2
COMMAND_TEXT = "\u200b"  # zero-width space: the label draws no ink
TOGGLE_TEXT = "Wi-Fi"
COMMAND_PADDING = {"start": 16, "end": 16, "top": 12, "bottom": 12}
TOGGLE_INSET = 12.25
TOGGLE_THUMB = 16


def phase_state(states):
    """The activation, hover and hold that make the native controls draw the states of a scene."""
    hold = {"kind": "pointer", "id": "primary", "inside": True} if "pressed" in states else None
    return {"schemaVersion": "0.1.0", "enabled": "disabled" not in states, "focused": "focused" in states,
            "hold": hold}, "hover" in states


def native_command(scene):
    """The Command snapshot request that draws a scene's body at the scene's size."""
    source = scene["request"]
    body = source["surface"]["body"]
    request = command_request("light", COMMAND_TEXT)
    request["content"].update(
        themeSource=body["theme"]["themeSource"], externalSources=body["theme"]["externalSources"],
        environment=copy.deepcopy(body["theme"]["environment"]),
        minimumSize=copy.deepcopy(body["size"]), maximumSize=copy.deepcopy(body["size"]), padding=COMMAND_PADDING)
    request["content"]["environment"]["textScale"] = 1
    request["body"].update(
        surface=copy.deepcopy(body["surface"]), appearance=copy.deepcopy(body["appearance"]),
        commandAppearance=copy.deepcopy(source["commandAppearance"]), light=copy.deepcopy(source["light"]),
        relief=copy.deepcopy(source["relief"]), foregroundRole=body["foregroundRole"],
        adjacentColor=copy.deepcopy(body["adjacentColor"]),
        postTreatmentBackdrop=copy.deepcopy(body["postTreatmentBackdrop"]),
        surroundingColor=copy.deepcopy(source["surface"]["surroundingColor"]),
        minimumContentContrast=4.5, minimumEdgeContrast=body["minimumEdgeContrast"])
    request["body"]["surface"]["states"] = {"schemaVersion": "0.1.0", "states": ["rest"]}
    request["activation"], request["hovered"] = phase_state(body["surface"]["states"]["states"])
    return request


def native_toggle(track, thumb):
    """The Toggle snapshot request that draws the track and thumb scenes of one state."""
    request = toggle_request("light", TOGGLE_TEXT, track["request"]["surface"]["body"]["theme"]["environment"]["layoutDirection"])
    body = track["request"]["surface"]["body"]
    request["content"].update(
        themeSource=body["theme"]["themeSource"], externalSources=body["theme"]["externalSources"],
        environment=copy.deepcopy(body["theme"]["environment"]))
    request["content"]["environment"]["textScale"] = 1
    for name, scene in (("track", track), ("thumb", thumb)):
        source = scene["request"]
        part = request[name]
        part.update(surface=copy.deepcopy(source["surface"]["body"]["surface"]),
                    appearance=copy.deepcopy(source["surface"]["body"]["appearance"]),
                    checkedColorRole=source["checkedColorRole"],
                    interactionAppearance=copy.deepcopy(source["interactionAppearance"]),
                    light=copy.deepcopy(source["light"]), relief=copy.deepcopy(source["relief"]),
                    foregroundRole=source["surface"]["body"]["foregroundRole"],
                    minimumContentContrast=source["surface"]["body"]["minimumContentContrast"],
                    minimumEdgeContrast=source["surface"]["body"]["minimumEdgeContrast"])
        part["surface"]["states"] = {"schemaVersion": "0.1.0", "states": ["rest"]}
    size = track["request"]["surface"]["body"]["size"]
    request["layout"] = {"trackSize": copy.deepcopy(size), "thumbSize": {"width": TOGGLE_THUMB, "height": TOGGLE_THUMB},
                         "insets": dict.fromkeys(("start", "end", "top", "bottom"), TOGGLE_INSET)}
    request["background"] = copy.deepcopy(track["request"]["surface"]["surroundingColor"])
    request["activation"], request["hovered"] = phase_state(body["surface"]["states"]["states"])
    request["checked"] = "checked" in body["surface"]["states"]["states"]
    return request


def with_thumbs(manifest):
    """The catalog plus a thumb scene, drawn from the same authored parts, for each track scene that has none."""
    names = {scene["name"] for scene in manifest["scenarios"]}
    derived = []
    for scene in manifest["scenarios"]:
        name = scene["name"].replace("-track-", "-thumb-")
        if scene["component"] == "toggle" and scene["part"] == "track" and name not in names:
            derived.append({**scene, "name": name, "part": "thumb",
                            "states": [state for state in scene["states"] if state != "focused"]})
    return {**manifest, "scenarios": manifest["scenarios"] + derived}


def authored_cases(theme_backend):
    manifest = with_thumbs(load_json(MANIFEST))
    bundle, resolutions = prepare(manifest, lambda request: backend_document(
        [theme_backend, "-"], request, "schemas/headless-result.schema.json", "scene theme", 30))
    scenes = {scene["name"]: scene for scene in bundle["scenarios"]}
    cases = []
    for name, scene in scenes.items():
        background = scene["request"]["surface"]["surroundingColor"]["components"]
        if scene["kind"] == "commandPaint":
            cases.append({"name": name, "kind": "command", "request": native_command(scene), "background": background,
                          "scenes": {"body": name}})
        elif scene["request"]["part"] == "track":
            thumb = scenes[name.replace("-track-", "-thumb-")]
            cases.append({"name": name, "kind": "toggle", "request": native_toggle(scene, thumb),
                          "background": background, "scenes": {"track": name, "thumb": thumb["name"]}})
    return cases, scenes, resolutions, bundle["capture"]


def over(lower, upper, low, high):
    """Bounds of a pixel after drawing a bound pair over bounds; the extremes lie at the corners."""
    results = []
    for under, color, alpha in itertools.product((lower, upper), (low, high), (low, high)):
        weight = alpha[3] / 255
        results.append(tuple(color[channel] * weight + under[channel] * (1 - weight) for channel in range(3)))
    return (tuple(min(result[channel] for result in results) for channel in range(3)),
            tuple(max(result[channel] for result in results) for channel in range(3)))


def device_rect(left, top, width, height, scale):
    """The whole device pixels that cover a logical rectangle: column, row, width, height."""
    column, row = math.floor(left * scale), math.floor(top * scale)
    return column, row, math.ceil((left + width) * scale) - column, math.ceil((top + height) * scale) - row


def part_bounds(layer, rect, scale):
    """Independent bounds of one part over a device rectangle, with the part's body origin at its logical placement."""
    scene, resolution, response, placement = layer
    column, row, width, height = rect
    view = {"origin": {"x": column / scale - placement[0], "y": row / scale - placement[1]},
            "width": width, "height": height, "pixelsPerUnit": scale, "samplesPerAxis": SAMPLES}
    return expected_pixels(scene["request"], resolution, response, view)


def measure(task, shift=0):
    """Worst RGB distance of a window capture from the bounds of its parts drawn in order over an opaque background.

    `shift` raises the red channel of every captured pixel, to prove that a small defect is detected."""
    name, path, rect, layers, background, scale = task
    column, row, width, height = rect
    parts = [(part_bounds(layer, part_rect, scale), part_rect) for layer, part_rect in layers]
    with Image.open(path) as source:
        image = source.convert("RGBA")
    worst = 0
    for y in range(height):
        for x in range(width):
            px, py = column + x, row + y
            lower = upper = background
            for bounds, (left, top, part_width, part_height) in parts:
                lx, ly = px - left, py - top
                if 0 <= lx < part_width and 0 <= ly < part_height:
                    low, high = bounds[ly * part_width + lx]
                    if high[3]:
                        lower, upper = over(lower, upper, low, high)
            actual = image.getpixel((px, py))
            if actual[3] != 255:
                raise ValueError(f"{name}: capture pixel ({px}, {py}) is not opaque: {actual}")
            actual = (actual[0] + shift, *actual[1:3])
            difference = max(max(lower[channel] - actual[channel], actual[channel] - upper[channel], 0)
                             for channel in range(3))
            if difference > TOLERANCE:
                raise ValueError(f"{name}: capture pixel ({px}, {py}) is {actual}, outside "
                                 f"{tuple(round(v, 2) for v in lower)}..{tuple(round(v, 2) for v in upper)}")
            worst = max(worst, difference)
    return name, scale, width * height, worst


def discriminate(task):
    """A capture whose red channel is two levels higher must fail, so that the comparison would notice a small defect."""
    try:
        measure(task, CONTROL_SHIFT)
    except ValueError:
        return task[0], task[5], 0, 0
    raise ValueError(f"{task[0]}: a {CONTROL_SHIFT}-level shift of the capture was not detected")


def plan_case(context, output, case, report, scale):
    """Check the native report and return the pixel comparison of the capture."""
    scenes, resolutions, margin = context
    if report["deviceScale"] != scale or report["renderer"] != "software":
        raise ValueError(f"{case['name']}: native capture was not drawn by the software renderer at scale {scale}")
    snapshot = json.loads(report["snapshot"])
    background = tuple(round(255 * channel) for channel in case["background"])
    path = output / (case["name"] + ".png")
    with Image.open(path) as source:
        if source.format != "PNG" or source.size != (round(500 * scale), round(400 * scale)):
            raise ValueError(f"{case['name']}: native capture format or dimensions differ")

    def layer(name, placement):
        size = scenes[name]["request"]["surface"]["body"]["size"]
        rect = device_rect(placement[0] - margin, placement[1] - margin, size["width"] + 2 * margin,
                           size["height"] + 2 * margin, scale)
        return (scenes[name], *resolutions[name], placement), rect

    if case["kind"] == "command":
        name = case["scenes"]["body"]
        if snapshot["label"]["size"] != scenes[name]["request"]["surface"]["body"]["size"]:
            raise ValueError(f"{name}: native body size {snapshot['label']['size']} differs from the scene")
        states = snapshot["paint"]["paint"]["body"]["states"]["states"]
        layers = [layer(name, (VIEW, VIEW))]
        expected = scenes[name]["request"]["surface"]["body"]["surface"]["states"]["states"]
    else:
        track, thumb = case["scenes"]["track"], case["scenes"]["thumb"]
        frame, grip = snapshot["layout"]["trackBounds"], snapshot["layout"]["thumbBounds"]
        if {"width": frame["width"], "height": frame["height"]} != scenes[track]["request"]["surface"]["body"]["size"] \
                or (grip["width"], grip["height"]) != (TOGGLE_THUMB, TOGGLE_THUMB):
            raise ValueError(f"{track}: native Toggle layout differs from the scene sizes")
        layers = [layer(track, (VIEW + frame["x"], VIEW + frame["y"])),
                  layer(thumb, (VIEW + grip["x"], VIEW + grip["y"]))]
        states = snapshot["track"]["paint"]["body"]["states"]["states"]
        expected = scenes[track]["request"]["surface"]["body"]["surface"]["states"]["states"]
        if snapshot["thumb"]["paint"]["body"]["states"]["states"] != states:
            raise ValueError(f"{track}: native track and thumb draw different states")
    if sorted(states) != sorted(expected):
        raise ValueError(f"{case['name']}: native states {states} differ from the scene states {expected}")
    return case["name"], path, layers[0][1], layers, background, scale


def run_measurements(plans, jobs):
    """Compare every capture, and one shifted capture that must fail, in worker processes started after the captures."""
    results = []
    with ProcessPoolExecutor(max(1, jobs)) as pool:
        futures = [pool.submit(measure, plan) for plan in plans] + [pool.submit(discriminate, plans[0])]
        try:
            for future in as_completed(futures):
                results.append(future.result())
        except BrokenProcessPool as error:
            raise ValueError(f"a comparison worker died: {error}") from error
        finally:
            for future in futures:
                future.cancel()
    return [result for result in results if result[2]]


def main():
    parser = updates_parser(__doc__)
    parser.add_argument("--theme-backend", required=True)
    parser.add_argument("--jobs", type=int, default=max(1, (os.cpu_count() or 2) - 1))
    args = parser.parse_args()
    cases, scenes, resolutions, capture = authored_cases(args.theme_backend)
    context = (scenes, resolutions, capture["margin"])
    plans = []
    status = run_updates("lit_body", args, {"FONTS": lambda: conformance_font.qml_sources(args.font_dir)},
                         lambda: cases,
                         lambda output, case, report, scale: plans.append(plan_case(context, output, case, report, scale)),
                         probe=PROBE, stable_targets=False, summary=False)
    if status:
        return status
    try:
        results = run_measurements(plans, args.jobs)
    except (ValueError, OSError) as error:
        print(f"FAIL Quickshell lit bodies: {error}", file=sys.stderr)
        return 1
    for scale in sorted({scale for _, scale, _, _ in results}):
        subset = [result for result in results if result[1] == scale]
        print(f"Lit bodies at device scale {scale}: {len(subset)} native captures, {sum(r[2] for r in subset)} pixels "
              f"compared, none skipped, worst RGB distance {max(r[3] for r in subset):.3f}")
    print(f"A {CONTROL_SHIFT}-level shift of one capture is detected")
    return 0


if __name__ == "__main__":
    sys.exit(main())
