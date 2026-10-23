"""Verify the lit Command and Toggle acceptance scenes against the independent lit control body arithmetic.

Every scene is resolved by the public paint command and drawn by the CPU raster. The resolved paint is
compared with the independent semantics of its theme and request, and every raster pixel with bounds derived
from the independent lit control body arithmetic. This certifies the scenes; it does not accept their styling.
"""
import argparse
import json
import math
import multiprocessing
import os
import subprocess
import sys

from check_command_paint_backend import command_mismatch
from check_headless_backend import run_backend
from check_lit_body_raster_backend import check_bounded_png, invoke
from check_schemas import load_json, parse_json, validator_for
from check_toggle_part_paint_backend import toggle_part_mismatch
from lit_control_scenes import MANIFEST, expected_pixels, prepare, viewport
from material_scenarios import backend_document

COARSE_PROFILE = (1, 2)
IR_SCHEMAS = {"commandPaint": "schemas/command-paint-ir.schema.json", "togglePartPaint": "schemas/toggle-part-paint-ir.schema.json"}
TIMEOUT = 120
CONTROLS = (
    ("hover-none-light", "hover", True),
    ("hover-ambient-light", "hover", True),
    ("hover-interaction-light", "hover-none-light", True),
    ("hover-high-contrast", "hover", True),
    ("hover-reduced-motion", "hover", True),
    ("hover-all-capabilities-off", "hover", True),
    ("hover-rtl", "hover", False),
)


def render(raster, request, view):
    options = ["-", str(view["width"]), str(view["height"]), str(view["origin"]["x"]), str(view["origin"]["y"]),
               str(view["pixelsPerUnit"]), str(view["samplesPerAxis"])]
    return invoke([raster], json.dumps(request).encode("utf-8"), options)


def profiles(capture):
    """The capture scale and sampling of the scene catalog, then a coarser scale with fewer samples."""
    return [(capture["pixelsPerUnit"], capture["samplesPerAxis"]), COARSE_PROFILE]


def check_scene(task):
    scene, resolution, response, capture, resolver, raster = task
    request, name = scene["request"], scene["name"]
    result = run_backend([resolver, "-"], json.dumps(request, allow_nan=False), TIMEOUT)
    if result.returncode != 0 or result.stderr:
        raise ValueError(f"{name}: resolver exit {result.returncode}: {result.stderr[:200]}")
    ir = parse_json(result.stdout, name)
    errors = list(validator_for(IR_SCHEMAS[scene["kind"]]).iter_errors(ir))
    if errors:
        raise ValueError(f"{name}: {errors[0].message}")
    mismatch = (command_mismatch(ir, request, None, resolution) if scene["kind"] == "commandPaint"
                else toggle_part_mismatch(ir, request, None, resolution))
    if mismatch:
        raise ValueError(f"{name}: resolved paint differs from the independent semantics at {mismatch}")
    if ir["paint"]["body"]["materialFamily"] != scene["expectedMaterialFamily"]:
        raise ValueError(f"{name}: resolved material family differs from the scene")
    worst, count = 0, 0
    for scale, samples in profiles(capture):
        view = viewport(request, capture, scale, samples)
        bounds = expected_pixels(request, resolution, response, view)
        first = render(raster, request, view)
        try:
            worst = max(worst, check_bounded_png(first, view["width"], view["height"], bounds))
        except ValueError as error:
            raise ValueError(f"{name} at scale {scale} with {samples} samples: {error}") from error
        if render(raster, request, view).stdout != first.stdout:
            raise ValueError(f"{name}: raster output is not deterministic")
        count += view["width"] * view["height"]
    return name, ir["paint"]["body"]["face"]["pigment"]["body"]["components"], count, worst


def discriminates(scene, other, resolutions, capture, raster):
    """Whether the pixels of `scene` leave the bounds of `other`, so that the evidence would notice the difference."""
    view = viewport(scene["request"], capture, *profiles(capture)[1])
    resolution, response = resolutions[other["name"]]
    bounds = expected_pixels(other["request"], resolution, response, view)
    try:
        check_bounded_png(render(raster, scene["request"], view), view["width"], view["height"], bounds)
    except ValueError:
        return True
    return False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--theme-backend", required=True)
    parser.add_argument("--command-backend", required=True)
    parser.add_argument("--toggle-backend", required=True)
    parser.add_argument("--command-raster", required=True)
    parser.add_argument("--toggle-raster", required=True)
    parser.add_argument("--jobs", type=int, default=min(4, os.cpu_count() or 1))
    arguments = parser.parse_args()
    try:
        bundle, resolutions = prepare(load_json(MANIFEST), lambda request: backend_document(
            [arguments.theme_backend, "-"], request, "schemas/headless-result.schema.json", "scene theme", 30))
        capture = bundle["capture"]
        scenes = {scene["name"]: scene for scene in bundle["scenarios"]}
        tasks = [(scene, *resolutions[scene["name"]], capture,
                  arguments.command_backend if scene["kind"] == "commandPaint" else arguments.toggle_backend,
                  arguments.command_raster if scene["kind"] == "commandPaint" else arguments.toggle_raster)
                 for scene in bundle["scenarios"]]
        with multiprocessing.Pool(max(1, arguments.jobs)) as pool:
            results = pool.map(check_scene, tasks)
        bodies = {name: body for name, body, _, _ in results}
        for name, scene in scenes.items():
            if scene["kind"] != "togglePartPaint" or scene["request"]["part"] != "thumb":
                continue
            track = scenes[name.replace("-thumb-", "-track-")]
            adjacent = scene["request"]["surface"]["body"]["adjacentColor"]["components"]
            if any(not math.isclose(a, b, rel_tol=0, abs_tol=1e-12) for a, b in zip(adjacent, bodies[track["name"]])):
                raise ValueError(f"{name}: the thumb edge is not measured against the resolved track body")
        controls = 0
        for theme in ("light", "dark"):
            for control, reference, distinct in CONTROLS:
                scene, other = scenes[f"{theme}-command-standard-{control}"], scenes[f"{theme}-command-standard-{reference}"]
                if discriminates(scene, other, resolutions, capture, arguments.command_raster) != distinct:
                    raise ValueError(f"{scene['name']}: its pixels {'cannot be told from' if distinct else 'differ from'} {other['name']}")
                controls += 1
    except (AssertionError, OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"FAIL lit control scenes: {error}", file=sys.stderr)
        return 1
    print(f"Lit control scenes passed {len(results)} scenes, {sum(count for _, _, count, _ in results)} independent pixels "
          f"and {controls} discrimination controls; worst RGB difference {max(worst for _, _, _, worst in results)}, exact alpha")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
