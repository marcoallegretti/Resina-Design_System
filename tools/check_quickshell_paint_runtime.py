import argparse
import io
import json
import math
import os
import re
from pathlib import Path
import subprocess
import sys

from PIL import Image


def compare_images(expected, actual):
    if expected.size != actual.size:
        raise ValueError(f"capture dimensions {actual.size} differ from {expected.size}")
    expected, actual = expected.convert("RGBA"), actual.convert("RGBA")
    for y in range(expected.height):
        for x in range(expected.width):
            ref, pixel = expected.getpixel((x, y)), actual.getpixel((x, y))
            if abs(ref[3] - pixel[3]) > 1:
                raise ValueError(f"alpha differs at ({x}, {y}): {ref}, {pixel}")
            for channel in range(3):
                if ref[3] == 255 and abs(ref[channel] - pixel[channel]) > 1:
                    raise ValueError(f"opaque pigment differs at ({x}, {y}): {ref}, {pixel}")
                ref_premultiplied = round(ref[channel] * ref[3] / 255)
                actual_premultiplied = round(pixel[channel] * pixel[3] / 255)
                if abs(ref_premultiplied - actual_premultiplied) > 1:
                    raise ValueError(f"premultiplied pigment differs at ({x}, {y}): {ref}, {pixel}")


def read_backend_output(command, source):
    result = subprocess.run(command, input=source, capture_output=True, timeout=30)
    if result.returncode or result.stderr:
        raise ValueError(f"paint command failed: {result.stderr.decode(errors='replace')[-2000:]}")
    return result.stdout


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--backend", required=True)
    parser.add_argument("--raster", required=True)
    parser.add_argument("--resolver", required=True)
    parser.add_argument("--scenes", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--platform", choices=["offscreen", "wayland"], default="offscreen")
    parser.add_argument("--scene-graph", choices=["software", "rhi"], default="software")
    parser.add_argument("--scales", nargs="+", type=float, default=[1.0, 1.25, 2.0])
    args = parser.parse_args()
    try:
        catalog = json.loads(args.scenes.read_text(encoding="utf-8"))
        scenes = [s for s in catalog["scenarios"] if s["kind"] == "surfacePaint"]
        if len(scenes) != 16 or len({s["name"] for s in scenes}) != 16:
            raise ValueError("expected the 16 distinct public complete material paint scenes")
        if any(scale not in [0.5, 1.0, 1.25, 2.0, 3.0, 4.0] for scale in args.scales):
            raise ValueError("unsupported probe scale")
        args.output_dir.mkdir(parents=True, exist_ok=False)
        root = Path(__file__).resolve().parent.parent
        count = 0
        for scene in scenes:
            source = json.dumps(scene["request"]).encode()
            ir = json.loads(read_backend_output([args.resolver, "-"], source))
            if "focus" in ir:
                outer = ir["focus"]["geometry"]["outer"]
                bounds = outer["contour"]["bounds"].copy()
                bounds["x"] += outer["offset"]["x"]
                bounds["y"] += outer["offset"]["y"]
            else:
                bounds = ir["body"]["geometry"]["silhouette"]["bounds"]
            for scale in args.scales:
                stem = f"paint-{count}"
                qml = args.output_dir / f"{stem}.qml"
                image_path = args.output_dir / f"{stem}.png"
                qml.write_bytes(read_backend_output([args.backend, "-", str(scale), "4", "44.3", "26.1"], source))
                left = math.floor((bounds["x"] + 44.3) * scale)
                top = math.floor((bounds["y"] + 26.1) * scale)
                width = math.ceil((bounds["x"] + bounds["width"] + 44.3) * scale) - left
                height = math.ceil((bounds["y"] + bounds["height"] + 26.1) * scale) - top
                text = qml.read_text(encoding="utf-8")
                for name, value in [("paintOriginX", left / scale), ("paintOriginY", top / scale), ("preparedDeviceScale", scale)]:
                    match = re.search(r"readonly property real " + name + r": ([^\n]+)", text)
                    if not match or float(match.group(1)) != value:
                        raise ValueError(f"incorrect prepared profile: {name}")
                expected_png = read_backend_output([args.raster, "-", str(width), str(height), str(left / scale - 44.3), str(top / scale - 26.1), str(scale), "4"], source)
                expected = Image.new("RGBA", (round(320 * scale), round(160 * scale)))
                with Image.open(io.BytesIO(expected_png)) as prepared:
                    expected.paste(prepared, (left, top))
                environment = os.environ.copy()
                environment.update(QT_QPA_PLATFORM=args.platform, QT_QUICK_BACKEND=args.scene_graph,
                                   QSG_RHI_BACKEND="opengl", QT_SCALE_FACTOR=str(scale),
                                   RESINA_PAINT_QML=qml.resolve().as_uri(), RESINA_PAINT_IMAGE=str(image_path.resolve()))
                result = subprocess.run(["quickshell", "--no-color", "--path", str(root / "conformance/quickshell/paint-probe.qml")], env=environment, capture_output=True, timeout=30)
                log = (result.stdout + result.stderr).decode(errors="replace")
                if result.returncode or "RESINA_PAINT_PASS" not in log or "RESINA_PAINT_FAIL" in log or "ERROR" in log:
                    raise ValueError(f"native {scene['name']} scale {scale} failed: {log[-2000:]}")
                with Image.open(image_path) as actual:
                    if actual.format != "PNG":
                        raise ValueError("native capture is not PNG")
                    compare_images(expected, actual)
                count += 1
                print(f"PASS {scene['name']} scale {scale}", flush=True)
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        print(f"FAIL Quickshell paint runtime: {error}", file=sys.stderr)
        return 1
    print(f"Quickshell complete paint passed {count} native captures ({args.platform}, {args.scene_graph})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
