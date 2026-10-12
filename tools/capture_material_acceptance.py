import argparse
import copy
import json
import math
import subprocess
import sys
from pathlib import Path

from check_command_motion_backend import equation_sample
from check_schemas import ROOT, load_json, parse_json
from material_scenarios import MANIFEST, asset_source, backend_document, prepare, validate
from visual_acceptance import capture_scene


ENVIRONMENTS = ROOT / "conformance/scenes/visual-environments.json"


def environments(document, base):
    validate("tools/schemas/visual-environments.schema.json", document, "visual environments")
    names = set()
    result = []
    for variant in document["variants"]:
        if variant["name"] in names:
            raise ValueError(f"duplicate visual environment: {variant['name']}")
        names.add(variant["name"])
        environment = copy.deepcopy(base)
        environment.update({name: value for name, value in variant.items() if name != "name"})
        validate("schemas/environment.schema.json", environment, variant["name"])
        result.append((variant["name"], environment))
    return result


def motion_scenes():
    request = load_json(ROOT / "conformance/ir/command-motion-request.json")
    theme = parse_json(request["surface"]["body"]["theme"]["themeSource"], "motion fixture theme")
    theme["materialAssignments"]["control"]["interactive"] = "elastomer"
    request["surface"]["body"]["theme"]["themeSource"] = json.dumps(theme, allow_nan=False)
    request["surface"]["body"]["surface"]["states"]["states"] = ["pressed", "focused"]
    for index, time in enumerate((0, 0.25, 1, 2)):
        frame = copy.deepcopy(request)
        frame["time"] = time
        yield {"name": f"diagnostic-command-press-{index}", "kind": "commandMotion", "expectedMaterialFamily": "elastomer", "request": frame}
    interrupted = copy.deepcopy(request)
    target = request["commandAppearance"]["profiles"]["elastomer"]["pressed"]
    for channel, value in target.items():
        interrupted["channels"][channel]["initial"] = equation_sample(request["channels"][channel], value, 0.25, False)["state"]
    interrupted["surface"]["body"]["surface"]["states"]["states"] = ["rest", "focused"]
    for index, time in enumerate((0, 0.25, 1, 2)):
        frame = copy.deepcopy(interrupted)
        frame["time"] = time
        yield {"name": f"diagnostic-command-retarget-{index}", "kind": "commandMotion", "expectedMaterialFamily": "elastomer", "request": frame}


def main():
    parser = argparse.ArgumentParser(description="Capture the authored opaque catalog and diagnostic interrupted motion; this does not accept styling or certify complete coverage.")
    parser.add_argument("--bin-dir", type=Path, required=True)
    parser.add_argument("--executable-suffix", default="")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=30)
    arguments = parser.parse_args()
    if not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        parser.error("timeout must be finite and positive")
    if arguments.output.exists():
        parser.error("output already exists; choose a new directory")
    binaries = arguments.bin_dir.resolve()

    def executable(name):
        return binaries / (name + arguments.executable_suffix)

    captured = 0
    try:
        manifest = load_json(MANIFEST)
        base = parse_json(asset_source(manifest["environmentSource"]), "visual base environment")
        for name, environment in environments(load_json(ENVIRONMENTS), base):
            bundle = prepare(manifest, lambda request: backend_document(
                [str(executable("resina-theme-resolve")), "-"], request, "schemas/headless-result.schema.json", name, arguments.timeout,
            ), environment=environment)
            viewport = copy.deepcopy(bundle["capture"])
            factor = environment["scale"] / viewport["pixelsPerUnit"]
            viewport.update(width=math.ceil(viewport["width"] * factor), height=math.ceil(viewport["height"] * factor), pixelsPerUnit=environment["scale"])
            for scene in bundle["scenarios"]:
                if scene["kind"] != "surfacePaint":
                    continue
                output = arguments.output / name / scene["name"]
                capture_scene(scene, viewport, executable("resina-surface-paint"), executable("resina-paint-raster"), output, "candidate", arguments.timeout)
                captured += 1
            print(f"Captured {name}: 16 complete material candidates", flush=True)
        viewport = {"origin": {"x": -6, "y": -6}, "width": 32, "height": 26, "pixelsPerUnit": 1, "samplesPerAxis": 4}
        for scene in motion_scenes():
            output = arguments.output / "motion" / scene["name"]
            capture_scene(scene, viewport, executable("resina-command-motion"), executable("resina-command-motion-raster"), output, "diagnostic", arguments.timeout)
            captured += 1
    except (AssertionError, OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"FAIL material capture: {error}", file=sys.stderr)
        return 1
    print(f"Captured {captured} repeated records. No visual baseline has been accepted.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
