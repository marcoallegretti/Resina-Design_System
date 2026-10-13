"""Compare prepared relief, content or focus pixels with an owned nested Wayland capture."""
import argparse
import copy
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import xml.etree.ElementTree as ET

from PIL import Image
from check_schemas import ROOT, load_json


def diagnostic_request():
    request = copy.deepcopy(load_json(ROOT / "conformance/ir/relief-paint-vectors.json")[7]["request"])
    request["outline"]["radii"] = {
        "topStart": {"x": 20, "y": 12}, "topEnd": {"x": 30, "y": 15},
        "bottomEnd": {"x": 24, "y": 18}, "bottomStart": {"x": 12, "y": 20},
    }
    request["profile"].update(amplitude=0.25, shoulderWidth=1)
    request.update(edgeWidth=0.25, points=[])
    return request


def diagnostic_outline_focus_request():
    from check_outline_focus_backend import case_request
    case = load_json(ROOT / "conformance/states/outline-focus-cases.json")[0]
    request, _, _ = case_request(case)
    request["directions"] = []
    request["outline"]["layoutDirection"] = "rtl"
    request["sideOffset"] = {"x": -6, "y": 4}
    return request


def diagnostic_content_focus_request(paint):
    from check_focus_indicator_backend import baseline, contrast
    focus, _ = baseline()
    family = paint["pigment"]["materialFamily"]
    assignments = focus["scenario"]["resolution"]["materialAssignments"]
    role = next((f"{namespace}.{name}" for namespace in ("surface", "control", "feedback")
                 for name, value in assignments[namespace].items() if value == family), None)
    if role is None:
        raise ValueError(f"no authored focus material role for {family!r}")
    focus["scenario"]["surface"]["materialRole"] = role
    focus["scenario"]["surface"]["treatmentStack"]["treatments"] = ["none"]
    focus["scenario"]["surface"]["states"]["states"] = ["selected", "focused", "hover"]
    focus["surroundingColor"] = copy.deepcopy(paint["edge"]["adjacentColor"])
    resolution = focus["scenario"]["resolution"]
    token = resolution["opaqueColorAssignments"]["roles"]["focus"].split(".")[1]
    components = [0, 0, 0] if contrast([0, 0, 0], focus["surroundingColor"]["components"]) >= 3 else [1, 1, 1]
    resolution["tokens"]["palette"][token]["$value"] = {
        "colorSpace": "srgb", "components": components, "alpha": 1}
    return {"schemaVersion": "0.1.0", "focus": focus, "outline": copy.deepcopy(paint["outline"]),
            "sideOffset": {"x": 0, "y": 0}, "directions": []}


def capture_matches(capture, prepared, origin):
    if origin[0] < 0 or origin[1] < 0 or origin[0] + prepared.width > capture.width or origin[1] + prepared.height > capture.height:
        raise ValueError("native capture clips the complete prepared image")
    count, worst = 0, 0
    for y in range(prepared.height):
        for x in range(prepared.width):
            expected = prepared.getpixel((x, y))
            if expected[3] != 255:
                continue
            actual = capture.getpixel((origin[0] + x, origin[1] + y))
            worst = max(worst, *(abs(expected[c] - actual[c]) for c in range(3)))
            count += 1
    if not count:
        raise ValueError("prepared image has no opaque verification pixels")
    return count, worst


def validate_canvas_reference(prepared, logical_size, scale):
    expected = tuple(round(dimension * scale) for dimension in logical_size)
    if prepared.size != expected:
        raise ValueError("native opaque-canvas GPU reference dimensions disagree")
    if prepared.getchannel("A").getextrema() != (255, 255):
        raise ValueError("native GPU reference must have an opaque canvas")


def board_dimensions(prepared, scale):
    width, height = prepared.size
    if width * height > 16_777_216 or width / scale != round(width / scale) or height / scale != round(height / scale):
        raise ValueError("native board must fit integral logical dimensions within 16 megapixels")
    logical = (round(width / scale), round(height / scale))
    validate_canvas_reference(prepared, logical, scale)
    return logical


def check_scale(probe, request, output, parent, scale, content=None, outline_focus=False, content_focus=None, board=None):
    preparation_seconds = 300 if board else 120 if content or outline_focus else 20
    with tempfile.TemporaryDirectory(prefix="resina-relief-native-") as runtime:
        config = Path(runtime) / "config"
        output_width = 1260 if content else 840
        output_height = 840
        env = dict(os.environ, XDG_RUNTIME_DIR=runtime, WAYLAND_DISPLAY=str(parent),
                   WLR_BACKENDS="wayland", WLR_RENDERER="pixman", WLR_LIBINPUT_NO_DEVICES="1")
        if content:
            fonts = Path(runtime) / "fonts"
            fonts.mkdir()
            fontconfig = Path(runtime) / "fonts.conf"
            root = ET.Element("fontconfig")
            ET.SubElement(root, "dir").text = str(fonts)
            ET.ElementTree(root).write(fontconfig, encoding="utf-8", xml_declaration=True)
            env["FONTCONFIG_FILE"] = str(fontconfig)
        logical_board = None
        if board:
            font, text_scale = content
            layout_path = output / f"layout-{scale}.png"
            preflight = subprocess.run([str(probe), "relief-board", str(request), str(font), "DejaVu Sans", str(layout_path), str(scale), str(text_scale), *board], env=env, cwd=ROOT, capture_output=True, text=True, timeout=preparation_seconds)
            if preflight.returncode != 0 or preflight.stderr:
                raise ValueError(f"board preflight failed: {preflight.stderr[:400]}")
            layout = Image.open(layout_path).convert("RGBA")
            logical_board = board_dimensions(layout, scale)
            output_width, output_height = layout.size
        config.write_text(f"xwayland disable\noutput * mode {output_width}x{output_height} scale {scale}\nseat seat0 xcursor_theme default 24\n")
        children = []
        client_path = output / f"client-{scale}.log"
        with (output / f"compositor-{scale}.log").open("w") as compositor_log, client_path.open("w") as client_log:
            try:
                compositor = subprocess.Popen(["sway", "--config", str(config)], env=env,
                                              stdout=compositor_log, stderr=compositor_log)
                children.append(compositor)
                socket, ipc = None, None
                for _ in range(200):
                    sockets = [p for p in Path(runtime).glob("wayland-*") if p.is_socket()]
                    peers = list(Path(runtime).glob("sway-ipc.*.sock"))
                    if sockets and peers:
                        socket, ipc = sockets[0], peers[0]
                        break
                    if compositor.poll() is not None:
                        raise ValueError("nested compositor exited; inspect its saved log")
                    time.sleep(0.05)
                if socket is None or ipc is None:
                    raise ValueError("nested compositor did not publish its sockets")
                env.update(WAYLAND_DISPLAY=str(socket), SWAYSOCK=str(ipc))
                outputs = json.loads(subprocess.check_output(
                    ["swaymsg", "--socket", str(ipc), "-t", "get_outputs"], env=env, text=True))
                if len(outputs) != 1 or outputs[0]["scale"] != scale:
                    raise ValueError("compositor did not adopt the requested single-output scale")
                mode = outputs[0]["current_mode"]
                if (mode["width"], mode["height"]) != (output_width, output_height):
                    raise ValueError("compositor did not adopt the required physical output size")
                prepared_path = output / f"prepared-{scale}.png"
                arguments = [str(probe), str(request), str(scale), str(prepared_path)]
                marker = "RESINA_RELIEF_PREPARED "
                if board:
                    font, text_scale = content
                    arguments = [str(probe), "relief-board-native", str(request), str(font), "DejaVu Sans", str(prepared_path), str(scale), str(text_scale), *board]
                    marker = "RESINA_RELIEF_BOARD_PREPARED "
                elif content:
                    font, text_scale = content
                    arguments = [str(probe), str(request), str(font), str(scale), str(text_scale), str(prepared_path)]
                    if content_focus:
                        arguments.append(str(content_focus))
                    marker = "RESINA_RELIEF_CONTENT_PREPARED "
                elif outline_focus:
                    marker = "RESINA_OUTLINE_FOCUS_PREPARED "
                client = subprocess.Popen(arguments,
                                          cwd=ROOT, env=env, stdout=client_log, stderr=client_log)
                children.append(client)
                metadata = None
                deadline = time.monotonic() + preparation_seconds
                while time.monotonic() < deadline:
                    for line in client_path.read_text().splitlines():
                        if line.startswith(marker):
                            metadata = json.loads(line.split(" ", 1)[1])
                    if metadata is not None and prepared_path.exists():
                        break
                    if client.poll() is not None:
                        raise ValueError("native client exited; inspect its saved log")
                    time.sleep(0.05)
                if metadata is None or not prepared_path.exists():
                    raise ValueError(f"native client did not publish prepared paint within {preparation_seconds} seconds")
                if metadata["scale"] != scale:
                    raise ValueError("native fixture preparation scale differs from compositor scale")
                if content_focus and metadata.get("focusComposed") is not True:
                    raise ValueError("native fixture did not prepare the requested focus composition")
                if board and (metadata.get("reliefKind"), metadata.get("reliefState"), metadata.get("textScale"), metadata.get("logicalWidth"), metadata.get("logicalHeight")) != (*board, content[1], *logical_board):
                    raise ValueError("native board metadata differs from its requested layout/state")
                prepared = Image.open(prepared_path).convert("RGBA")
                if board:
                    validate_canvas_reference(prepared, logical_board, scale)
                    origin = (0, 0)
                elif content or outline_focus:
                    validate_canvas_reference(prepared, (360, 200) if content else (160, 96), scale)
                    origin = (0, 0)
                else:
                    origin = tuple(round(metadata["origin"][axis] * scale) for axis in ("x", "y"))
                capture_path = output / f"native-{scale}.png"
                for _ in range(100):
                    subprocess.run(["grim", "-o", outputs[0]["name"], str(capture_path)], env=env,
                                   check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
                    capture = Image.open(capture_path).convert("RGBA")
                    if capture.size != (mode["width"], mode["height"]):
                        raise ValueError("native capture resamples the physical output; pixel comparison is unsupported")
                    count, worst = capture_matches(capture, prepared, origin)
                    if worst <= 2:
                        return {"scale": scale, "output": outputs[0]["name"], "comparedOpaquePixels": count,
                                "worstOpaqueDifference": worst,
                                "captureSha256": hashlib.sha256(capture_path.read_bytes()).hexdigest()}
                    if client.poll() is not None:
                        raise ValueError("native client exited before matching captured pixels")
                    time.sleep(0.1)
                raise ValueError(f"scale {scale}: opaque native pixels differ by {worst} (limit 2)")
            finally:
                for child in reversed(children):
                    child.terminate()
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        child.kill()
                        child.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--request", type=Path)
    parser.add_argument("--content-font", type=Path, help="use the complete-content probe with an explicit regular DejaVu Sans file")
    parser.add_argument("--outline-focus", action="store_true", help="check the continuous-outline focus probe against its full opaque GPU reference")
    parser.add_argument("--content-focus", action="store_true", help="compose a guarded focus ring around the actual complete-content footprint")
    parser.add_argument("--relief-board", choices=["flat", "convex", "recessed"], help="check the complete themed Relief Board using the Lab binary")
    parser.add_argument("--board-state", choices=["rest", "focused"], default="rest")
    parser.add_argument("--text-scale", type=float, default=1)
    parser.add_argument("--scales", nargs="+", type=float, default=[1, 1.25, 1.5, 2, 3])
    args = parser.parse_args()
    if args.relief_board and (not args.content_font or not args.request or args.content_focus or args.outline_focus):
        parser.error("relief board requires --content-font, --request and no other probe mode")
    if not args.relief_board and args.board_state != "rest":
        parser.error("board state requires --relief-board")
    if args.content_focus and not args.content_font:
        parser.error("content focus requires --content-font and the complete-content probe")
    if args.outline_focus and (args.content_font or any(
            not math.isfinite(s) or not 1 <= s <= 3
            or 160 * s != round(160 * s) or 96 * s != round(96 * s) for s in args.scales)):
        parser.error("outline focus requires scales [1,3] with integral 160x96 pixels and no content font")
    if not args.content_font and args.text_scale != 1:
        parser.error("text scale requires the complete-content probe and --content-font")
    if any(not math.isfinite(s) or s <= 0 or (not args.relief_board and (840 / s) != round(840 / s)) for s in args.scales):
        parser.error("each positive scale must divide the 840 px output into integral logical dimensions")
    if args.content_font and (not math.isfinite(args.text_scale) or not 1 <= args.text_scale <= (3 if args.relief_board else 2) or any(s < 1 or s > 3 or (not args.relief_board and (1260 / s) != round(1260 / s)) for s in args.scales)):
        parser.error(f"content requires device scales [1,3], integral output dimensions and text scale [1,{3 if args.relief_board else 2}]")
    try:
        parent = Path(os.environ["WAYLAND_DISPLAY"])
        if not parent.is_absolute():
            parent = Path(os.environ["XDG_RUNTIME_DIR"]) / parent
        if not parent.is_socket():
            raise ValueError("parent Wayland socket is absent")
        args.output_dir.mkdir(parents=True)
        request_path = args.output_dir / "request.json"
        request = load_json(args.request) if args.request else (
            diagnostic_outline_focus_request() if args.outline_focus else diagnostic_request())
        if args.content_font and not args.request:
            request["outline"]["size"] = {"width": 320, "height": 160}
            request["outline"]["layoutDirection"] = "rtl"
        request_path.write_text(json.dumps(request, allow_nan=False))
        content = (args.content_font.resolve(), args.text_scale) if args.content_font else None
        content_focus = None
        if args.content_focus:
            content_focus = (args.output_dir / "focus.json").resolve()
            content_focus.write_text(json.dumps(diagnostic_content_focus_request(request), allow_nan=False))
        board = (args.relief_board, args.board_state) if args.relief_board else None
        records = [check_scale(args.probe.resolve(), request_path.resolve(), args.output_dir.resolve(), parent, scale, content, args.outline_focus, content_focus, board)
                   for scale in args.scales]
        report = {"records": records, "nestedBackend": "wayland", "renderer": "pixman",
                  "scope": "opaque native face-paint pixels; no appearance, input, focus or AT acceptance"}
        if content:
            report["scope"] = "complete native face and glyph pixels against GPU reference; no appearance, input, focus or AT acceptance"
            report["textScale"] = args.text_scale
            report["fontSha256"] = hashlib.sha256(args.content_font.read_bytes()).hexdigest()
            if content_focus:
                report["scope"] = "complete native relief, glyph and focus pixels against GPU reference; no component appearance, input or AT acceptance"
                report["focusRequestSha256"] = hashlib.sha256(content_focus.read_bytes()).hexdigest()
            if board:
                report["scope"] = "complete native themed Relief Board against GPU reference; no live input or AT acceptance"
                report["reliefKind"], report["reliefState"] = board
        elif args.outline_focus:
            report["scope"] = "complete native continuous-outline focus pixels against GPU reference; no component appearance, input or AT acceptance"
        (args.output_dir / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report))
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"FAIL native paint: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
