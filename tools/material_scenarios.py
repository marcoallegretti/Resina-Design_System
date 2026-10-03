import argparse
import copy
import json
import math
import sys

from check_headless_backend import run_backend
from check_schemas import ROOT, load_json, parse_json, validator_for


MANIFEST = ROOT / "conformance/scenes/tier0-materials.json"


def validate(schema, document, label):
    errors = list(validator_for(schema).iter_errors(document))
    if errors:
        raise ValueError(f"{label}: {errors[0].message}")


def asset_source(path):
    source_path = (ROOT / path).resolve()
    if not source_path.is_relative_to(ROOT.resolve()):
        raise ValueError(f"scene asset escapes repository: {path}")
    source = source_path.read_text(encoding="utf-8")
    parse_json(source, path)
    return source


def backend_document(command, request, schema, label, timeout):
    completed = run_backend(command, json.dumps(request, ensure_ascii=False, allow_nan=False), timeout)
    if completed.returncode != 0 or completed.stderr:
        raise ValueError(f"{label}: backend exit {completed.returncode}, stderr {completed.stderr[:200]!r}")
    result = parse_json(completed.stdout, label)
    validate(schema, result, label)
    return result


def prepare(manifest, resolve_theme):
    validate("schemas/material-scene-manifest.schema.json", manifest, "material scenes")
    names = set()
    for scene in manifest["scenarios"]:
        if scene["name"] in names:
            raise ValueError(f"duplicate material scene: {scene['name']}")
        names.add(scene["name"])
        if scene["theme"] not in manifest["themes"]:
            raise ValueError(f"{scene['name']}: unknown theme {scene['theme']}")
        states = scene["surface"]["states"]["states"]
        if scene["kind"] == "opaqueSurface" and states != ["rest"]:
            raise ValueError(f"{scene['name']}: opaque scene requires rest")
        if scene["kind"] == "focusRing" and "focused" not in states:
            raise ValueError(f"{scene['name']}: focus scene requires focused")
    environment = parse_json(asset_source(manifest["environmentSource"]), "scene environment")
    appearance = parse_json(asset_source(manifest["appearanceSource"]), "scene appearance")
    validate("schemas/environment.schema.json", environment, "scene environment")
    validate("schemas/opaque-surface-appearance.schema.json", appearance, "scene appearance")
    themes = {}
    for name, theme in manifest["themes"].items():
        request = {
            "schemaVersion": "0.1.0",
            "themeSource": asset_source(theme["source"]),
            "externalSources": {key: asset_source(path) for key, path in theme["externalSources"].items()},
            "environment": copy.deepcopy(environment),
        }
        validate("schemas/theme-resolution-request.schema.json", request, name)
        resolved = resolve_theme(request)
        validate("schemas/headless-result.schema.json", resolved, name)
        themes[name] = request, resolved
    prepared = []
    for scene in manifest["scenarios"]:
        theme, resolved = themes[scene["theme"]]
        if resolved["materials"][scene["surface"]["materialRole"]] != scene["expectedMaterialFamily"]:
            raise ValueError(f"{scene['name']}: declared material family differs from resolved role")
        surrounding = resolved["opaqueColorFallbacks"][scene["surroundingColorRole"]]
        request = {
            "schemaVersion": "0.1.0", "theme": copy.deepcopy(theme),
            "surface": copy.deepcopy(scene["surface"]), "size": copy.deepcopy(manifest["size"]),
        }
        if scene["kind"] == "opaqueSurface":
            request.update({
                "appearance": copy.deepcopy(appearance), "foregroundRole": scene["foregroundRole"],
                "adjacentColor": copy.deepcopy(surrounding),
                "minimumContentContrast": scene["minimumContentContrast"],
                "minimumEdgeContrast": scene["minimumEdgeContrast"],
            })
            if scene["expectedMaterialFamily"] == "frost":
                request["postTreatmentBackdrop"] = copy.deepcopy(surrounding)
            schema = "schemas/opaque-surface-request.schema.json"
        else:
            request.update({
                "shapeAssignments": copy.deepcopy(appearance["shapeAssignments"]),
                "depthAssignments": copy.deepcopy(appearance["depthAssignments"]),
                "keyLight": copy.deepcopy(appearance["keyLight"]),
                "surroundingColor": copy.deepcopy(surrounding),
            })
            schema = "schemas/focus-ir-request.schema.json"
        validate(schema, request, scene["name"])
        prepared.append({key: scene[key] for key in ("name", "kind", "expectedMaterialFamily")} | {"request": request})
    return {"schemaVersion": "0.1.0", "capture": copy.deepcopy(manifest["capture"]), "scenarios": prepared}


def main():
    parser = argparse.ArgumentParser(description="Prepare portable static material scene requests using a theme backend.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        parser.error("provide a theme backend command and a positive finite timeout")
    try:
        prepared = prepare(load_json(MANIFEST), lambda request: backend_document(
            command, request, "schemas/headless-result.schema.json", "scene theme", arguments.timeout,
        ))
    except (AssertionError, OSError, ValueError, KeyError) as error:
        print(f"FAIL material scenes: {error}", file=sys.stderr)
        return 1
    sys.stdout.buffer.write((json.dumps(prepared, ensure_ascii=False, allow_nan=False, indent=2) + "\n").encode("utf-8"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
