import argparse
import math
import sys

from check_schemas import load_json
from material_scenarios import MANIFEST, backend_document, prepare


def check_scene(scene, result, capture):
    request = scene["request"]
    binding = result if scene["kind"] == "opaqueSurface" else result["indicator"]["binding"]
    for field in ("materialRole", "colorRole", "form"):
        if binding[field] != request["surface"][field]:
            raise ValueError(f"{scene['name']}: resolved {field} differs from scene")
    if set(binding["states"]["states"]) != set(request["surface"]["states"]["states"]):
        raise ValueError(f"{scene['name']}: resolved states differ from scene")
    if binding["materialFamily"] != scene["expectedMaterialFamily"]:
        raise ValueError(f"{scene['name']}: resolved material family differs from scene")
    if scene["kind"] == "opaqueSurface":
        if result["foregroundRole"] != request["foregroundRole"]:
            raise ValueError(f"{scene['name']}: resolved foreground role differs from scene")
        for actual, minimum in (
            (result["contentContrastRatio"], request["minimumContentContrast"]),
            (result["edge"]["contrastRatio"], request["minimumEdgeContrast"]),
        ):
            if actual < minimum and not math.isclose(actual, minimum, rel_tol=0, abs_tol=1e-12):
                raise ValueError(f"{scene['name']}: resolved contrast falls below scene guard")
        bounds = result["geometry"]["silhouette"]["bounds"]
        x, y = bounds["x"], bounds["y"]
    else:
        outer = result["geometry"]["outer"]
        bounds = outer["contour"]["bounds"]
        x, y = bounds["x"] + outer["offset"]["x"], bounds["y"] + outer["offset"]["y"]
    left, top = capture["origin"]["x"], capture["origin"]["y"]
    right = left + capture["width"] / capture["pixelsPerUnit"]
    bottom = top + capture["height"] / capture["pixelsPerUnit"]
    if not all(math.isfinite(value) for value in (right, bottom)):
        raise ValueError(f"{scene['name']}: capture extent is nonfinite")
    if any(
        actual > limit and not math.isclose(actual, limit, rel_tol=1e-12, abs_tol=1e-12)
        for actual, limit in ((left, x), (top, y), (x + bounds["width"], right), (y + bounds["height"], bottom))
    ):
        raise ValueError(f"{scene['name']}: capture clips required paint bounds")


def main():
    parser = argparse.ArgumentParser(description="Resolve every authored material scene and check semantic bindings and complete capture bounds.")
    parser.add_argument("--theme-backend", required=True)
    parser.add_argument("--surface-backend", required=True)
    parser.add_argument("--focus-backend", required=True)
    parser.add_argument("--timeout", type=float, default=30)
    arguments = parser.parse_args()
    if not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        parser.error("timeout must be finite and positive")
    try:
        bundle = prepare(load_json(MANIFEST), lambda request: backend_document(
            [arguments.theme_backend, "-"], request, "schemas/headless-result.schema.json", "scene theme", arguments.timeout,
        ))
        for scene in bundle["scenarios"]:
            if scene["kind"] == "opaqueSurface":
                command, schema = arguments.surface_backend, "schemas/opaque-surface-ir.schema.json"
            else:
                command, schema = arguments.focus_backend, "schemas/focus-indicator-ir.schema.json"
            result = backend_document([command, "-"], scene["request"], schema, scene["name"], arguments.timeout)
            check_scene(scene, result, bundle["capture"])
    except (AssertionError, OSError, ValueError, KeyError) as error:
        print(f"FAIL material scenes: {error}", file=sys.stderr)
        return 1
    print(f"Material scenarios passed {len(bundle['scenarios'])} resolved scenes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
