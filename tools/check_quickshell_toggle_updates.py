import copy
import json
import math

from jsonschema.exceptions import ValidationError
from PIL import Image

from check_schemas import validator_for
from check_command_paint_backend import disabled_label_bound, disabled_label_mismatch
from check_qt_command_label_runtime import verify_layout
from check_qt_toggle_control_runtime import request_for
from check_quickshell_command_updates import (
    contrast, inline_color, run_updates, unique_object, updates_parser, verify_authored_focus,
    verify_native_focus, verify_typography,
)
from check_surface_paint_backend import surface_paint_channel_mismatch


def authored_cases():
    cases = []
    for theme in ("light", "dark"):
        for direction in ("ltr", "rtl"):
            for checked in (False, True):
                cases.append({"name": f"{theme}-{direction}-{'on' if checked else 'off'}",
                              "request": request_for(theme, direction=direction), "checked": checked})
    for theme in ("light", "dark"):
        for language, text, direction in (
                ("german", "Automatische Aktualisierungen herunterladen", "ltr"),
                ("arabic", "تحديثات تلقائية Automatic updates", "rtl")):
            cases.append({"name": f"{theme}-{language}", "request": request_for(theme, text, direction)})
        for scale in (1.5, 2):
            request = request_for(theme)
            request["content"]["environment"]["textScale"] = scale
            cases.append({"name": f"{theme}-text-{str(scale).replace('.', '-')}", "request": request,
                          "checked": True})
    for name in ("light-ltr-off", "light-ltr-on", "dark-ltr-off", "dark-rtl-on"):
        baseline = next(case for case in cases if case["name"] == name)
        cases.append({**copy.deepcopy(baseline), "name": name + "-focused", "focused": True})
    source = request_for("light")
    cases.append({"name": "disabled", "request": source, "enabled": False, "checked": True})
    cases.append({"name": "dark-disabled", "request": request_for("dark"), "enabled": False})
    high_contrast = request_for("light")
    high_contrast["content"]["environment"]["accessibilityPreferences"]["highContrast"] = True
    cases.append({"name": "disabled-high-contrast", "request": high_contrast, "enabled": False, "checked": True})
    # The label guard applies to the undimmed label, so it rejects before dimming.
    label = copy.deepcopy(source)
    label["minimumLabelContrast"] = 21
    thumb = copy.deepcopy(source)
    thumb["minimumThumbContrast"] = 21
    for name, request, options in (
        ("malformed", "{", {"expectedError": "EOF while parsing an object"}),
        ("unsupported-focus-policy", source, {"disabledFocusable": True,
            "expectedError": "does not support discoverable disabled focus"}),
        ("unsupported-transform", source, {"rotation": 10,
            "expectedError": "requires matching native translation and pixel density"}),
        ("thumb-contrast", thumb, {"expectedError": "thumb edge contrast against painted track"}),
        ("disabled-label-contrast", label, {"enabled": False, "expectedError": "toggle label contrast is"}),
    ):
        cases.append({"name": name, "request": request, "reject": True, **options})
        recovery = {"name": "recover-" + name, "request": source, "enabled": True}
        if name == "unsupported-transform":
            recovery["renewPlacement"] = True
        cases.append(recovery)
    for theme_name in ("light", "dark"):
        baseline = next(case for case in cases if case["name"] == theme_name + "-ltr-off-focused")
        fallback = {**copy.deepcopy(baseline), "name": theme_name + "-focus-fallback"}
        theme = json.loads(fallback["request"]["content"]["themeSource"])
        path = theme["colorAssignments"]["roles"]["surface.base"]
        theme["opaqueColorAssignments"]["roles"]["focus"] = path
        fallback["request"]["content"]["themeSource"] = json.dumps(theme)
        cases.append(fallback)
        rejected = {**copy.deepcopy(fallback), "name": theme_name + "-focus-contrast",
                    "reject": True, "expectedError": "focus indicator contrast is insufficient"}
        # Light track edges already fall back to the strong outline. Keep edge
        # readability satisfied so this case reaches the focus contrast guard.
        roles = theme["opaqueColorAssignments"]["roles"]
        roles["outline"] = roles.get("outline.strong", theme["colorAssignments"]["roles"]["outline.strong"])
        roles["outline.strong"] = path
        rejected["request"]["content"]["themeSource"] = json.dumps(theme)
        cases.append(rejected)
        cases.append({**copy.deepcopy(baseline), "name": theme_name + "-focus-recovery"})
    for name, theme, checked, options in (
        ("focus-during-preparation", "light", False, {"focused": True, "focusDuringPreparation": True}),
        ("blur-during-preparation", "dark", True, {"focusDuringPreparation": False}),
        ("unmount-pending", "light", True, {"focused": True, "unmount": "pending"}),
        ("unmount-from-callback", "dark", False, {"focused": True, "unmount": "callback"}),
    ):
        cases.append({"name": name, "request": request_for(theme), "checked": checked, **options})
    return cases


def static_authored_states(state, hovered, checked):
    if state["hold"] is not None:
        raise ValueError("authored update acquired an unexpected hold")
    flags = ["rest"] if state["enabled"] and not hovered else []
    if hovered:
        flags.append("hover")
    if state["focused"]:
        flags.append("focused")
    if checked:
        flags.append("checked")
    if not state["enabled"]:
        flags.append("disabled")
    return {"schemaVersion": "0.1.0", "states": flags}


def verify_part_paint(request, name, part, states):
    theme = json.loads(request["content"]["themeSource"], object_pairs_hook=unique_object)
    config = request[name]
    surface = config["surface"]
    group, role = surface["materialRole"].split(".")
    family = theme["materialAssignments"][group][role]
    body = part["paint"]["body"]
    phase = next((phase for phase in ("disabled", "pressed", "hover") if phase in states["states"]), "rest")
    response = {"bodyMix": 0, "depthScale": 1} if phase == "rest" \
        else config["interactionAppearance"]["profiles"][family][phase]
    color_role = config["checkedColorRole"] if request["checked"] else surface["colorRole"]
    color = inline_color(theme, color_role)
    mix = response["bodyMix"]
    color["components"] = [channel * (1 + mix) if mix < 0 else channel + (1 - channel) * mix
                           for channel in color["components"]]
    if part["part"] != name or part["checked"] != request["checked"] or part["phase"] != phase \
            or part["response"] != response or body["states"] != states \
            or body["materialFamily"] != family or body["colorRole"] != color_role \
            or any(body[field] != surface[field] for field in ("materialRole", "form")) \
            or body["foregroundRole"] != config["foregroundRole"] or body["pigment"]["body"] != color:
        raise ValueError(f"native {name} paint differs from the authored theme, selection or response")
    size = request["layout"][name + "Size"]
    if body["geometry"]["front"]["bounds"] != {"x": 0, "y": 0, **size}:
        raise ValueError(f"native {name} front differs from the authored part allocation")
    if surface_paint_channel_mismatch(part["paint"]):
        raise ValueError(f"native {name} focus binding or silhouette differs from the current body")
    if ("focus" in part["paint"]) != (name == "track" and request["activation"]["focused"]):
        raise ValueError("track-only navigation paint disagrees with actual focus")
    if "focus" in part["paint"]:
        verify_authored_focus(theme, request["background"], part["paint"]["focus"]["indicator"])


def verify_snapshot(snapshot, request):
    members = {"label", "labelOrigin", "labelForeground", "labelBackground", "labelContrastRatio",
               "layout", "track", "thumb", "hitRegion", "accessibility", "thumbContrastRatio"}
    if set(snapshot) != members:
        raise ValueError("native preparation did not publish exactly the complete Toggle channels")
    for member, schema in (("label", "command-label-ir"), ("layout", "toggle-layout-ir"),
                           ("track", "toggle-part-paint-ir"), ("thumb", "toggle-part-paint-ir"),
                           ("hitRegion", "hit-region-ir"), ("accessibility", "toggle-accessibility-ir")):
        validator_for(f"schemas/{schema}.schema.json").validate(snapshot[member])
    activation = request["activation"]
    checked = request["checked"]
    states = static_authored_states(activation, request["hovered"], checked)
    for name in ("track", "thumb"):
        verify_part_paint(request, name, snapshot[name], states)
    if snapshot["layout"]["checked"] != checked:
        raise ValueError("native layout disagrees with the checked owner")
    theme = json.loads(request["content"]["themeSource"], object_pairs_hook=unique_object)
    foreground = inline_color(theme, request["labelForegroundRole"])
    published = snapshot["labelForeground"]
    if activation["enabled"]:
        label_differs = published != foreground
    else:
        label_differs = published["alpha"] != 1 or disabled_label_mismatch(
            published["components"], request["background"]["components"], foreground["components"],
            disabled_label_bound(request["content"]["environment"])) is not None
    ratio = contrast(published["components"], request["background"]["components"])
    guarded = contrast(foreground["components"], request["background"]["components"])
    if snapshot["labelOrigin"] != request["labelOrigin"] or snapshot["labelBackground"] != request["background"] \
            or label_differs \
            or not math.isclose(snapshot["labelContrastRatio"], ratio, rel_tol=0, abs_tol=1e-12) \
            or guarded < request["minimumLabelContrast"]:
        raise ValueError("native label colors differ from the authored theme or contrast guard")
    if type(snapshot["thumbContrastRatio"]) not in (int, float) \
            or not request["minimumThumbContrast"] <= snapshot["thumbContrastRatio"] <= 21:
        raise ValueError("native thumb contrast does not satisfy the authored minimum")
    accessibility = dict(snapshot["accessibility"])
    accessibility.pop("role")
    expected = {"schemaVersion": "0.1.0", "name": request["content"]["text"],
                "description": request["description"], "value": None,
                "state": {"enabled": activation["enabled"], "focused": activation["focused"], "checked": checked},
                "actions": [{"kind": "invoke", "available": activation["enabled"]}],
                "focusable": request["focusable"], "relationships": []}
    if accessibility != expected:
        raise ValueError("complete semantic dimensions disagree with native content/current state")
    if snapshot["label"]["text"] != request["content"]["text"] \
            or snapshot["label"]["layoutDirection"] != request["content"]["environment"]["layoutDirection"]:
        raise ValueError("snapshot changed content or direction")


def verify_case(case, report, scale):
    fields = {"name", "ready", "error", "snapshot", "target", "state", "checked", "hovered", "ignored",
              "deviceScale", "renderer", "publications", "unmounts", "measurements", "nativeFocused", "nativeFocus"}
    if set(report) != fields or type(report["deviceScale"]) not in (int, float) or report["deviceScale"] != scale:
        raise ValueError("native toggle report fields or actual device scale differ")
    if report["renderer"] != "software":
        raise ValueError("native toggle did not use the requested software renderer")
    if not all(isinstance(report[field], str) for field in ("error", "snapshot", "target")) \
            or not all(type(report[field]) is bool
                       for field in ("ready", "checked", "hovered", "ignored", "nativeFocused")) \
            or not isinstance(report["publications"], list) or not isinstance(report["unmounts"], list):
        raise ValueError("native toggle report types differ")
    unmount = case.get("unmount")
    if report["unmounts"] != ([{"published": unmount == "callback", "prepared": True, "nativeFocused": True}]
                              if unmount else []):
        raise ValueError("native toggle was not unloaded focused with the required pending or published frame")
    verify_native_focus(report)
    validator_for("schemas/activation-state.schema.json").validate(report["state"])
    if report["state"]["focused"] != report["nativeFocused"]:
        raise ValueError("native focus observation and committed state disagree")
    checked = case.get("checked", False)
    if report["checked"] != checked:
        raise ValueError("native checked owner differs from the authored replacement")
    if case.get("reject"):
        if case.get("expectedError", "") not in report["error"]:
            raise ValueError("rejected replacement reported a different failure cause")
        if not report["error"] or report["ready"] or report["snapshot"] or report["target"] \
                or not report["ignored"] or report["publications"] or report["measurements"]:
            raise ValueError("rejected replacement retained a toggle channel")
        return
    if report["error"] or not report["ready"] or report["ignored"] or not report["publications"]:
        raise ValueError("native toggle did not publish complete supported content")
    state = report["state"]
    enabled = case.get("enabled", True)
    if state != {"schemaVersion": "0.1.0", "enabled": enabled, "focused": case.get("focused", False), "hold": None} \
            or report["hovered"]:
        raise ValueError("authored update changed the expected native interaction state")
    request = copy.deepcopy(case["request"])
    request.update(activation=state, hovered=False, checked=checked, focusable=enabled)
    snapshot = json.loads(report["snapshot"], object_pairs_hook=unique_object)
    verify_snapshot(snapshot, request)
    verify_typography(case, snapshot["label"])
    verify_layout(snapshot["label"], report["measurements"], request["content"])
    if json.loads(report["target"], object_pairs_hook=unique_object) != snapshot["hitRegion"]:
        raise ValueError("native target and published snapshot disagree")
    for publication in report["publications"]:
        if set(publication) != {"configuration", "snapshot", "state", "checked", "hovered",
                                "nativeFocused", "nativeFocus"}:
            raise ValueError("native publication observation is incomplete")
        source = json.loads(publication["configuration"], object_pairs_hook=unique_object)
        if source != case["request"] or publication["snapshot"]["label"]["text"] != source["content"]["text"]:
            raise ValueError("consumer observed a mixed authored generation")
        validator_for("schemas/activation-state.schema.json").validate(publication["state"])
        if any(type(publication[field]) is not bool for field in ("checked", "hovered", "nativeFocused")):
            raise ValueError("publication native observation is not Boolean")
        verify_native_focus(publication)
        if publication["state"]["focused"] != publication["nativeFocused"]:
            raise ValueError("consumer callback disagrees with native focus observation")
        if publication["checked"] != checked:
            raise ValueError("consumer callback observed a different checked owner")
        if publication["state"]["focused"] != case.get("focusDuringPreparation", publication["state"]["focused"]):
            raise ValueError("consumer callback did not carry the focus changed during preparation")
        source.update(activation=publication["state"], hovered=publication["hovered"],
                      checked=publication["checked"], focusable=publication["state"]["enabled"])
        verify_snapshot(publication["snapshot"], source)
        verify_typography(case, publication["snapshot"]["label"])
        if publication["snapshot"]["label"] != snapshot["label"]:
            raise ValueError("consumer callback changed the fixed authored layout")
        if publication["snapshot"]["hitRegion"] != snapshot["hitRegion"]:
            raise ValueError("consumer callback changed the fixed authored target")


def matches(pixel, color):
    return pixel[3] == 255 and all(abs(pixel[channel] - round(color[channel] * 255)) <= 1 for channel in range(3))


def verify_capture(path, snapshot, scale, background):
    with Image.open(path) as source:
        if source.format != "PNG" or source.size != (round(500 * scale), round(800 * scale)):
            raise ValueError("native toggle capture format or dimensions differ")
        image = source.convert("RGBA")
    for color in (background, snapshot["labelForeground"]):
        if color["colorSpace"] != "srgb" or color["alpha"] != 1:
            raise ValueError("capture requires opaque sRGB surroundings and label foreground")
    if not matches(image.getpixel((0, 0)), background["components"]):
        raise ValueError("native toggle surroundings differ from the authored background")
    target = snapshot["hitRegion"]["bounds"]
    origin_x, origin_y = 50 - target["x"], 50 - target["y"]

    def pixel(x, y, description):
        column, row = math.floor((origin_x + x) * scale), math.floor((origin_y + y) * scale)
        if not 0 <= column < image.width or not 0 <= row < image.height:
            raise ValueError(f"native {description} lies outside the capture")
        return image.getpixel((column, row))

    layout = snapshot["layout"]
    thumb = layout["thumbBounds"]
    uncovered = layout["onThumbBounds"] if not layout["checked"] else layout["offThumbBounds"]
    for name, bounds in (("thumb", thumb), ("track", uncovered)):
        color = snapshot[name]["paint"]["body"]["pigment"]["body"]
        if color["colorSpace"] != "srgb" or color["alpha"] != 1:
            raise ValueError("capture requires opaque sRGB part pigment")
        if not matches(pixel(bounds["x"] + bounds["width"] / 2, bounds["y"] + bounds["height"] / 2, name),
                       color["components"]):
            raise ValueError(f"native capture {name} pigment differs from its current authored snapshot")
    label = snapshot["label"]
    bounds = label["labelBounds"]
    offset = snapshot["labelOrigin"]
    typography = label["typography"]
    line_height = typography["fontSize"]["value"] * typography["lineHeight"]
    lines = round(bounds["height"] / line_height)
    if lines < 1 or abs(bounds["height"] - lines * line_height) > 1 / 1024:
        raise ValueError("native complete label does not retain fixed line heights")
    left = math.floor((origin_x + offset["x"] + bounds["x"]) * scale)
    right = math.ceil((origin_x + offset["x"] + bounds["x"] + bounds["width"]) * scale)
    top = origin_y + offset["y"] + bounds["y"]
    ink = snapshot["labelForeground"]["components"]
    for line in range(lines):
        first = math.floor((top + line * line_height) * scale)
        last = math.ceil((top + (line + 1) * line_height) * scale)
        if left < 0 or right > image.width or first < 0 or last > image.height:
            raise ValueError("native complete label lies outside the capture")
        if not any(matches(image.getpixel((x, y)), ink) for y in range(first, last) for x in range(left, right)):
            raise ValueError(f"native capture has no label ink on line {line + 1}")
    focus = snapshot["track"]["paint"].get("focus")
    if focus is not None:
        indicator = focus["indicator"]
        if indicator["color"]["colorSpace"] != "srgb" or indicator["color"]["alpha"] != 1:
            raise ValueError("capture requires the supported opaque sRGB focus indicator")
        silhouette = snapshot["track"]["paint"]["body"]["geometry"]["silhouette"]["bounds"]
        x, y, width, height = (silhouette[field] for field in ("x", "y", "width", "height"))
        distance = indicator["gap"] + indicator["strokeWidth"] / 2
        for point in ((x + width / 2, y - distance), (x + width / 2, y + height + distance),
                      (x - distance, y + height / 2), (x + width + distance, y + height / 2)):
            if not matches(pixel(*point, "focus indicator"), indicator["color"]["components"]):
                raise ValueError("native focus indicator is absent, shifted or differs from its snapshot")


def verify_output(output, case, report, scale):
    try:
        verify_case(case, report, scale)
        if not case.get("reject"):
            verify_capture(output / (case["name"] + ".png"), json.loads(report["snapshot"]), scale,
                           case["request"]["background"])
    except (ValueError, KeyError, TypeError, ValidationError) as error:
        raise ValueError(f"{case['name']}: {error}") from error


def main():
    args = updates_parser("Verify authored Toggle replacements in released Quickshell.").parse_args()
    return run_updates("toggle", args, {"FONT": args.font}, authored_cases, verify_output)


if __name__ == "__main__":
    raise SystemExit(main())
