"""Check material-relief QML placement and embedded paint against independent arithmetic."""
import argparse
import copy
import json
import math
import os
from pathlib import Path
import re
import subprocess
import tempfile
from types import SimpleNamespace

from PIL import Image

from check_material_relief_paint_backend import source_cases
from check_material_relief_raster_backend import check_png
from check_schemas import ROOT, load_json
from check_quickshell_paint_runtime import compare_images
from material_relief_arithmetic import rectangular_pixels


def component_png(result, width, height, placement, scale, expected):
    if result.returncode or result.stderr:
        raise ValueError("material-relief QML command failed")
    text = result.stdout.decode("utf-8")
    left, top = (math.floor(value * scale) for value in placement)
    values = {
        "paintOriginX": left / scale,
        "paintOriginY": top / scale,
        "preparedDeviceScale": scale,
    }
    for name, value in values.items():
        matches = re.findall(r"^\s*readonly property real " + name + r": ([^\n]+)$", text, re.MULTILINE)
        if len(matches) != 1 or float(matches[0]) != value:
            raise ValueError(f"incorrect or repeated native profile: {name}")
    for name, value in (("implicitWidth", width / scale), ("implicitHeight", height / scale)):
        matches = re.findall(r"^\s*" + name + r": ([^\n]+)$", text, re.MULTILINE)
        if len(matches) != 1 or float(matches[0]) != value:
            raise ValueError(f"incorrect native extent: {name}")
    for name, value in (("width", width), ("height", height)):
        matches = re.findall(r"^\s*" + name + r": ([^\n]+)$", text, re.MULTILINE)
        if len(matches) != 1 or int(matches[0]) != value:
            raise ValueError(f"incorrect image-node extent: {name}")
    transforms = re.findall(r"transform: Scale \{ xScale: ([^;]+); yScale: ([^}]+) \}", text)
    if len(transforms) != 1 or any(float(value) != 1 / scale for value in transforms[0]):
        raise ValueError("incorrect image-node inverse-density transform")
    for marker in ("focus: false", "activeFocusOnTab: false", "smooth: false", "mipmap: false", "asynchronous: false", "paint.status === Image.Ready", "paint.status === Image.Error"):
        if marker not in text:
            raise ValueError(f"missing native paint profile: {marker}")
    if text.count("Accessible.ignored: true") != 2:
        raise ValueError("paint must remain decorative")
    images = re.findall(r'source: "data:image/png,((?:%[0-9A-F]{2})+)"', text)
    if len(images) != 1 or text.count("data:image/png,") != 1:
        raise ValueError("expected one complete encoded paint image")
    png = bytes.fromhex(images[0].replace("%", ""))
    worst = check_png(SimpleNamespace(returncode=0, stderr=b"", stdout=png), width, height, expected)
    return png, worst


def invoke(command, source, options):
    return subprocess.run([*command, *options], input=source, capture_output=True, timeout=60, check=False)


def verify(command, source, request, scale, samples, native_output=None):
    placement = (13.3, 17.1)
    size = request["outline"]["size"]
    left, top = (math.floor(value * scale) for value in placement)
    width = math.ceil((placement[0] + size["width"]) * scale) - left
    height = math.ceil((placement[1] + size["height"]) * scale) - top
    origin = (left / scale - placement[0], top / scale - placement[1])
    expected = rectangular_pixels(request, width, height, origin, scale, samples)
    options = ["-", str(scale), str(samples), *(str(value) for value in placement)]
    result = invoke(command, source, options)
    _, worst = component_png(result, width, height, placement, scale, expected)
    repeated = invoke(command, source, options)
    if repeated.returncode or repeated.stderr or repeated.stdout != result.stdout:
        raise ValueError("material-relief QML output does not repeat")
    if native_output is not None:
        native_output.mkdir()
        (native_output / "request.json").write_bytes(source)
        qml = native_output / "Paint.qml"
        qml.write_bytes(result.stdout)
        capture = native_output / "capture.png"
        environment = os.environ.copy()
        environment.update(QT_QPA_PLATFORM="wayland", QT_QUICK_BACKEND="software",
                           QT_SCALE_FACTOR=str(scale), RESINA_PAINT_QML=qml.resolve().as_uri(),
                           RESINA_PAINT_IMAGE=str(capture.resolve()))
        native = subprocess.run(["quickshell", "--no-color", "--path", str(ROOT / "conformance/quickshell/paint-probe.qml")],
                                env=environment, capture_output=True, timeout=30, check=False)
        log = (native.stdout + native.stderr).decode("utf-8", errors="replace")
        (native_output / "native.log").write_text(log, encoding="utf-8")
        if native.returncode or "RESINA_PAINT_PASS" not in log or "RESINA_PAINT_FAIL" in log or "ERROR" in log:
            raise ValueError(f"native material-relief paint failed: {log[-2000:]}")
        canvas = Image.new("RGBA", (round(320 * scale), round(160 * scale)))
        prepared = Image.new("RGBA", (width, height))
        prepared.putdata(expected)
        canvas.paste(prepared, (left, top))
        with Image.open(capture) as actual:
            actual.load()
            compare_images(canvas, actual)
        canvas.save(native_output / "expected.png")
    return width * height, worst


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-output-dir", type=Path, help="also compare actual Quickshell drawing in a supplied scale-1 Wayland session")
    parser.add_argument("backend", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.backend[1:] if args.backend[:1] == ["--"] else args.backend
    if not command:
        parser.error("supply a backend command after --")
    if args.native_output_dir is not None:
        args.native_output_dir.mkdir(parents=True, exist_ok=False)
    cases = load_json(ROOT / "conformance/ir/material-relief-paint-vectors.json")
    baseline = next(case["request"] for case in cases if case["name"] == "cast-convex-opaque-face-active-light")
    checked, pixels, worst = 0, 0, 0
    for family in ("cast", "frost", "elastomer", "gel"):
        for kind in ("flat", "convex", "recessed"):
            request = copy.deepcopy(baseline)
            request["outline"]["size"] = {"width": 12, "height": 10}
            request["profile"].update(kind=kind, amplitude=0 if kind == "flat" else 0.5, shoulderWidth=0 if kind == "flat" else 2)
            request["pigment"]["materialFamily"] = family
            request.update(edgeWidth=0.5, points=[])
            for scale in (1, 1.25, 1.5, 2, 3):
                for samples in (1, 3):
                    native_output = args.native_output_dir / f"{family}-{kind}-{scale}-{samples}" if args.native_output_dir is not None else None
                    count, difference = verify(command, json.dumps(request).encode("utf-8"), request, scale, samples, native_output)
                    pixels += count
                    worst = max(worst, difference)
                    checked += 1
    failures, controls = source_cases()
    failures = list(failures)
    failures.extend((name, source) for name, source in (("oversized", b" " * 1_048_577), ("invalid UTF-8", b"\xff"), ("malformed", b"{")))
    for name, source in failures:
        result = invoke(command, source.encode("utf-8") if isinstance(source, str) else source, ["-", "1", "1", "0", "0"])
        if result.returncode != 1 or result.stdout or not result.stderr:
            raise ValueError(f"{name}: invalid source must fail atomically")
        checked += 1
    for _, source, _ in controls:
        count, difference = verify(command, source.encode("utf-8"), json.loads(source), 1, 1)
        pixels += count
        worst = max(worst, difference)
        checked += 1
    for options in (["-", "nan", "1", "0", "0"], ["-", "1", "0", "0", "0"], ["-", "1", "1", "1e20", "0"], ["-", "10000", "1", "0", "0"]):
        result = invoke(command, json.dumps(baseline).encode("utf-8"), options)
        if result.returncode != 1 or result.stdout or not result.stderr:
            raise ValueError("invalid native placement/sampling must fail atomically")
        checked += 1
    with tempfile.TemporaryDirectory() as directory:
        request_path = Path(directory) / "réquest.json"
        source = json.dumps(baseline).encode("utf-8")
        request_path.write_bytes(source)
        options = ["1", "1", "13.3", "17.1"]
        file_result = invoke(command, b"", [str(request_path), *options])
        stdin_result = invoke(command, source, ["-", *options])
        if file_result.returncode or file_result.stderr or stdin_result.returncode or stdin_result.stderr or not file_result.stdout or file_result.stdout != stdin_result.stdout:
            raise ValueError("non-ASCII file input differs from stdin")
        checked += 1
    usage = invoke(command, b"", [])
    if usage.returncode != 2 or usage.stdout or not usage.stderr:
        raise ValueError("missing arguments must produce status2 and usage")
    checked += 1
    print(f"Material relief QML passed {checked} cases and {pixels} independent pixels; worst RGB difference {worst}, exact alpha")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.TimeoutExpired) as error:
        raise SystemExit(f"FAIL: {error}") from error
