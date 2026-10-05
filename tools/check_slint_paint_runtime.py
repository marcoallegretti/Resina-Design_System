import argparse
import io
import json
import math
import os
from pathlib import Path
import re
import subprocess
import sys

from PIL import Image


def compare_images(expected, actual):
    if expected.size != actual.size:
        raise ValueError(f"capture dimensions {actual.size} differ from {expected.size}")
    expected, actual = expected.convert("RGBA"), actual.convert("RGBA")
    for y in range(expected.height):
        for x in range(expected.width):
            reference, pixel = expected.getpixel((x, y)), actual.getpixel((x, y))
            if reference[3] != pixel[3] or any(abs(a - b) > 1 for a, b in zip(reference[:3], pixel[:3])):
                raise ValueError(f"composited paint differs at ({x}, {y}): {reference}, {pixel}")


def run(command, source=None, environment=None):
    result = subprocess.run(command, input=source, env=environment,
                            capture_output=True, timeout=30)
    if result.returncode or result.stderr:
        raise ValueError(f"{command[0]} exited {result.returncode}: "
                         f"{result.stderr.decode(errors='replace')[-2000:]}")
    return result.stdout


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--backend", required=True)
    parser.add_argument("--raster", required=True)
    parser.add_argument("--resolver", required=True)
    parser.add_argument("--viewer", required=True)
    parser.add_argument("--scenes", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--scales", nargs="+", type=float, default=[0.5, 1.0, 1.25, 2.0])
    args = parser.parse_args()
    try:
        version = run([args.viewer, "--version"]).decode().strip()
        if version != "slint-viewer 1.18.1":
            raise ValueError(f"expected Slint viewer 1.18.1, got {version!r}")
        catalog = json.loads(args.scenes.read_text(encoding="utf-8"))
        scenes = [s for s in catalog["scenarios"] if s["kind"] == "surfacePaint"]
        if len(scenes) != 16 or len({s["name"] for s in scenes}) != 16:
            raise ValueError("expected the 16 distinct public complete material paint scenes")
        if not args.scales or any(scale not in [0.5, 1.0, 1.25, 2.0, 3.0, 4.0] for scale in args.scales):
            raise ValueError("unsupported probe scale")
        args.output_dir.mkdir(parents=True, exist_ok=False)
        count = 0
        for scene in scenes:
            source = json.dumps(scene["request"]).encode()
            ir = json.loads(run([args.resolver, "-"], source))
            if "focus" in ir:
                outer = ir["focus"]["geometry"]["outer"]
                bounds = outer["contour"]["bounds"].copy()
                bounds["x"] += outer["offset"]["x"]
                bounds["y"] += outer["offset"]["y"]
            else:
                bounds = ir["body"]["geometry"]["silhouette"]["bounds"]
            color = scene["request"]["surroundingColor"]
            if color["colorSpace"] != "srgb" or color["alpha"] != 1:
                raise ValueError("probe requires an authored opaque sRGB surrounding color")
            background = tuple(math.floor(channel * 255 + 0.5) for channel in color["components"])
            for scale in args.scales:
                stem = f"paint-{count}"
                component = args.output_dir / f"{stem}.slint"
                emitted = run([args.backend, "-", str(scale), "4", "44.3", "26.1"], source)
                component.write_bytes(emitted)
                left = math.floor((bounds["x"] + 44.3) * scale)
                top = math.floor((bounds["y"] + 26.1) * scale)
                width = math.ceil((bounds["x"] + bounds["width"] + 44.3) * scale) - left
                height = math.ceil((bounds["y"] + bounds["height"] + 26.1) * scale) - top
                for name, value in [("paint-origin-x", left / scale), ("paint-origin-y", top / scale)]:
                    match = re.search(r"out property <length> " + name + r": ([^\s;]+)px;", emitted.decode())
                    if not match or float(match.group(1)) != value:
                        raise ValueError(f"incorrect prepared profile: {name}")
                match = re.search(r"out property <float> prepared-device-scale: ([^;]+);", emitted.decode())
                if not match or float(match.group(1)) != scale:
                    raise ValueError("incorrect prepared device scale")
                expected_png = run([args.raster, "-", str(width), str(height),
                                    str(left / scale - 44.3), str(top / scale - 26.1),
                                    str(scale), "4"], source)
                expected = Image.new("RGBA", (round(320 * scale), round(160 * scale)), (*background, 255))
                with Image.open(io.BytesIO(expected_png)) as paint:
                    expected.alpha_composite(paint.convert("RGBA"), (left, top))
                environment = os.environ.copy()
                environment.pop("SLINT_BACKEND", None)
                environment["SLINT_SCALE_FACTOR"] = str(scale)
                for resized in [False, True] if count == 0 else [False]:
                    suffix = "-resized" if resized else ""
                    wrapper = args.output_dir / f"{stem}{suffix}-probe.slint"
                    image = args.output_dir / f"{stem}{suffix}.png"
                    resize = "width: 300px; height: 140px;" if resized else ""
                    wrapper.write_text(
                        f'import {{ ResinaSurfacePaint }} from "{component.name}";\n'
                        'export component PaintProbe inherits Window {\n'
                        f'    width: 320px; height: 160px; background: rgb{background};\n'
                        '    ResinaSurfacePaint {\n'
                        f'        x: self.paint-origin-x; y: self.paint-origin-y; {resize}\n'
                        '    }\n}\n', encoding="utf-8")
                    run([args.viewer, "--screenshot", str(image.resolve()), str(wrapper.resolve())],
                        environment=environment)
                    with Image.open(image) as actual:
                        compare_images(expected, actual)
                count += 1
        print(f"PASS: {count} Slint software material frames and one resized-container capture")
    except (OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
        print(f"Slint paint runtime check failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
