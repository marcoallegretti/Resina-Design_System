import argparse
import copy
import json
import math
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

from jsonschema.exceptions import ValidationError
from PIL import Image

from check_schemas import validator_for
from check_qt_command_snapshot_runtime import ROOT, request_for, verify_snapshot
from check_qt_command_label_runtime import verify_layout
from check_surface_paint_backend import surface_paint_channel_mismatch


def authored_cases():
    cases = []
    for theme in ("light", "dark"):
        for language, text in (("latin", "Reconnect"),
                               ("german", "Verbindung mit gespeicherten Einstellungen wiederherstellen"),
                               ("arabic", "اتصل\u2029Reconnect")):
            for scale in (1, 1.0006, 1.5, 2):
                request = request_for(theme, text)
                request["content"]["environment"].update(
                    textScale=scale, layoutDirection="rtl" if language == "arabic" else "ltr")
                cases.append({"name": f"{theme}-{language}-{str(scale).replace('.', '-')}",
                              "request": request, "enabled": True})
    for direction in ("ltr", "rtl"):
        request = request_for("light", "Reconnect to Wi-Fi")
        request["content"]["environment"].update(textScale=1, layoutDirection=direction)
        request["content"].update(minimumSize={"width": 200, "height": 100},
                                  maximumSize={"width": 300, "height": 600},
                                  padding={"start": 40, "end": 24, "top": 30, "bottom": 20})
        theme = json.loads(request["content"]["themeSource"])
        theme["typographyAssignments"]["roles"]["label"].update(
            fontSize="type.size.2", fontWeight="type.weight.bold",
            lineHeight="type.line.body", letterSpacing="type.tracking.normal")
        request["content"]["themeSource"] = json.dumps(theme)
        cases.append({"name": "authored-layout-" + direction, "request": request,
                      "typography": {"familyRole": "sans", "fontSize": {"value": 16, "unit": "px"},
                                     "fontWeight": 700, "lineHeight": 1.48,
                                     "letterSpacing": {"value": 0, "unit": "px"}}})
    for theme_name in ("light", "dark"):
        request = request_for(theme_name, "Reconnect to Wi-Fi")
        request["content"]["environment"]["textScale"] = 1
        request["content"]["fontFamily"] = {"role": "monospace", "name": "DejaVu Sans Mono"}
        theme = json.loads(request["content"]["themeSource"])
        theme["typographyAssignments"]["roles"]["label"]["familyRole"] = "monospace"
        request["content"]["themeSource"] = json.dumps(theme)
        cases.append({"name": theme_name + "-font-family", "request": request,
                      "typography": {"familyRole": "monospace", "fontSize": {"value": 14, "unit": "px"},
                                     "fontWeight": 500, "lineHeight": 1.34,
                                     "letterSpacing": {"value": 0.15, "unit": "px"}}})
    primary = json.loads((ROOT / "definitions/components/command.json").read_text())["variants"]["primary"]["body"]
    for theme_name in ("light", "dark"):
        request = request_for(theme_name, "Reconnect")
        request["content"]["environment"]["textScale"] = 1
        request["body"]["surface"].update(
            materialRole=primary["materialRole"], colorRole=primary["colorRole"],
            form={"schemaVersion": "0.1.0", "shape": primary["shape"], "elevation": primary["elevation"]})
        request["body"]["foregroundRole"] = primary["contentRole"]
        cases.append({"name": theme_name + "-primary", "request": request, "enabled": True})
    for name in ("light-latin-1", "dark-latin-1", "light-primary", "dark-primary"):
        baseline = next(case for case in cases if case["name"] == name)
        cases.append({**copy.deepcopy(baseline), "name": name + "-focused", "focused": True})
    source = request_for("light", "Reconnect")
    cases.append({"name": "disabled", "request": source, "enabled": False})
    for name, request, options in (
        ("malformed", "{", {"expectedError": "EOF while parsing an object"}),
        ("unsupported-focus-policy", source, {"disabledFocusable": True,
            "expectedError": "does not support discoverable disabled focus"}),
        ("unsupported-transform", source, {"rotation": 10,
            "expectedError": "requires matching native translation and pixel density"}),
    ):
        cases.append({"name": name, "request": request, "reject": True, **options})
        recovery = {"name": "recover-" + name, "request": source, "enabled": True}
        if name == "unsupported-transform":
            recovery["renewPlacement"] = True
        cases.append(recovery)
    return cases


def verify_typography(case, label):
    scale = max(1, case["request"]["content"]["environment"]["textScale"])
    expected = case.get("typography", {
        "familyRole": "sans", "fontSize": {"value": 14 * scale, "unit": "px"},
        "fontWeight": 500, "lineHeight": 1.34,
        "letterSpacing": {"value": 0.15 * scale, "unit": "px"}})
    if label["typography"] != expected:
        raise ValueError("native label differs from authored typography")


def verify_authored_paint(request, snapshot):
    theme = json.loads(request["content"]["themeSource"], object_pairs_hook=unique_object)
    surface = request["body"]["surface"]
    group, role = surface["materialRole"].split(".")
    family = theme["materialAssignments"][group][role]
    paint = snapshot["paint"]
    body = paint["paint"]["body"]
    response = {"bodyMix": 0, "depthScale": 1} if paint["phase"] == "rest" \
        else request["body"]["commandAppearance"]["profiles"][family][paint["phase"]]

    def inline_color(role):
        token = theme["tokenResolver"]["sets"]["theme"]["sources"][0]
        for member in theme["colorAssignments"]["roles"][role].split("."):
            token = token[member]
        value = token["$value"]
        return {"colorSpace": value["colorSpace"], "components": value["components"],
                "alpha": value.get("alpha", 1)}

    color = inline_color(surface["colorRole"])
    mix = response["bodyMix"]
    color["components"] = [channel * (1 + mix) if mix < 0 else channel + (1 - channel) * mix
                           for channel in color["components"]]
    if paint["response"] != response or body["materialFamily"] != family \
            or any(body[field] != surface[field] for field in ("materialRole", "colorRole", "form")) \
            or body["foregroundRole"] != request["body"]["foregroundRole"] \
            or body["pigment"]["body"] != color \
            or body["foreground"] != inline_color(request["body"]["foregroundRole"]) \
            or body["edge"]["color"] != inline_color("outline"):
        raise ValueError("native paint differs from the authored theme, form or response")
    verify_authored_geometry(request, snapshot)
    if surface_paint_channel_mismatch(paint["paint"]):
        raise ValueError("native focus binding or silhouette differs from the current body")


def verify_authored_geometry(request, snapshot):
    appearance = request["body"]["appearance"]
    theme = json.loads(request["content"]["themeSource"], object_pairs_hook=unique_object)
    sources = theme["tokenResolver"]["sets"]["foundation"]["sources"]
    if len(sources) != 1 or set(sources[0]) != {"$ref"}:
        raise ValueError("authored geometry requires the fixture's single foundation source")
    token = json.loads(request["content"]["externalSources"][sources[0]["$ref"]], object_pairs_hook=unique_object)
    elevation = request["body"]["surface"]["form"]["elevation"]
    for member in appearance["depthAssignments"]["roles"][elevation].split("."):
        token = token[member]
    dimension = token["$value"]
    if not isinstance(dimension, dict) or dimension.get("unit") != "px" \
            or type(dimension.get("value")) not in (int, float) \
            or not math.isfinite(dimension["value"]) or dimension["value"] < 0:
        raise ValueError("authored geometry requires a literal foundation depth in px")
    body = snapshot["paint"]["paint"]["body"]
    depth = dimension["value"] * snapshot["paint"]["response"]["depthScale"]
    light = appearance["keyLight"]["direction"]
    magnitude = math.hypot(light["x"], light["y"])
    if not math.isfinite(magnitude) or magnitude == 0:
        raise ValueError("authored geometry requires a finite nonzero key light")
    direction = {axis: light[axis] / magnitude for axis in ("x", "y")}
    offset = {axis: -direction[axis] * depth for axis in ("x", "y")}

    def equal_numbers(actual, expected):
        return set(actual) == set(expected) and all(
            math.isclose(actual[name], value, rel_tol=1e-12, abs_tol=1e-12)
            for name, value in expected.items())

    if not equal_numbers(body["lighting"]["direction"], direction) \
            or not equal_numbers(body["lighting"]["sideOffset"], offset):
        raise ValueError("native lighting differs from the authored depth response")
    geometry = body["geometry"]
    front = geometry["front"]["bounds"]
    edge = appearance["bands"][body["materialFamily"]]["edgeWidth"]
    silhouette = {"x": min(0, offset["x"]), "y": min(0, offset["y"]),
                  "width": front["width"] + abs(offset["x"]),
                  "height": front["height"] + abs(offset["y"])}
    interior = {**silhouette, "width": front["width"] - 2 * edge + abs(offset["x"]),
                "height": front["height"] - 2 * edge + abs(offset["y"])}
    if not equal_numbers(geometry["silhouette"]["bounds"], silhouette) \
            or not equal_numbers(geometry["edgeInterior"]["contour"]["bounds"], interior) \
            or not equal_numbers(geometry["edgeInterior"]["offset"], {"x": edge, "y": edge}):
        raise ValueError("native geometry differs from the authored depth response")


def unique_object(pairs):
    value = {}
    for name, member in pairs:
        if name in value:
            raise ValueError("duplicate native report member: " + name)
        value[name] = member
    return value


def verify_version(result):
    if result.returncode:
        diagnostic = result.stderr.strip().splitlines()
        cause = diagnostic[-1][-1000:] if diagnostic else "no diagnostic"
        raise ValueError("host version verification failed: " + cause)
    if result.stdout.strip() != "Quickshell 0.3.2 / Qt Quick 6.11.2":
        raise ValueError("requires released Quickshell 0.3.2 built and running with Qt 6.11.2")


def read_reports(log, cases):
    if len(log) > 16 * 1024 * 1024:
        raise ValueError("native output exceeds the probe limit")
    if "RESINA_COMMAND_FAIL" in log:
        raise ValueError("native command reported an execution failure")
    done = re.findall(r"RESINA_COMMAND_DONE ([0-9]+)(?:\s|$)", log)
    reports = [json.loads(source, object_pairs_hook=unique_object)
               for source in re.findall(r"RESINA_COMMAND_CASE (\{[^\n]*\})", log)]
    if done != [str(len(cases))] or [report.get("name") for report in reports] != [case["name"] for case in cases]:
        raise ValueError("native command omitted, repeated or reordered a case")
    return reports


def static_authored_states(state, hovered):
    if state["hold"] is not None:
        raise ValueError("authored update acquired an unexpected hold")
    flags = ["rest"] if state["enabled"] and not hovered else []
    if hovered:
        flags.append("hover")
    if state["focused"]:
        flags.append("focused")
    if not state["enabled"]:
        flags.append("disabled")
    return {"schemaVersion": "0.1.0", "states": flags}


def verify_case(case, report, scale):
    fields = {"name", "ready", "error", "snapshot", "target", "state", "hovered",
              "ignored", "deviceScale", "renderer", "publications", "measurements", "nativeFocused"}
    if set(report) != fields or type(report["deviceScale"]) not in (int, float) or report["deviceScale"] != scale:
        raise ValueError("native command report fields or actual device scale differ")
    if report["renderer"] != "software":
        raise ValueError("native command did not use the requested software renderer")
    if not all(isinstance(report[field], str) for field in ("error", "snapshot", "target")) \
            or not all(type(report[field]) is bool for field in ("ready", "hovered", "ignored", "nativeFocused")) \
            or not isinstance(report["publications"], list):
        raise ValueError("native command report types differ")
    validator_for("schemas/activation-state.schema.json").validate(report["state"])
    if report["state"]["focused"] != report["nativeFocused"]:
        raise ValueError("native focus observation and committed state disagree")
    if case.get("reject"):
        if case.get("expectedError", "") not in report["error"]:
            raise ValueError("rejected replacement reported a different failure cause")
        if not report["error"] or report["ready"] or report["snapshot"] or report["target"] \
                or not report["ignored"] or report["publications"] or report["measurements"]:
            raise ValueError("rejected replacement retained a command channel")
        return
    if report["error"] or not report["ready"] or report["ignored"] \
            or not report["publications"]:
        raise ValueError("native command did not publish complete supported content")
    state = report["state"]
    enabled = case.get("enabled", True)
    if state != {"schemaVersion": "0.1.0", "enabled": enabled, "focused": case.get("focused", False), "hold": None} \
            or report["hovered"]:
        raise ValueError("authored update changed the expected native interaction state")
    request = copy.deepcopy(case["request"])
    request.update(activation=state, hovered=False, focusable=enabled)
    states = static_authored_states(state, False)
    snapshot = json.loads(report["snapshot"], object_pairs_hook=unique_object)
    verify_snapshot(snapshot, request, states)
    verify_authored_paint(request, snapshot)
    verify_typography(case, snapshot["label"])
    verify_layout(snapshot["label"], report["measurements"], request["content"])
    if json.loads(report["target"], object_pairs_hook=unique_object) != snapshot["hitRegion"]:
        raise ValueError("native target and published snapshot disagree")
    for publication in report["publications"]:
        if set(publication) != {"configuration", "snapshot", "state", "hovered", "nativeFocused"}:
            raise ValueError("native publication observation is incomplete")
        source = json.loads(publication["configuration"], object_pairs_hook=unique_object)
        if source != case["request"] or publication["snapshot"]["label"]["text"] != source["content"]["text"]:
            raise ValueError("consumer observed a mixed authored generation")
        validator_for("schemas/activation-state.schema.json").validate(publication["state"])
        if any(type(publication[field]) is not bool for field in ("hovered", "nativeFocused")):
            raise ValueError("publication native observation is not Boolean")
        if publication["state"]["focused"] != publication["nativeFocused"]:
            raise ValueError("consumer callback disagrees with native focus observation")
        source.update(activation=publication["state"], hovered=publication["hovered"],
                      focusable=publication["state"]["enabled"])
        verify_snapshot(publication["snapshot"], source,
                        static_authored_states(publication["state"], publication["hovered"]))
        verify_authored_paint(source, publication["snapshot"])
        verify_typography(case, publication["snapshot"]["label"])
        if publication["snapshot"]["label"] != snapshot["label"]:
            raise ValueError("consumer callback changed the fixed authored layout")
        if publication["snapshot"]["hitRegion"] != snapshot["hitRegion"]:
            raise ValueError("consumer callback changed the fixed authored target")


def verify_capture(path, snapshot, scale, surrounding_color=None):
    with Image.open(path) as source:
        if source.format != "PNG" or source.size != (round(500 * scale), round(800 * scale)):
            raise ValueError("native command capture format or dimensions differ")
        image = source.convert("RGBA")
    label = snapshot["label"]
    bounds = label["labelBounds"]
    target = snapshot["hitRegion"]["bounds"]
    typography = label["typography"]
    line_height = typography["fontSize"]["value"] * typography["lineHeight"]
    lines = round(bounds["height"] / line_height)
    if lines < 1 or abs(bounds["height"] - lines * line_height) > 1 / 1024:
        raise ValueError("native complete label does not retain fixed line heights")
    foreground = snapshot["paint"]["paint"]["body"]["foreground"]
    if foreground["colorSpace"] != "srgb" or foreground["alpha"] != 1:
        raise ValueError("capture requires the supported opaque sRGB foreground")
    ink = tuple(round(channel * 255) for channel in foreground["components"])
    left = math.floor((50 - target["x"] + bounds["x"]) * scale)
    right = math.ceil((50 - target["x"] + bounds["x"] + bounds["width"]) * scale)
    top = 50 - target["y"] + bounds["y"]
    body = snapshot["paint"]["paint"]["body"]
    color = body["pigment"]["body"]
    if color["colorSpace"] != "srgb" or color["alpha"] != 1:
        raise ValueError("capture requires the supported opaque sRGB body pigment")
    body_x = math.floor((50 - target["x"] + label["size"]["width"] / 2) * scale)
    body_y = math.floor((50 - target["y"] + (body["geometry"]["content"]["offset"]["y"] + bounds["y"]) / 2) * scale)
    pixel = image.getpixel((body_x, body_y))
    if pixel[3] != 255 or any(abs(pixel[channel] - round(color["components"][channel] * 255)) > 1
                             for channel in range(3)):
        raise ValueError("native capture body pigment differs from its current authored snapshot")
    for line in range(lines):
        first = math.floor((top + line * line_height) * scale)
        last = math.ceil((top + (line + 1) * line_height) * scale)
        if left < 0 or right > image.width or first < 0 or last > image.height:
            raise ValueError("native complete label lies outside the capture")
        if not any(pixel[3] == 255 and all(abs(pixel[channel] - ink[channel]) <= 1 for channel in range(3))
                   for y in range(first, last) for x in range(left, right)
                   for pixel in (image.getpixel((x, y)),)):
            raise ValueError(f"native capture has no foreground ink on label line {line + 1}")
    focus = snapshot["paint"]["paint"].get("focus")
    if focus is not None:
        if surrounding_color is None or surrounding_color["colorSpace"] != "srgb" or surrounding_color["alpha"] != 1:
            raise ValueError("focus capture requires explicit opaque sRGB surroundings")
        background = tuple(round(channel * 255) for channel in surrounding_color["components"])
        pixel = image.getpixel((0, 0))
        if pixel[3] != 255 or any(abs(pixel[channel] - background[channel]) > 1 for channel in range(3)):
            raise ValueError("native focus surroundings differ from the guarded authored background")
        indicator = focus["indicator"]
        color = indicator["color"]
        if color["colorSpace"] != "srgb" or color["alpha"] != 1:
            raise ValueError("capture requires the supported opaque sRGB focus indicator")
        ink = tuple(round(channel * 255) for channel in color["components"])
        silhouette = body["geometry"]["silhouette"]["bounds"]
        x, y, width, height = (silhouette[field] for field in ("x", "y", "width", "height"))
        distance = indicator["gap"] + indicator["strokeWidth"] / 2
        points = ((x + width / 2, y - distance), (x + width / 2, y + height + distance),
                  (x - distance, y + height / 2), (x + width + distance, y + height / 2))
        for x, y in points:
            column = math.floor((50 - target["x"] + x) * scale)
            row = math.floor((50 - target["y"] + y) * scale)
            if not 0 <= column < image.width or not 0 <= row < image.height:
                raise ValueError("native focus indicator lies outside the capture")
            pixel = image.getpixel((column, row))
            if pixel[3] != 255 or any(abs(pixel[channel] - ink[channel]) > 1 for channel in range(3)):
                raise ValueError("native focus indicator is absent, shifted or differs from its snapshot")


def verify_stable_targets(cases, reports):
    targets = {}
    for case, report in zip(cases, reports):
        if case.get("reject"):
            continue
        source = json.dumps(case["request"], sort_keys=True)
        target = json.loads(report["target"], object_pairs_hook=unique_object)
        if targets.setdefault(source, target) != target:
            raise ValueError("availability or recovery changed the same authored target")


def main():
    parser = argparse.ArgumentParser(description="Verify authored Command replacements in released Quickshell.")
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--module-path", type=Path, required=True)
    parser.add_argument("--font", type=Path, required=True)
    parser.add_argument("--alternate-font", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--platform", choices=("offscreen", "xcb", "wayland"), default="offscreen")
    parser.add_argument("--scales", nargs="+", type=float, default=[1, 1.25, 2])
    args = parser.parse_args()
    try:
        if not args.scales or any(scale not in (1, 1.25, 2) for scale in args.scales):
            raise ValueError("requires device scales 1, 1.25 or 2")
        host, module, font = (path.resolve(strict=True) for path in (args.host, args.module_path, args.font))
        alternate_font = args.alternate_font.resolve(strict=True)
        environment = dict(os.environ, RESINA_QUICKSHELL_HOST=str(host), RESINA_QT_RUNTIME_MODULE=str(module))
        environment["QT_QPA_PLATFORM"] = args.platform
        version = subprocess.run([sys.executable, str(ROOT / "tools/run_qt_availability_probe.py"), "--version"],
                                 env=environment, capture_output=True, text=True, timeout=15)
        verify_version(version)
        args.output_dir.mkdir(parents=True, exist_ok=False)
        cases = authored_cases()
        for index, scale in enumerate(args.scales):
            output = args.output_dir / str(index)
            output.mkdir()
            shutil.copyfile(ROOT / "conformance/quickshell/command-updates-probe.qml", output / "config.qml")
            shutil.copytree(ROOT / "backends/quickshell/resina-qml/qml", output / "controls")
            case_file = output / "cases.json"
            case_file.write_text(json.dumps(cases), encoding="utf-8")
            environment.update(QML_IMPORT_PATH=str(module), QT_QPA_PLATFORM=args.platform,
                               QT_QUICK_BACKEND="software", QT_SCALE_FACTOR=str(scale),
                               RESINA_COMMAND_CASES=str(case_file.resolve()),
                               RESINA_COMMAND_FONT=font.as_uri(), RESINA_COMMAND_OUTPUT=str(output.resolve()))
            environment["RESINA_COMMAND_ALTERNATE_FONT"] = alternate_font.as_uri()
            try:
                result = subprocess.run([str(host), "--no-color", "--path", str((output / "config.qml").resolve())],
                                        env=environment, capture_output=True, text=True, timeout=10 * len(cases))
            except subprocess.TimeoutExpired as error:
                partial = "".join(stream.decode("utf-8", errors="replace") if isinstance(stream, bytes)
                                  else stream or "" for stream in (error.stdout, error.stderr))
                (output / "run.log").write_text(partial, encoding="utf-8")
                raise
            log = result.stdout + result.stderr
            (output / "run.log").write_text(log, encoding="utf-8")
            if result.returncode:
                raise ValueError(f"native host failed with exit {result.returncode}: {log[-2000:]}")
            reports = read_reports(log, cases)
            for case, report in zip(cases, reports):
                verify_case(case, report, scale)
                if not case.get("reject"):
                    verify_capture(output / (case["name"] + ".png"), json.loads(report["snapshot"]), scale,
                                   case["request"]["body"]["surroundingColor"])
            verify_stable_targets(cases, reports)
            (output / "observations.json").write_text(json.dumps(reports, indent=2), encoding="utf-8")
            print(f"PASS {len(cases)} authored Command cases at device scale {scale}", flush=True)
    except (OSError, ValueError, KeyError, TypeError, ValidationError, subprocess.TimeoutExpired) as error:
        print("FAIL Quickshell Command updates: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
