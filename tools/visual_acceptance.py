import argparse
import hashlib
import io
import json
import math
import platform
import struct
import subprocess
import sys
import tempfile
from functools import lru_cache
from pathlib import Path

import PIL
from PIL import Image

from check_command_motion_backend import motion_mismatch
from check_headless_backend import mismatch
from check_surface_readability_backend import contrast
from check_material_scenarios import check_scene
from check_schemas import ROOT, load_json, validator_for
from material_scenarios import backend_document, validate


KINDS = {
    "opaqueSurface": ("opaque-surface-request", "opaque-surface-ir"),
    "focusRing": ("focus-ir-request", "focus-indicator-ir"),
    "surfacePaint": ("surface-paint-request", "surface-paint-ir"),
    "commandMotion": ("command-motion-request", "command-motion-ir"),
}
MAX_PIXELS = 4_194_304
MAX_PNG_BYTES = 64 * 1024 * 1024
SRGB_TO_XYZ = (
    (506752 / 1228815, 87881 / 245763, 12673 / 70218),
    (87098 / 409605, 175762 / 245763, 12673 / 175545),
    (7918 / 409605, 87881 / 737289, 1001167 / 1053270),
)
XYZ_TO_LMS = (
    (0.819022437996703, 0.3619062600528904, -0.1288737815209879),
    (0.0329836539323885, 0.9292868615863434, 0.0361446663506424),
    (0.0481771893596242, 0.2642395317527308, 0.6335478284694309),
)
LMS_TO_OKLAB = (
    (0.210454268309314, 0.7936177747023054, -0.0040720430116193),
    (1.9779985324311684, -2.42859224204858, 0.450593709617411),
    (0.0259040424655478, 0.7827717124575296, -0.8086757549230774),
)


def transform(matrix, vector):
    return tuple(sum(a * b for a, b in zip(row, vector, strict=True)) for row in matrix)


def decode_srgb(component):
    return component / 12.92 if component <= 0.04045 else ((component + 0.055) / 1.055) ** 2.4


def linear_to_oklab(linear):
    xyz = transform(SRGB_TO_XYZ, linear)
    return transform(LMS_TO_OKLAB, tuple(math.cbrt(value) for value in transform(XYZ_TO_LMS, xyz)))


def srgb_to_oklab(components):
    if len(components) != 3 or any(not math.isfinite(c) or not 0 <= c <= 1 for c in components):
        raise ValueError("Oklab input requires three finite encoded sRGB components in [0, 1]")
    return linear_to_oklab(tuple(decode_srgb(c) for c in components))


def png_image(source):
    if len(source) > MAX_PNG_BYTES:
        raise ValueError("PNG exceeds capture byte limit")
    if (len(source) < 33 or source[:8] != b"\x89PNG\r\n\x1a\n"
            or source[8:16] != b"\x00\x00\x00\rIHDR" or source[24:26] != b"\x08\x06"):
        raise ValueError("capture must be an RGBA8 PNG without depth conversion")
    width, height = struct.unpack(">II", source[16:24])
    if not width or not height or width * height > MAX_PIXELS:
        raise ValueError("capture exceeds pixel limit or has empty dimensions")
    try:
        image = Image.open(io.BytesIO(source))
        if image.format != "PNG" or image.mode != "RGBA":
            raise ValueError("capture must be an RGBA PNG")
        if image.width * image.height > MAX_PIXELS or getattr(image, "n_frames", 1) != 1:
            raise ValueError("capture exceeds pixel limit or contains multiple frames")
        if image.info.get("srgb") != 0 or "icc_profile" in image.info:
            raise ValueError("capture must declare perceptual sRGB without an alternate ICC profile")
        image.load()
        return image
    except (OSError, Image.DecompressionBombError) as error:
        raise ValueError(f"invalid capture PNG: {error}") from error


@lru_cache(maxsize=65536)
def pixel_difference(first, second):
    alpha_difference = abs(first[3] - second[3]) / 255
    maximum = 0.0
    for backdrop in (0.0, 1.0):
        colors = []
        for pixel in (first, second):
            alpha = pixel[3] / 255
            linear = tuple(decode_srgb(c / 255) * alpha + backdrop * (1 - alpha) for c in pixel[:3])
            colors.append(linear_to_oklab(linear))
        maximum = max(maximum, math.dist(*colors))
    return maximum, alpha_difference


def image_metrics(first_source, second_source):
    first, second = png_image(first_source), png_image(second_source)
    if first.size != second.size:
        raise ValueError("capture dimensions differ")
    changed, total, maximum, alpha = 0, 0.0, 0.0, 0.0
    first_rgba, second_rgba = first.tobytes(), second.tobytes()
    for offset in (() if first_rgba == second_rgba else range(0, len(first_rgba), 4)):
        before, after = first_rgba[offset:offset + 4], second_rgba[offset:offset + 4]
        if before == after:
            continue
        changed += 1
        distance, alpha_difference = pixel_difference(before, after)
        total += distance
        maximum = max(maximum, distance)
        alpha = max(alpha, alpha_difference)
    return {
        "changedPixels": changed,
        "pixelCount": first.width * first.height,
        "maximumOklabDistance": maximum,
        "meanOklabDistance": total / (first.width * first.height),
        "maximumAlphaDifference": alpha,
    }


def validate_capture(scene, result, viewport):
    validate_viewport(viewport)
    kind = scene["kind"]
    if kind not in KINDS:
        raise ValueError(f"unsupported visual scene kind: {kind}")
    request_schema, result_schema = KINDS[kind]
    validate(f"schemas/{request_schema}.schema.json", scene["request"], scene["name"])
    validate(f"schemas/{result_schema}.schema.json", result, scene["name"])
    if kind == "commandMotion":
        difference = motion_mismatch(result, scene["request"])
        if difference:
            raise ValueError(f"{scene['name']}: sampled command differs at {difference}")
        check_lit_command_scene(scene, result["command"]["paint"], viewport)
    else:
        check_scene(scene, result, viewport)
        validate_paint_evidence(kind, scene["request"], result)


def check_lit_command_scene(scene, paint, viewport):
    """Bindings, edge guard and complete capture bounds of a lit Command paint sample."""
    body, request = paint["body"], scene["request"]["surface"]["body"]
    if body["materialFamily"] != scene["expectedMaterialFamily"]:
        raise ValueError(f"{scene['name']}: resolved material family differs from scene")
    edge = body["face"]["edge"]
    validate_ratio(edge["color"], request["adjacentColor"], edge["contrastRatio"],
                   request["minimumEdgeContrast"], "edge")
    if "focus" in paint:
        check_scene(scene | {"kind": "focusRing", "request": {"surface": request["surface"]}},
                    paint["focus"], viewport)
    bounds = body["geometry"]["silhouette"]["bounds"]
    left, top = viewport["origin"]["x"], viewport["origin"]["y"]
    right = left + viewport["width"] / viewport["pixelsPerUnit"]
    bottom = top + viewport["height"] / viewport["pixelsPerUnit"]
    if any(actual > limit and not math.isclose(actual, limit, rel_tol=1e-12, abs_tol=1e-12)
           for actual, limit in ((left, bounds["x"]), (top, bounds["y"]),
                                 (bounds["x"] + bounds["width"], right), (bounds["y"] + bounds["height"], bottom))):
        raise ValueError(f"{scene['name']}: capture clips required paint bounds")


def validate_ratio(color, background, reported, minimum, label):
    actual = contrast(color, background)
    if not math.isclose(actual, reported, rel_tol=0, abs_tol=1e-12):
        raise ValueError(f"{label}: reported contrast differs from actual colors")
    if actual < minimum and not math.isclose(actual, minimum, rel_tol=0, abs_tol=1e-12):
        raise ValueError(f"{label}: actual contrast falls below requested guard")


def validate_paint_evidence(kind, request, result):
    if kind == "surfacePaint":
        validate_paint_evidence("opaqueSurface", request["body"], result["body"])
        if "focus" in result:
            validate_ratio(result["focus"]["indicator"]["color"], request["surroundingColor"],
                           result["focus"]["indicator"]["contrastRatio"], 3, "focus")
    elif kind == "opaqueSurface":
        front = result["geometry"]["front"]["bounds"]
        if mismatch(front, {"x": 0, "y": 0, **request["size"]}):
            raise ValueError("front geometry differs from requested size")
        direction = request["appearance"]["keyLight"]["direction"]
        length = math.hypot(direction["x"], direction["y"])
        if not length or not math.isfinite(length):
            raise ValueError("physical key light direction must be finite and nonzero")
        if mismatch(result["lighting"]["direction"], {axis: value / length for axis, value in direction.items()}):
            raise ValueError("resolved physical key light differs from request")
        band = request["appearance"]["bands"][result["materialFamily"]]
        if any(mismatch(result[field], band[field]) for field in ("edgeWidth", "highlightWidth")):
            raise ValueError("resolved paint bands differ from request")
        validate_ratio(result["foreground"], result["pigment"]["body"], result["contentContrastRatio"],
                       request["minimumContentContrast"], "content")
        validate_ratio(result["edge"]["color"], request["adjacentColor"], result["edge"]["contrastRatio"],
                       request["minimumEdgeContrast"], "edge")
    elif kind == "focusRing":
        validate_ratio(result["indicator"]["color"], request["surroundingColor"],
                       result["indicator"]["contrastRatio"], 3, "focus")


def source_environment(scene):
    request = scene["request"]
    if scene["kind"] == "commandMotion":
        request = request["surface"]["body"]
    elif scene["kind"] == "surfacePaint":
        request = request["body"]
    return request["theme"]["environment"]


def sha256(source):
    return hashlib.sha256(source).hexdigest()


def git_blob(source):
    return hashlib.sha1(b"blob " + str(len(source)).encode("ascii") + b"\0" + source).hexdigest()


def validate_viewport(viewport):
    schema = load_json(ROOT / "tools/schemas/visual-capture-record.schema.json")["$defs"]["capture"]
    errors = list(validator_for("tools/schemas/visual-capture-record.schema.json").evolve(schema=schema).iter_errors(viewport))
    if errors:
        raise ValueError(f"capture viewport: {errors[0].message}")
    values = [viewport["pixelsPerUnit"], *viewport["origin"].values()]
    if any(not math.isfinite(value) for value in values):
        raise ValueError("capture coordinates and scale must be finite")
    pixels = viewport["width"] * viewport["height"]
    if pixels > MAX_PIXELS or pixels * viewport["samplesPerAxis"] ** 2 > 16_777_216:
        raise ValueError("capture exceeds pixel or sampling budget")


def validate_record(record, source):
    validate("tools/schemas/visual-capture-record.schema.json", record, "visual record")
    scene, viewport = record["scene"], record["capture"]
    validate_capture(scene, record["result"], viewport)
    image = png_image(source)
    if image.size != (viewport["width"], viewport["height"]):
        raise ValueError("PNG dimensions differ from capture record")
    if sha256(source) != record["image"]["sha256"]:
        raise ValueError("PNG digest differs from capture record")
    time = scene["request"]["time"] if scene["kind"] == "commandMotion" else 0
    channels = scene["request"]["channels"] if scene["kind"] == "commandMotion" else None
    if record["sampling"] != {"time": time, "initialChannels": channels}:
        raise ValueError("capture sampling differs from its complete request")
    environment = source_environment(scene)
    if record["scales"] != {"device": viewport["pixelsPerUnit"], "text": environment["textScale"]}:
        raise ValueError("capture scales differ from actual raster/text context")
    if scene["kind"] == "commandMotion" and record["purpose"] != "diagnostic":
        raise ValueError("the current sampled Command producer is diagnostic, not accepted styling")


def record_image(path, record):
    image_path = (path.parent / record["image"]["file"]).resolve()
    if not image_path.is_relative_to(path.parent.resolve()):
        raise ValueError("capture image escapes record directory")
    with image_path.open("rb") as source:
        if image_path.stat().st_size > MAX_PNG_BYTES:
            raise ValueError("PNG exceeds capture byte limit")
        data = source.read(MAX_PNG_BYTES + 1)
    if len(data) > MAX_PNG_BYTES:
        raise ValueError("PNG exceeds capture byte limit")
    return data


def validate_native_label(record, source):
    validate("tools/schemas/native-label-capture.schema.json", record, "native label capture")
    image = png_image(source)
    if image.size != (record["width"], record["height"]):
        raise ValueError("native label PNG dimensions differ from record")
    if git_blob(source) != record["image"]["gitBlob"]:
        raise ValueError("native label PNG Git blob differs from record")
    if record["image"]["file"] != record["scenario"] + ".png":
        raise ValueError("native label image differs from scenario name")
    bounds = record["semanticResult"]["labelBounds"]
    scale = record["deviceScale"]
    if (bounds["x"] + bounds["width"]) * scale > image.width + 1e-12 or (bounds["y"] + bounds["height"]) * scale > image.height + 1e-12:
        raise ValueError("native label capture clips resolved content bounds")


def compare_native_labels(before, after, images):
    for record, source in zip((before, after), images, strict=True):
        validate_native_label(record, source)
    for field in ("kind", "purpose", "scenario", "width", "height", "deviceScale", "textScale", "sampling", "font", "scriptFonts", "semanticResult", "colorEncoding"):
        difference = mismatch(after[field], before[field])
        if difference:
            raise ValueError(f"native label context/semantics differ at /{field}{difference}")
    if mismatch(after["renderer"]["gpu"], before["renderer"]["gpu"]):
        raise ValueError("cross-GPU native label tolerance has not been accepted")
    return image_metrics(*images)


def compare_records(before_path, after_path):
    before, after = load_json(before_path), load_json(after_path)
    for record in (before, after):
        native = isinstance(record, dict) and record.get("kind") == "nativeLabel"
        schema = "native-label-capture" if native else "visual-capture-record"
        validate(f"tools/schemas/{schema}.schema.json", record, "comparison record")
    images = record_image(before_path, before), record_image(after_path, after)
    if before.get("kind") == "nativeLabel" or after.get("kind") == "nativeLabel":
        return compare_native_labels(before, after, images)
    for record, source in zip((before, after), images, strict=True):
        validate_record(record, source)
    for field in ("purpose", "scene", "capture", "sampling", "scales", "colorEncoding", "font"):
        difference = mismatch(after[field], before[field])
        if difference:
            raise ValueError(f"capture context differs at /{field}{difference}")
    if after["renderer"]["name"] != before["renderer"]["name"]:
        raise ValueError("cross-renderer tolerance has not been accepted")
    difference = mismatch(after["result"], before["result"])
    if difference:
        raise ValueError(f"semantic/geometry result differs at {difference}")
    return image_metrics(*images)


def render_request(raster, request, viewport, timeout):
    validate_viewport(viewport)
    arguments = [str(raster), "-", str(viewport["width"]), str(viewport["height"]),
                 str(viewport["origin"]["x"]), str(viewport["origin"]["y"]),
                 str(viewport["pixelsPerUnit"]), str(viewport["samplesPerAxis"])]
    with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as diagnostic:
        rendered = subprocess.run(arguments, input=json.dumps(request, ensure_ascii=False, allow_nan=False).encode("utf-8"),
                                  stdout=output, stderr=diagnostic, timeout=timeout, check=False)
        diagnostic.seek(0)
        error = diagnostic.read(201)
        if rendered.returncode != 0 or error:
            raise ValueError(f"raster failed: exit {rendered.returncode}, stderr {error[:200]!r}")
        if output.tell() > MAX_PNG_BYTES:
            raise ValueError("raster PNG exceeds capture byte limit")
        output.seek(0)
        return output.read(MAX_PNG_BYTES + 1)


def capture_scene(scene, viewport, resolver, raster, output, purpose, timeout):
    validate("tools/schemas/visual-scene.schema.json", scene, "visual scene")
    validate_viewport(viewport)
    if output.exists():
        raise ValueError("capture output already exists; choose a new directory")
    state = subprocess.run(["git", "status", "--porcelain"], cwd=ROOT, capture_output=True, text=True, check=True)
    if state.stdout:
        raise ValueError("capture requires a committed, clean source tree")
    revision = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    result_schema = f"schemas/{KINDS[scene['kind']][1]}.schema.json"
    results, images = [], []
    for _ in range(2):
        result = backend_document([str(resolver), "-"], scene["request"], result_schema, scene["name"], timeout)
        validate_capture(scene, result, viewport)
        results.append(result)
        images.append(render_request(raster, scene["request"], viewport, timeout))
    if mismatch(*results) or image_metrics(*images)["changedPixels"]:
        raise ValueError("repeating the exact capture changed semantic results or pixels")
    environment = source_environment(scene)
    record = {
        "schemaVersion": "0.1.0", "commit": revision, "purpose": purpose,
        "scene": scene, "result": results[0], "capture": viewport,
        "scales": {"device": viewport["pixelsPerUnit"], "text": environment["textScale"]},
        "sampling": {"time": scene["request"].get("time", 0), "initialChannels": scene["request"].get("channels")},
        "font": None, "colorEncoding": "rgba8-srgb-straight",
        "renderer": {"name": raster.stem, "sha256": sha256(raster.read_bytes()), "resolverSha256": sha256(resolver.read_bytes()), "gpu": None},
        "runtime": {"system": platform.platform(), "machine": platform.machine(), "python": platform.python_version(), "pillow": PIL.__version__},
        "image": {"file": "image.png", "sha256": sha256(images[0])},
    }
    validate_record(record, images[0])
    output.mkdir(parents=True)
    (output / "image.png").write_bytes(images[0])
    (output / "capture.json").write_text(json.dumps(record, ensure_ascii=False, allow_nan=False, indent=2) + "\n", encoding="utf-8")
    return record


def main():
    parser = argparse.ArgumentParser(description="Capture bounded Resina paint evidence or compare identical-context records; no styling acceptance is inferred.")
    commands = parser.add_subparsers(dest="command", required=True)
    capture = commands.add_parser("capture")
    capture.add_argument("--scene", type=Path, required=True)
    capture.add_argument("--viewport", type=Path, required=True)
    capture.add_argument("--resolver", type=Path, required=True)
    capture.add_argument("--raster", type=Path, required=True)
    capture.add_argument("--output", type=Path, required=True)
    capture.add_argument("--purpose", choices=("candidate", "diagnostic"), required=True)
    capture.add_argument("--timeout", type=float, default=30)
    compare = commands.add_parser("compare")
    compare.add_argument("before", type=Path)
    compare.add_argument("after", type=Path)
    arguments = parser.parse_args()
    try:
        if arguments.command == "capture":
            if not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
                raise ValueError("timeout must be positive and finite")
            capture_scene(load_json(arguments.scene), load_json(arguments.viewport), arguments.resolver.resolve(),
                          arguments.raster.resolve(), arguments.output, arguments.purpose, arguments.timeout)
            print("Repeated semantic and raster capture passed; styling acceptance remains explicit.")
        else:
            metrics = compare_records(arguments.before, arguments.after)
            print(json.dumps(metrics, allow_nan=False, sort_keys=True))
            return int(metrics["changedPixels"] != 0)
    except (AssertionError, OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"FAIL visual evidence: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
