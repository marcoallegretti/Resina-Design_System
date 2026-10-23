"""Independent peak visibility of the static key light on the lit Command and Toggle part bodies.

The table in conformance/materials/lit-control-visibility.json is recomputed from the authored definitions
with the independent lit body arithmetic. Generation additionally asks a theme backend for the resolved
rest colors; every other number is derived here.
"""
import argparse
import itertools
import json
import math
import sys

from check_command_paint_backend import oklab
from check_headless_backend import run_backend
from check_schemas import ROOT, load_json, parse_json, validator_for
from lit_body_arithmetic import Body

TABLE = ROOT / "conformance/materials/lit-control-visibility.json"
STEP = 0.25
REFINED_STEP = 1e-9
FAMILIES = ("cast", "frost", "elastomer")
LOGICAL = ("topStart", "topEnd", "bottomEnd", "bottomStart")
PARTS = ("commandBody", "toggleTrack", "toggleThumb")
SIZES = {
    "commandBody": {"width": 96, "height": 48},
    "toggleTrack": {"width": 64.5, "height": 40.5},
    "toggleThumb": {"width": 16, "height": 16},
}
THEMES = {"light": "tokens/themes/light.json", "dark": "tokens/themes/dark.json"}
TOLERANCE = 1e-9
POINT_TOLERANCE = 1e-6


def components():
    return load_json(ROOT / "definitions/components/command.json"), load_json(ROOT / "definitions/components/toggle.json")


def bodies():
    """The anatomy-bound bodies as (part, variant, checked, material slot, color role, content role, shape)."""
    command, toggle = components()
    result = []
    for variant in ("standard", "primary"):
        body = command["variants"][variant]["body"]
        result.append(("commandBody", variant, None, body["materialRole"].split(".")[1], body["colorRole"], body["contentRole"], body["shape"]))
    for part, name in (("toggleTrack", "track"), ("toggleThumb", "thumb")):
        body = toggle[name]
        for checked in (False, True):
            role = body["checkedColorRole"] if checked else body["colorRole"]
            result.append((part, name, checked, body["materialRole"].split(".")[1], role, body["contentRole"], body["shape"]))
    return result


def radius_of(part, shape, appearance, foundation):
    size = SIZES[part]
    profile = appearance["shapeAssignments"]["profiles"][shape]
    if profile["kind"] == "capsule":
        return min(size["width"], size["height"]) / 2
    if profile["kind"] != "uniform":
        raise ValueError(f"{part}: the anatomy binds a uniform or capsule shape")
    group, name = profile["radius"].split(".")
    return foundation[group][name]["$value"]["value"]


def source(part, shape, family, body, appearance, foundation, relief, environment):
    radius = radius_of(part, shape, appearance, foundation)

    def color(components):
        return {"colorSpace": "srgb", "components": list(components), "alpha": 1}

    return {
        "identity": {
            "materialRole": "control.interactive", "colorRole": "surface.high", "materialFamily": family,
            "form": {"shape": shape, "elevation": "raised"}, "states": {"schemaVersion": "0.1.0", "states": ["rest"]},
            "foregroundRole": "content.primary", "contentFallbackApplied": False,
            **({"frostRepresentation": "opaqueDimensional"} if family == "frost" else {}),
        },
        "size": SIZES[part],
        "radii": {name: {"x": radius, "y": radius} for name in LOGICAL},
        "relief": relief[part],
        "body": color(body),
        "pigmentProfiles": appearance["pigmentProfiles"],
        "keyLight": appearance["keyLight"],
        "light": {"ambient": None, "interaction": None},
        "environment": environment,
        "edge": {"schemaVersion": "0.1.0", "outline": color((0, 0, 0)), "outlineStrong": color((0, 0, 0)),
                 "adjacentColor": color((1, 1, 1)), "minimumContrast": 1},
        "edgeWidth": 1,
        "foreground": color((0, 0, 0)),
        "minimumContentContrast": 1,
        "depth": 4,
        "points": [],
    }


def response_at(geometry, point):
    direction = geometry.light["keyDirection"]
    normal = geometry.normal(point)
    return normal[0] * direction["x"] + normal[1] * direction["y"]


def refine(geometry, point, sign):
    """Pattern search from a grid point to the local extremum of the signed key response."""
    best, value = point, sign * response_at(geometry, point)
    step = STEP
    while step > REFINED_STEP:
        moved = False
        for dx, dy in itertools.product((-step, 0.0, step), repeat=2):
            candidate = (best[0] + dx, best[1] + dy)
            if not (0 <= candidate[0] <= geometry.width and 0 <= candidate[1] <= geometry.height):
                continue
            response = sign * response_at(geometry, candidate)
            if response > value:
                best, value, moved = candidate, response, True
        if not moved:
            step /= 2
    return best, sign * value


def peaks(geometry):
    """The greatest positive and most negative signed key response of one body.

    The sample grid locates the extremum and a pattern search converges on it.
    """
    best, worst = (0.0, None), (0.0, None)
    for column in range(int(geometry.width / STEP) + 1):
        for row in range(int(geometry.height / STEP) + 1):
            point = (column * STEP, row * STEP)
            response = response_at(geometry, point)
            if response > best[0]:
                best = (response, point)
            if response < worst[0]:
                worst = (response, point)
    (high_point, high), (low_point, low) = refine(geometry, best[1], 1), refine(geometry, worst[1], -1)
    return (high, high_point), (low, low_point)


def delta(first, second):
    return math.dist(oklab(first), oklab(second))


def witness(point, response):
    return {"point": {"x": point[0], "y": point[1]}, "response": response}


def row_deltas(geometry, highlight, shading, body):
    colors = [geometry.face_color((entry["point"]["x"], entry["point"]["y"])) for entry in (highlight, shading)]
    return [delta(body, color["components"]) for color in colors]


def inputs():
    appearance = load_json(ROOT / "definitions/tier0-surface-appearance.json")
    foundation = load_json(ROOT / "tokens/foundation.json")
    relief = load_json(ROOT / "definitions/control-relief.json")
    environment = load_json(ROOT / "conformance/environment/valid-minimal-capabilities.json")
    return appearance, foundation, relief, environment


def geometries(appearance, foundation, relief, environment):
    """One body per part, whose normals alone depend on the part geometry, the relief and the key light."""
    result = {}
    for part, _, _, _, _, _, shape in bodies():
        if part not in result:
            result[part] = Body(source(part, shape, "cast", (0.5, 0.5, 0.5), appearance, foundation, relief, environment))
    return result


def build(resolved):
    """The complete table from resolved rest colors, keyed by theme, material family and color role."""
    appearance, foundation, relief, environment = inputs()
    parts = {}
    for part, geometry in geometries(appearance, foundation, relief, environment).items():
        (high, high_point), (low, low_point) = peaks(geometry)
        parts[part] = {"size": SIZES[part], "highlight": witness(high_point, high), "shading": witness(low_point, low)}
    rows = []
    for theme in THEMES:
        for family in FAMILIES:
            for part, variant, checked, slot, role, _, shape in bodies():
                body = resolved[theme][family][role]
                geometry = Body(source(part, shape, family, body, appearance, foundation, relief, environment))
                highlight, shading = row_deltas(geometry, parts[part]["highlight"], parts[part]["shading"], body)
                row = {"theme": theme, "materialFamily": family, "part": part, "variant": variant, "colorRole": role, "body": list(body),
                       "highlightDeltaE": highlight, "shadingDeltaE": shading}
                if checked is not None:
                    row["checked"] = checked
                rows.append(row)
    return {"schemaVersion": "0.1.0", "keyLight": appearance["keyLight"]["direction"], "sampleStep": STEP, "parts": parts, "rows": rows}


def resolve_colors(command, timeout):
    """Resolved opaque rest colors per theme, family and role from a theme backend."""
    appearance, _, _, environment = inputs()
    resolved = {}
    roles = {role for _, _, _, _, role, _, _ in bodies()}
    for theme, path in THEMES.items():
        source_text = (ROOT / path).read_text(encoding="utf-8")
        foundation = (ROOT / "tokens/foundation.json").read_text(encoding="utf-8")
        resolved[theme] = {}
        for family in FAMILIES:
            document = parse_json(source_text, path)
            document["materialAssignments"]["control"] = {"passive": family, "interactive": family, "primary": family}
            request = {"schemaVersion": "0.1.0", "themeSource": json.dumps(document, allow_nan=False),
                       "externalSources": {"foundation.json": foundation}, "environment": environment}
            completed = run_backend(command, json.dumps(request, allow_nan=False), timeout)
            if completed.returncode != 0 or completed.stderr:
                raise ValueError(f"theme backend failed for {theme}: exit {completed.returncode}")
            fallbacks = parse_json(completed.stdout, "theme resolution")["opaqueColorFallbacks"]
            resolved[theme][family] = {role: fallbacks[role]["components"] for role in roles}
    return resolved


def check(table):
    errors = list(validator_for("schemas/lit-control-visibility.schema.json").iter_errors(table))
    if errors:
        raise ValueError(f"visibility table: {errors[0].message}")
    resolved = {theme: {family: {} for family in FAMILIES} for theme in THEMES}
    for row in table["rows"]:
        previous = resolved[row["theme"]][row["materialFamily"]].setdefault(row["colorRole"], row["body"])
        if previous != row["body"]:
            raise ValueError(f"{row['theme']} {row['materialFamily']} {row['colorRole']}: conflicting bodies")
    required = {role for _, _, _, _, role, _, _ in bodies()}
    for theme in resolved.values():
        for roles in theme.values():
            if set(roles) != required:
                raise ValueError("visibility table does not cover every theme, family and color role")
    expected = build(resolved)
    difference = first_difference(table, expected)
    if difference:
        raise ValueError(f"visibility table differs from the independent arithmetic at {difference}")
    return len(table["rows"])


def first_difference(actual, expected, path=""):
    if isinstance(expected, dict):
        if not isinstance(actual, dict) or set(actual) != set(expected):
            return path or "/"
        for key in expected:
            found = first_difference(actual[key], expected[key], f"{path}/{key}")
            if found:
                return found
        return None
    if isinstance(expected, list):
        if not isinstance(actual, list) or len(actual) != len(expected):
            return path or "/"
        for index, (first, second) in enumerate(zip(actual, expected)):
            found = first_difference(first, second, f"{path}/{index}")
            if found:
                return found
        return None
    if isinstance(expected, bool) or isinstance(expected, str) or expected is None:
        return None if actual == expected else path
    if isinstance(actual, bool) or not isinstance(actual, (int, float)):
        return path
    tolerance = POINT_TOLERANCE if "/point/" in path else TOLERANCE
    return None if math.isclose(actual, expected, rel_tol=tolerance, abs_tol=tolerance) else path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--generate", action="store_true",
                        help="write the table to stdout from the resolved colors of the theme backend command")
    parser.add_argument("backend", nargs=argparse.REMAINDER, help="the theme backend command, after --")
    arguments = parser.parse_args()
    try:
        if arguments.generate:
            command = arguments.backend[1:] if arguments.backend[:1] == ["--"] else arguments.backend
            if not command:
                parser.error("supply a theme backend command after --")
            table = build(resolve_colors(command, 30))
            sys.stdout.write(json.dumps(table, indent=2, allow_nan=False) + "\n")
            return 0
        rows = check(load_json(TABLE))
    except (AssertionError, OSError, ValueError, KeyError) as error:
        print(f"FAIL lit control visibility: {error}", file=sys.stderr)
        return 1
    print(f"Lit control visibility reproduced {rows} rows from the independent arithmetic")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
