import argparse
import hashlib
import math
import os
from pathlib import Path
import re
import subprocess
import sys

from PIL import Image, ImageChops


def check_image(path, scale, overhang=False):
    width, height = round(400 * scale), round(640 * scale)
    with Image.open(path) as source:
        if source.format != "PNG" or source.size != (width, height):
            raise ValueError("native label capture format or dimensions differ")
        image = source.convert("RGBA")
    if image.getchannel("A").getextrema() != (255, 255):
        raise ValueError("native label capture must be opaque")
    actual = image.convert("RGB")
    reference_path = path.with_name(path.stem + "-reference.png")
    with Image.open(reference_path) as source:
        if source.format != "PNG" or source.size != image.size:
            raise ValueError("native reference capture format or dimensions differ")
        reference_rgba = source.convert("RGBA")
    if reference_rgba.getchannel("A").getextrema() != (255, 255):
        raise ValueError("native reference capture must be opaque")
    reference = reference_rgba.convert("RGB")
    difference = ImageChops.difference(actual, reference)
    if any(maximum > 1 for _, maximum in difference.getextrema()):
        raise ValueError("native label differs from complete Qt reference")
    ink = ImageChops.difference(actual, Image.new("RGB", actual.size, "white"))
    if not ink.getbbox() or (overhang and not ink.crop((0, 0, round(20 * scale), height)).getbbox()):
        raise ValueError("native capture has no complete ink or required glyph overhang")
    return hashlib.sha256(image.tobytes()).digest()


def check_probe_output(result, scale):
    log = result.stdout + result.stderr
    matches = re.findall(r"RESINA_LABEL_RENDER_PASS 25 9 ([0-9.]+)(?:\s|$)", log)
    if result.returncode or "RESINA_LABEL_RENDER_FAIL" in log or len(matches) != 1 \
        or not math.isclose(float(matches[0]), scale, rel_tol=0, abs_tol=0.00001):
        raise ValueError(f"native drawing probe failed (exit {result.returncode}): {log[-2000:]}")


def main():
    parser = argparse.ArgumentParser(description="Verify complete native Qt label drawing against independent Text items.")
    parser.add_argument("--font", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--quickshell", default="quickshell")
    parser.add_argument("--platform", choices=("offscreen", "wayland"), default="offscreen")
    parser.add_argument("--scene-graph", choices=("software", "rhi"), default="software")
    parser.add_argument("--scales", nargs="+", type=float, default=[1, 1.25, 2, 1.3])
    arguments = parser.parse_args()
    if not arguments.font.is_file():
        parser.error("provide the DejaVu Sans font file")
    for scale in arguments.scales:
        if not math.isfinite(scale) or scale <= 0 or scale > 4:
            parser.error("device scale must be positive and capture must remain within 4194304 pixels")
        if 400 * scale != round(400 * scale) or 640 * scale != round(640 * scale):
            parser.error("device scale must produce integral capture dimensions")
    root = Path(__file__).resolve().parent.parent
    environment = os.environ.copy()
    environment.update(QT_QPA_PLATFORM=arguments.platform, QT_QUICK_BACKEND=arguments.scene_graph,
        QSG_RHI_BACKEND="opengl",
        RESINA_LABEL_FONT=arguments.font.resolve().as_uri(),
        RESINA_LABEL_MEASURE_QML=(root / "backends/quickshell/resina-qml/qml/ResinaLabelMeasure.qml").as_uri(),
        RESINA_LABEL_QML=(root / "backends/quickshell/resina-qml/qml/ResinaLabel.qml").as_uri())
    try:
        arguments.output_dir.mkdir(parents=True, exist_ok=False)
        for index, scale in enumerate(arguments.scales):
            directory = arguments.output_dir / str(index)
            directory.mkdir()
            environment.update(QT_SCALE_FACTOR=str(scale), RESINA_LABEL_DEVICE_SCALE=str(scale),
                RESINA_LABEL_CAPTURE_DIR=str(directory.resolve()))
            result = subprocess.run([arguments.quickshell, "--no-color", "--path",
                str(root / "conformance/quickshell/label-render-probe.qml")], env=environment,
                capture_output=True, encoding="utf-8", errors="strict", timeout=60, check=False)
            check_probe_output(result, scale)
            hashes = {check_image(directory / f"frame-{frame}.png", scale, frame == 0) for frame in range(25)}
            if len(hashes) != 25:
                raise ValueError("native drawing reused a stale or duplicate capture")
    except (OSError, UnicodeError, ValueError, subprocess.TimeoutExpired) as error:
        print(f"FAIL Quickshell label drawing: {error}", file=sys.stderr)
        return 1
    print(f"Quickshell native label drawing passed {25 * len(arguments.scales)} complete capture pairs and {9 * len(arguments.scales)} rejection cases on {arguments.platform} ({arguments.scene_graph})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
