"""Verify the public material-lit raster against independent rectangular arithmetic."""
import argparse
import copy
import io
import json
import math
import subprocess
import zlib

from PIL import Image

from check_material_relief_paint_backend import source_cases
from check_schemas import ROOT, load_json
from material_relief_arithmetic import rectangular_pixels


def invoke(command, source, options):
    return subprocess.run([*command, *options], input=source, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60, check=False)


def check_png(result, width, height, expected):
    if result.returncode != 0 or result.stderr:
        raise ValueError(f"raster failed with status {result.returncode}: {result.stderr.decode('utf-8', errors='replace')[:300]}")
    if len(expected) != width * height:
        raise ValueError("independent pixels must cover the complete requested image")
    pixels = decoded_png(result, width, height)
    worst = 0
    for index, wanted in enumerate(expected):
        actual = tuple(pixels[4 * index:4 * index + 4])
        if wanted[3] == 0 and actual != (0, 0, 0, 0):
            raise ValueError(f"pixel {index}: transparent pixels must be clear")
        if actual[3] != wanted[3]:
            raise ValueError(f"pixel {index}: coverage differs: {actual[3]} != {wanted[3]}")
        difference = max(abs(actual[channel] - wanted[channel]) for channel in range(3))
        worst = max(worst, difference)
        if difference > 1:
            raise ValueError(f"pixel {index}: color differs: {actual} != {wanted}")
    return worst


def decoded_png(result, width, height):
    """The RGBA8 bytes of one complete perceptual-sRGB PNG with the requested dimensions."""
    if result.returncode != 0 or result.stderr:
        raise ValueError(f"raster failed with status {result.returncode}: {result.stderr.decode('utf-8', errors='replace')[:300]}")
    if len(result.stdout) < 33 or result.stdout[:8] != b"\x89PNG\r\n\x1a\n" or result.stdout[12:16] != b"IHDR" or result.stdout[24:26] != bytes((8, 6)):
        raise ValueError("result must declare 8-bit RGBA in its PNG header")
    offset = 8
    while offset < len(result.stdout):
        if len(result.stdout) - offset < 12:
            raise ValueError("result must contain a single PNG")
        length = int.from_bytes(result.stdout[offset:offset + 4], "big")
        end = offset + length + 12
        if end > len(result.stdout):
            raise ValueError("result must contain a single PNG")
        checksum = int.from_bytes(result.stdout[end - 4:end], "big")
        if zlib.crc32(result.stdout[offset + 4:end - 4]) != checksum:
            raise ValueError("PNG chunk checksum mismatch")
        if result.stdout[offset + 4:offset + 8] == b"IEND":
            if length != 0 or end != len(result.stdout):
                raise ValueError("result must contain a single PNG")
            break
        offset = end
    else:
        raise ValueError("result must contain a single PNG")
    with Image.open(io.BytesIO(result.stdout)) as image:
        image.load()
        if image.format != "PNG" or image.mode != "RGBA" or image.size != (width, height) or image.info.get("srgb") != 0:
            raise ValueError("result must be an RGBA8 perceptual-sRGB PNG with the requested dimensions")
        return image.tobytes()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("backend", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.backend[1:] if args.backend[:1] == ["--"] else args.backend
    if not command:
        parser.error("supply a backend command after --")
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
                    width, height = math.ceil(14 * scale), math.ceil(12 * scale)
                    options = ["-", str(width), str(height), "-1", "-1", str(scale), str(samples)]
                    expected = rectangular_pixels(request, width, height, (-1, -1), scale, samples)
                    result = invoke(command, json.dumps(request).encode("utf-8"), options)
                    difference = check_png(result, width, height, expected)
                    repeated = invoke(command, json.dumps(request).encode("utf-8"), options)
                    if repeated.returncode != 0 or repeated.stderr or repeated.stdout != result.stdout:
                        raise ValueError(f"{family}/{kind}/{scale}/{samples}: output is not deterministic")
                    worst = max(worst, difference)
                    checked += 1
                    pixels += width * height
    failures, controls = source_cases()
    for name, source in failures:
        result = invoke(command, source.encode("utf-8"), ["-", "1", "1", "59.5", "1.5", "1", "1"])
        if result.returncode != 1 or result.stdout or not result.stderr:
            raise ValueError(f"{name}: invalid source must fail atomically with status 1 and a diagnostic")
        checked += 1
    for name, source, _ in controls:
        request = json.loads(source)
        expected = rectangular_pixels(request, 1, 1, (59.5, 1.5), 1, 1)
        difference = check_png(invoke(command, source.encode("utf-8"), ["-", "1", "1", "59.5", "1.5", "1", "1"]), 1, 1, expected)
        worst = max(worst, difference)
        checked += 1
        pixels += 1
    for name, source in (("oversized", b" " * 1_048_577), ("invalid UTF-8", b"\xff"), ("malformed", b"{")):
        result = invoke(command, source, ["-", "1", "1", "0", "0", "1", "1"])
        if result.returncode != 1 or result.stdout or not result.stderr:
            raise ValueError(f"{name}: invalid input must fail atomically")
        checked += 1
    for name, options in (("zero width", ["-", "0", "1", "0", "0", "1", "1"]),
                          ("nonfinite scale", ["-", "1", "1", "0", "0", "nan", "1"]),
                          ("invalid samples", ["-", "1", "1", "0", "0", "1", "0"]),
                          ("resource limit", ["-", "4294967295", "1", "0", "0", "1", "1"])):
        result = invoke(command, json.dumps(baseline).encode("utf-8"), options)
        if result.returncode != 1 or result.stdout or not result.stderr:
            raise ValueError(f"{name}: invalid viewport must fail atomically")
        checked += 1
    usage = invoke(command, b"", [])
    if usage.returncode != 2 or usage.stdout or not usage.stderr:
        raise ValueError("missing arguments must produce status 2 and usage on stderr")
    checked += 1
    print(f"Material relief raster passed {checked} cases and {pixels} independent pixels; worst RGB difference {worst}, exact alpha")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.TimeoutExpired) as error:
        raise SystemExit(f"FAIL: {error}") from error
