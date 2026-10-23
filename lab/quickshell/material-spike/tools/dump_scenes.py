"""Write the lit Command and Toggle acceptance scenes as shader parameters and CPU raster reference images.

The parameters come from the independent lit control body arithmetic; the reference images come from the
CPU raster that released Quickshell uses today. Focused scenes are skipped: the spike draws no focus ring.
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "tools"))

from check_schemas import load_json  # noqa: E402
from lit_body_arithmetic import Body  # noqa: E402
from lit_control_scenes import MANIFEST, body_source, prepare, viewport  # noqa: E402
from material_scenarios import backend_document  # noqa: E402

SCALES = (1, 2)


def color(value):
    return [float(c) for c in value["components"]]


def parameters(scene, resolution, response):
    request = scene["request"]
    body = Body(body_source(request, resolution, response))
    light = body.light
    interaction = light["interaction"]
    return {
        "name": scene["name"],
        "component": "toggle" if "part" in request else "command",
        "family": scene["expectedMaterialFamily"],
        "size": [body.width, body.height],
        "radii": [float(r) for r in body.radii],
        "edgeWidth": body.edge_width,
        "depth": body.depth,
        "relief": {"kind": body.kind, "amplitude": body.amplitude, "shoulder": body.shoulder},
        "body": color(body.pigment["body"]),
        "highlight": color(body.pigment["highlight"]),
        "side": color(body.pigment["side"]),
        "edge": color(body.edge),
        "key": [light["keyDirection"]["x"], light["keyDirection"]["y"]],
        "ambient": light["ambient"],
        "interaction": None if interaction is None else {
            "direction": [interaction["direction"]["x"], interaction["direction"]["y"]],
            "strength": interaction["strength"]},
        "surround": color(request["surface"]["surroundingColor"]),
    }


def render(raster, request, view):
    options = ["-", str(view["width"]), str(view["height"]), str(view["origin"]["x"]), str(view["origin"]["y"]),
               str(view["pixelsPerUnit"]), str(view["samplesPerAxis"])]
    done = subprocess.run([raster, *options], input=json.dumps(request).encode(), capture_output=True, timeout=120)
    if done.returncode != 0:
        raise ValueError(done.stderr.decode(errors="replace")[:300])
    return done.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--theme-backend", required=True)
    parser.add_argument("--command-raster", required=True)
    parser.add_argument("--toggle-raster", required=True)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--bench-scene", type=Path, help="also write the benchmark scene (light-command-standard-rest) as a script")
    arguments = parser.parse_args()
    bundle, resolutions = prepare(load_json(MANIFEST), lambda request: backend_document(
        [arguments.theme_backend, "-"], request, "schemas/headless-result.schema.json", "scene theme", 30))
    capture = bundle["capture"]
    arguments.output_dir.mkdir(parents=True, exist_ok=True)
    records = []
    for scene in bundle["scenarios"]:
        request = scene["request"]
        if "focused" in request["surface"]["body"]["surface"]["states"]["states"]:
            continue
        record = parameters(scene, *resolutions[scene["name"]])
        record["margin"] = capture["margin"]
        record["views"] = {}
        raster = arguments.command_raster if record["component"] == "command" else arguments.toggle_raster
        for scale in SCALES:
            view = viewport(request, capture, scale, capture["samplesPerAxis"])
            name = f"{scene['name']}@{scale}.png"
            (arguments.output_dir / name).write_bytes(render(raster, request, view))
            record["views"][str(scale)] = {"image": name, "width": view["width"], "height": view["height"]}
        records.append(record)
    (arguments.output_dir / "scenes.json").write_text(json.dumps({"scenes": records}, indent=1) + "\n")
    if arguments.bench_scene:
        scene = {key: value for key, value in next(r for r in records if r["name"] == "light-command-standard-rest").items() if key != "views"}
        text = ".pragma library\nvar scene = " + json.dumps(scene, indent=1) + ";\n"
        arguments.bench_scene.write_text(text)
    print(f"{len(records)} scenes")


if __name__ == "__main__":
    sys.exit(main())
