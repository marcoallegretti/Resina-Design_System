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


def verify_case(case, report, scale):
    fields = {"name", "ready", "error", "snapshot", "target", "state", "hovered",
              "ignored", "deviceScale", "renderer", "publications", "measurements"}
    if set(report) != fields or type(report["deviceScale"]) not in (int, float) or report["deviceScale"] != scale:
        raise ValueError("native command report fields or actual device scale differ")
    if report["renderer"] != "software":
        raise ValueError("native command did not use the requested software renderer")
    if not all(isinstance(report[field], str) for field in ("error", "snapshot", "target")) \
            or not all(type(report[field]) is bool for field in ("ready", "hovered", "ignored")) \
            or not isinstance(report["publications"], list):
        raise ValueError("native command report types differ")
    validator_for("schemas/activation-state.schema.json").validate(report["state"])
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
    if state != {"schemaVersion": "0.1.0", "enabled": enabled, "focused": False, "hold": None} \
            or report["hovered"]:
        raise ValueError("authored update changed the expected native interaction state")
    request = copy.deepcopy(case["request"])
    request.update(activation=state, hovered=False, focusable=enabled)
    states = {"schemaVersion": "0.1.0", "states": ["rest"] if enabled else ["disabled"]}
    snapshot = json.loads(report["snapshot"], object_pairs_hook=unique_object)
    verify_snapshot(snapshot, request, states)
    verify_authored_paint(request, snapshot)
    verify_typography(case, snapshot["label"])
    verify_layout(snapshot["label"], report["measurements"], request["content"])
    if json.loads(report["target"], object_pairs_hook=unique_object) != snapshot["hitRegion"]:
        raise ValueError("native target and published snapshot disagree")
    for publication in report["publications"]:
        if set(publication) != {"configuration", "snapshot", "state", "hovered"}:
            raise ValueError("native publication observation is incomplete")
        source = json.loads(publication["configuration"], object_pairs_hook=unique_object)
        if source != case["request"] or publication["snapshot"]["label"]["text"] != source["content"]["text"]:
            raise ValueError("consumer observed a mixed authored generation")
        validator_for("schemas/activation-state.schema.json").validate(publication["state"])
        if type(publication["hovered"]) is not bool:
            raise ValueError("publication hover observation is not Boolean")
        source.update(activation=publication["state"], hovered=publication["hovered"],
                      focusable=publication["state"]["enabled"])
        flags = []
        if publication["hovered"]:
            flags.append("hover")
        if publication["state"]["focused"]:
            flags.append("focused")
        if not publication["state"]["enabled"]:
            flags.append("disabled")
        if publication["state"]["hold"] is not None:
            raise ValueError("authored update acquired an unexpected hold")
        verify_snapshot(publication["snapshot"], source,
                        {"schemaVersion": "0.1.0", "states": flags or ["rest"]})
        verify_authored_paint(source, publication["snapshot"])
        verify_typography(case, publication["snapshot"]["label"])


def verify_capture(path, snapshot, scale):
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
            result = subprocess.run([str(host), "--no-color", "--path", str((output / "config.qml").resolve())],
                                    env=environment, capture_output=True, text=True, timeout=10 * len(cases))
            log = result.stdout + result.stderr
            (output / "run.log").write_text(log, encoding="utf-8")
            if result.returncode:
                raise ValueError(f"native host failed with exit {result.returncode}: {log[-2000:]}")
            reports = read_reports(log, cases)
            for case, report in zip(cases, reports):
                verify_case(case, report, scale)
                if not case.get("reject"):
                    verify_capture(output / (case["name"] + ".png"), json.loads(report["snapshot"]), scale)
            verify_stable_targets(cases, reports)
            (output / "observations.json").write_text(json.dumps(reports, indent=2), encoding="utf-8")
            print(f"PASS {len(cases)} authored Command cases at device scale {scale}", flush=True)
    except (OSError, ValueError, KeyError, TypeError, ValidationError, subprocess.TimeoutExpired) as error:
        print("FAIL Quickshell Command updates: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
