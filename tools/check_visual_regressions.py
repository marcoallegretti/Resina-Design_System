import argparse
import copy
import math
import subprocess
import sys
from pathlib import Path

from check_schemas import load_json
from visual_acceptance import compare_records, image_metrics, record_image, render_request, validate_capture, validate_record


def check_regressions(captures, raster, timeout):
    path = captures / "standard-ltr/light-cast-paint-focused/capture.json"
    record = load_json(path)
    original = record_image(path, record)
    validate_record(record, original)
    request, viewport = record["scene"]["request"], record["capture"]
    for name in ("radius", "light", "focus"):
        altered = copy.deepcopy(request)
        if name == "radius":
            shape = altered["body"]["surface"]["form"]["shape"]
            altered["body"]["appearance"]["shapeAssignments"]["profiles"][shape] = {"kind": "uniform", "radius": "radius.0"}
        elif name == "light":
            light = altered["body"]["appearance"]["keyLight"]["direction"]
            light["x"], light["y"] = -light["x"], -light["y"]
        else:
            altered["body"]["surface"]["states"]["states"] = ["rest"]
        changed = render_request(raster, altered, viewport, timeout)
        result = image_metrics(original, changed)
        if not result["changedPixels"]:
            raise ValueError(f"deliberate {name} regression escaped the pixel gate")
        print(f"Detected {name} regression: {result['changedPixels']} changed pixels")
    clipped = viewport | {"width": viewport["width"] - 24}
    try:
        validate_capture(record["scene"], record["result"], clipped)
    except ValueError as error:
        if "clips" not in str(error):
            raise
    else:
        raise ValueError("deliberate clipping escaped the geometry gate")
    changed = render_request(raster, request, clipped, timeout)
    try:
        image_metrics(original, changed)
    except ValueError as error:
        if "dimensions" not in str(error):
            raise
    else:
        raise ValueError("deliberate cropping escaped the image-dimension gate")
    print("Detected clipping regression in geometry and image dimensions")
    first_path = captures / "motion/diagnostic-command-press-1/capture.json"
    late_path = captures / "motion/diagnostic-command-press-3/capture.json"
    first, late = load_json(first_path), load_json(late_path)
    images = record_image(first_path, first), record_image(late_path, late)
    for frame, image in zip((first, late), images, strict=True):
        validate_record(frame, image)
    if image_metrics(*images)["changedPixels"] == 0:
        raise ValueError("replacing a sampled motion frame escaped the pixel gate")
    try:
        compare_records(first_path, late_path)
    except ValueError as error:
        if "context differs" not in str(error):
            raise
    else:
        raise ValueError("a different motion time escaped the sampling/context gate")
    print("Detected motion-frame regression in pixels and sampling context")


def main():
    parser = argparse.ArgumentParser(description="Detect deliberately changed radius, light, focus, crop and sampled motion using real bounded raster captures.")
    parser.add_argument("--captures", type=Path, required=True)
    parser.add_argument("--raster", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=30)
    arguments = parser.parse_args()
    if not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        parser.error("timeout must be finite and positive")
    try:
        check_regressions(arguments.captures, arguments.raster.resolve(), arguments.timeout)
    except (AssertionError, OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"FAIL visual regression evidence: {error}", file=sys.stderr)
        return 1
    print("Visual gate detected all five deliberate regressions; no styling acceptance is inferred.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
