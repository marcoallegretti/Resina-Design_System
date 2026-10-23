"""Prepare the lit Command and Toggle acceptance scenes and derive their expected pixels independently.

The catalog names authored parts, states, light and environments. Requests are assembled from the authored
definitions and the colors a theme backend resolves; the expected paint of every scene comes from the
independent lit control body arithmetic and not from the reference resolver.
"""
import argparse
import copy
import json
import math
import sys

from check_command_paint_backend import control_body_source, mix_body, phase_of
from check_schemas import ROOT, load_json, parse_json
from check_toggle_part_paint_backend import RELIEF_PARTS, selected_role
from lit_body_arithmetic import contrast, pixels
from material_scenarios import asset_source, backend_document, validate

MANIFEST = ROOT / "conformance/scenes/lit-control-scenes.json"
KINDS = {"command": "commandPaint", "toggle": "togglePartPaint"}
LOGICAL = ("topStart", "topEnd", "bottomEnd", "bottomStart")
FOCUS_GAP = 2
FOCUS_STROKE = 2
FOCUS_CONTRAST = 3


class Resolution:
    """Resolved colors, logical radii and semantic depth of one scene's theme and environment."""

    def __init__(self, colors, foundation, appearance):
        self.colors = colors
        self.foundation = foundation
        self.appearance = appearance

    def color(self, request, role):
        return self.colors[role]["components"]

    def token(self, path):
        group, name = path.split(".")
        return self.foundation[group][name]["$value"]["value"]

    def radii(self, request):
        body = request["surface"]["body"]
        size = body["size"]
        profile = self.appearance["shapeAssignments"]["profiles"][body["surface"]["form"]["shape"]]
        if profile["kind"] == "capsule":
            values = dict.fromkeys(LOGICAL, min(size["width"], size["height"]) / 2)
        elif profile["kind"] == "uniform":
            values = dict.fromkeys(LOGICAL, self.token(profile["radius"]))
        else:
            values = {name: self.token(profile["radii"][name]) for name in LOGICAL}
        return {name: {"x": value, "y": value} for name, value in values.items()}

    def depth(self, request):
        elevation = request["surface"]["body"]["surface"]["form"]["elevation"]
        return self.token(self.appearance["depthAssignments"]["roles"][elevation])


def environment_for(manifest, base, name):
    settings = manifest["environments"][name]
    environment = copy.deepcopy(base)
    environment["accessibilityPreferences"].update(highContrast=settings["highContrast"], reducedMotion=settings["reducedMotion"])
    environment["rendererCapabilities"]["dynamicLighting"] = settings["dynamicLighting"]
    environment["qualityPolicy"] = settings["qualityPolicy"]
    environment["layoutDirection"] = settings["layoutDirection"]
    validate("schemas/environment.schema.json", environment, name)
    return environment


def surface_state(scene):
    states = list(scene["states"])
    if scene.get("checked"):
        states.append("checked")
    return states


def part_definition(manifest, scene, anatomy):
    """The anatomy-bound surface part of a scene: (size key, definition, relief part, content role)."""
    if scene["component"] == "command":
        body = anatomy["command"]["variants"][scene["variant"]]["body"]
        return "command", body, "commandBody", body["contentRole"]
    part = anatomy["toggle"][scene["part"]]
    return "toggle" + scene["part"].capitalize(), part, RELIEF_PARTS[scene["part"]], part["contentRole"]


def prepare(manifest, resolve_theme):
    """Complete scene requests with their independent resolution, in catalog order."""
    validate("schemas/lit-control-scene-manifest.schema.json", manifest, "lit control scenes")
    names = set()
    for scene in manifest["scenarios"]:
        if scene["name"] in names:
            raise ValueError(f"duplicate lit control scene: {scene['name']}")
        names.add(scene["name"])
        for key, group in (("theme", "themes"), ("light", "lights"), ("environment", "environments")):
            if scene[key] not in manifest[group]:
                raise ValueError(f"{scene['name']}: unknown {key} {scene[key]}")
        if (scene["component"] == "command") != ("variant" in scene) or (scene["component"] == "toggle") != ("part" in scene):
            raise ValueError(f"{scene['name']}: a Command names a variant and a Toggle part")
        if scene["component"] == "command" and "checked" in scene or scene.get("part") == "thumb" and "focused" in scene["states"]:
            raise ValueError(f"{scene['name']}: a Command is never checked and a thumb draws no focus ring")
    base = parse_json(asset_source(manifest["environmentSource"]), "lit control base environment")
    appearance = parse_json(asset_source(manifest["appearanceSource"]), "lit control appearance")
    validate("schemas/opaque-surface-appearance.schema.json", appearance, "lit control appearance")
    relief = parse_json(asset_source(manifest["reliefSource"]), "lit control relief")
    anatomy = {name: parse_json(asset_source(path), name + " anatomy") for name, path in manifest["anatomy"].items()}
    foundation = parse_json(asset_source("tokens/foundation.json"), "foundation tokens")
    resolved_themes = {}
    prepared, resolutions = [], {}
    for scene in manifest["scenarios"]:
        theme = manifest["themes"][scene["theme"]]
        key = (scene["theme"], scene["environment"])
        environment = environment_for(manifest, base, scene["environment"])
        if key not in resolved_themes:
            request = {
                "schemaVersion": "0.1.0", "themeSource": asset_source(theme["source"]),
                "externalSources": {name: asset_source(path) for name, path in theme["externalSources"].items()},
                "environment": copy.deepcopy(environment),
            }
            validate("schemas/theme-resolution-request.schema.json", request, scene["name"])
            resolved = resolve_theme(request)
            validate("schemas/headless-result.schema.json", resolved, scene["name"])
            resolved_themes[key] = request, resolved
        theme_request, resolved = resolved_themes[key]
        resolution = Resolution(resolved["opaqueColorFallbacks"], foundation, appearance)
        size_key, definition, _, content_role = part_definition(manifest, scene, anatomy)
        material_role = definition["materialRole"]
        family = resolved["materials"][material_role]
        responses = parse_json(asset_source(theme["responseSource"]), "lit control responses")
        states = surface_state(scene)
        phase = phase_of(states)
        response = {"bodyMix": 0, "depthScale": 1} if phase == "rest" else responses["profiles"][family][phase]
        canvas = copy.deepcopy(resolved["opaqueColorFallbacks"][manifest["surroundingColorRole"]])
        adjacent = canvas
        if scene.get("part") == "thumb":
            track = anatomy["toggle"]["track"]
            track_role = track["checkedColorRole"] if scene.get("checked") else track["colorRole"]
            track_family = resolved["materials"][track["materialRole"]]
            track_response = {"bodyMix": 0, "depthScale": 1} if phase == "rest" else responses["profiles"][track_family][phase]
            adjacent = {"colorSpace": "srgb", "alpha": 1,
                        "components": mix_body(resolution.color(None, track_role), track_response["bodyMix"])}
        body = {
            "schemaVersion": "0.1.0",
            "theme": {**copy.deepcopy(theme_request), "environment": copy.deepcopy(environment)},
            "surface": {
                "schemaVersion": "0.2.0", "materialRole": material_role,
                "colorRole": definition["colorRole"],
                "form": {"schemaVersion": "0.1.0", "shape": definition["shape"], "elevation": definition["elevation"]},
                "states": {"schemaVersion": "0.1.0", "states": states},
                "treatmentStack": {"schemaVersion": "0.1.0", "treatments": ["none"]},
            },
            "size": copy.deepcopy(manifest["sizes"][size_key]),
            "appearance": copy.deepcopy(appearance),
            "foregroundRole": content_role,
            "postTreatmentBackdrop": copy.deepcopy(adjacent),
            "adjacentColor": copy.deepcopy(adjacent),
            "minimumContentContrast": 4.5 if scene["component"] == "command" else 1,
            "minimumEdgeContrast": 3,
        }
        surface = {"schemaVersion": "0.1.0", "body": body, "surroundingColor": copy.deepcopy(canvas)}
        light = copy.deepcopy(manifest["lights"][scene["light"]])
        if scene["component"] == "command":
            request = {"schemaVersion": "0.2.0", "surface": surface, "commandAppearance": copy.deepcopy(responses),
                       "light": light, "relief": copy.deepcopy(relief)}
            schema = "schemas/command-paint-request.schema.json"
        else:
            request = {"schemaVersion": "0.2.0", "part": scene["part"], "surface": surface,
                       "checkedColorRole": definition["checkedColorRole"],
                       "interactionAppearance": copy.deepcopy(responses), "light": light, "relief": copy.deepcopy(relief)}
            schema = "schemas/toggle-part-paint-request.schema.json"
        validate(schema, request, scene["name"])
        prepared.append({"name": scene["name"], "kind": KINDS[scene["component"]], "expectedMaterialFamily": family,
                         "request": request})
        resolutions[scene["name"]] = resolution, response
    return {"schemaVersion": "0.1.0", "capture": copy.deepcopy(manifest["capture"]), "scenarios": prepared}, resolutions


def body_source(request, resolution, response):
    """The independent lit control body source of a prepared scene request."""
    relief_part = RELIEF_PARTS[request["part"]] if "part" in request else "commandBody"
    role = selected_role(request) if "part" in request else None
    source = control_body_source(request, response, relief_part, role, resolution)
    if phase_of(request["surface"]["body"]["surface"]["states"]["states"]) == "disabled":
        source["minimumContentContrast"] = 1
    return source


def focus_ring(request, resolution):
    """The spec/37 ring color, gap and stroke: focus, else outline.strong, at 3:1 against the surround."""
    if "focused" not in request["surface"]["body"]["surface"]["states"]["states"] or request.get("part") == "thumb":
        return None
    surround = request["surface"]["surroundingColor"]["components"]
    for role in ("focus", "outline.strong"):
        color = resolution.color(request, role)
        if contrast(color, surround) >= FOCUS_CONTRAST:
            return color, FOCUS_GAP, FOCUS_STROKE
    raise ValueError("no focus color meets 3:1 against the surround")


def viewport(request, capture, scale, samples):
    """The physical viewport of a scene: the part, its sweep and any focus ring inside the capture margin."""
    size = request["surface"]["body"]["size"]
    margin = capture["margin"]
    return {"origin": {"x": -margin, "y": -margin},
            "width": math.ceil((size["width"] + 2 * margin) * scale),
            "height": math.ceil((size["height"] + 2 * margin) * scale),
            "pixelsPerUnit": scale, "samplesPerAxis": samples}


def expected_pixels(request, resolution, response, view):
    source = body_source(request, resolution, response)
    return pixels(source, view["width"], view["height"], (view["origin"]["x"], view["origin"]["y"]),
                  view["pixelsPerUnit"], view["samplesPerAxis"], ring=focus_ring(request, resolution))


def main():
    parser = argparse.ArgumentParser(description="Prepare the lit Command and Toggle acceptance scene requests using a theme backend.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command[1:] if arguments.command[:1] == ["--"] else arguments.command
    if not command or not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        parser.error("provide a theme backend command and a positive finite timeout")
    try:
        bundle, _ = prepare(load_json(MANIFEST), lambda request: backend_document(
            command, request, "schemas/headless-result.schema.json", "scene theme", arguments.timeout))
    except (AssertionError, OSError, ValueError, KeyError) as error:
        print(f"FAIL lit control scenes: {error}", file=sys.stderr)
        return 1
    sys.stdout.buffer.write((json.dumps(bundle, ensure_ascii=False, allow_nan=False, indent=2) + "\n").encode("utf-8"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
