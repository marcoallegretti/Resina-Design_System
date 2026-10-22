import copy
import json
import math

from check_schemas import ROOT, load_json

OPAQUE_SURFACE_EXPECTED = "conformance/ir/opaque-surface-expected.json"
FOCUS_EXPECTED = "conformance/ir/focus-ir-expected.json"
FOCUS_MINIMUM = 3.0
TRANSLUCENT = ("shapedBackdrop", "regularBackdrop", "translucentPigmented")


def linear(channel):
    if channel <= 0.04045:
        return channel / 12.92
    return ((channel + 0.055) / 1.055) ** 2.4


def luminance(components):
    red, green, blue = (linear(channel) for channel in components)
    return 0.2126 * red + 0.7152 * green + 0.0722 * blue


def luminance_ratio(first, second):
    return (max(first, second) + 0.05) / (min(first, second) + 0.05)


def ratio(foreground, background):
    return luminance_ratio(luminance(foreground), luminance(background))


def bound(foreground, lower, upper):
    own = luminance(foreground)
    closest = min(max(own, luminance(lower)), luminance(upper))
    return luminance_ratio(own, closest)


def color(components, alpha=1.0):
    return {"colorSpace": "srgb", "components": list(components), "alpha": alpha}


def over(body, alpha, backdrop):
    return [channel * alpha + under * (1.0 - alpha) for channel, under in zip(body, backdrop)]


def select_representation(environment):
    preferences = environment["accessibilityPreferences"]
    capabilities = environment["rendererCapabilities"]
    quality = environment["qualityPolicy"]
    if (preferences["reducedTransparency"] or preferences["highContrast"]
            or not capabilities["translucentSurfaces"]):
        return "opaqueDimensional"
    if quality == "economy":
        return "translucentPigmented"
    if capabilities["backdropEffect"] and capabilities["backdropBlur"]:
        if capabilities["shapedBackdrop"] and quality == "full":
            return "shapedBackdrop"
        return "regularBackdrop"
    return "translucentPigmented"


def _token(theme, path):
    node = theme["tokens"]
    for segment in path.split("."):
        node = node[segment]
    value = node["$value"]
    if value["colorSpace"] != "srgb":
        raise ValueError("the independent oracle resolves sRGB tokens only")
    return value["components"], value.get("alpha", 1)


def _role(theme, assignments, role):
    components, alpha = _token(theme, theme[assignments]["roles"][role])
    return components, alpha


def _family(theme, material_role):
    kind, name = material_role.split(".")
    return theme["materialAssignments"][kind][name]


def _range(document):
    lower, upper = document["lower"], document["upper"]
    for bound_color in (lower, upper):
        if bound_color["alpha"] != 1:
            raise ValueError("range bound must be opaque")
    if any(low > high for low, high in zip(lower["components"], upper["components"])):
        raise ValueError("lower exceeds upper")
    return lower["components"], upper["components"]


def _worst(components, ranges):
    return min(bound(components, lower, upper) for lower, upper in ranges)


def _select(candidates, ranges, minimum, label):
    ratios = []
    for role, components in candidates:
        value = _worst(components, ranges)
        ratios.append(value)
        if value >= minimum:
            return role, components, value, role != candidates[0][0]
    raise ValueError(f"{label} contrast is insufficient: {ratios}")


def _srgb_token_color(theme, assignments, role):
    components, alpha = _role(theme, assignments, role)
    return color(components, alpha)


def _binding(request, theme, representation, portable_body, opaque_body):
    surface = request["surface"]
    role = surface["colorRole"]
    source = _srgb_token_color(theme, "colorAssignments", role)
    states = [state for state in ("rest", "focused") if state in surface["states"]["states"]]
    return {
        "schemaVersion": "0.4.0",
        "materialRole": surface["materialRole"],
        "colorRole": role,
        "materialFamily": "frost",
        "sourceColor": source,
        "colorFallback": source,
        "opaqueColorFallback": color(opaque_body),
        "frostPortableBody": portable_body,
        "form": surface["form"],
        "states": {"schemaVersion": "0.1.0", "states": states},
        "treatmentStack": surface["treatmentStack"],
        "frostRepresentation": representation,
    }


def _check_request(request):
    if request["schemaVersion"] != "0.1.0":
        raise ValueError("schemaVersion must be 0.1.0")
    surface = request["surface"]
    theme = json.loads(request["theme"]["themeSource"])
    if _family(theme, surface["materialRole"]) != "frost":
        raise ValueError("surface material family must be Frost")
    if any(state not in ("rest", "focused") for state in surface["states"]["states"]):
        raise ValueError("supports only rest and focused")
    if surface["treatmentStack"]["treatments"][-1] != "none":
        raise ValueError("treatment must be none")
    radius = request["diffusion"]["radius"]
    if not math.isfinite(radius) or radius <= 0:
        raise ValueError("diffusion radius must be finite and positive")
    for name in ("minimumContentContrast", "minimumEdgeContrast"):
        if not 1 <= request[name] <= 21:
            raise ValueError(f"{name} must be finite and in [1, 21]")
    if not request["adjacentRanges"]:
        raise ValueError("adjacent ranges must not be empty")
    if "surroundingRanges" in request and not request["surroundingRanges"]:
        raise ValueError("surrounding ranges must not be empty")
    focused = "focused" in surface["states"]["states"]
    if focused and "surroundingRanges" not in request:
        raise ValueError("focused surface requires surrounding ranges")
    return theme


def expected_result(request):
    theme = _check_request(request)
    environment = request["theme"]["environment"]
    surface = request["surface"]
    role = surface["colorRole"]
    representation = select_representation(environment)
    foreground, foreground_alpha = _role(theme, "colorAssignments", request["foregroundRole"])
    if foreground_alpha != 1:
        raise ValueError("foreground role must resolve to an opaque color")
    opaque_body, _ = _role(theme, "opaqueColorAssignments", role)
    portable, portable_alpha = _role(theme, "colorAssignments", role)
    tint = theme["frostPigment"]["tintStrength"]
    alpha = portable_alpha * tint
    minimum = request["minimumContentContrast"]
    backdrop = _range(request["backdrop"])
    adjacent = [_range(item) for item in request["adjacentRanges"]]
    surrounding = [_range(item) for item in request.get("surroundingRanges", [])]
    focused = "focused" in surface["states"]["states"]

    certified = None
    reason = None
    if representation == "opaqueDimensional":
        reason = "selection"
    else:
        composited = [over(portable, alpha, edge) for edge in backdrop]
        certified = bound(foreground, *composited)
        if certified < minimum:
            fallback = ratio(foreground, opaque_body)
            if fallback < minimum:
                raise ValueError(
                    f"content contrast is insufficient: {certified}, opaque {fallback}"
                )
            representation, reason, certified = "opaqueDimensional", "legibility", None
    if representation == "opaqueDimensional":
        content = ratio(foreground, opaque_body)
        if content < minimum:
            raise ValueError(f"content contrast is insufficient: opaque {content}")
        body, body_alpha = opaque_body, 1.0
    else:
        body, body_alpha = portable, alpha

    outline, _ = _role(theme, "opaqueColorAssignments", "outline")
    strong, _ = _role(theme, "opaqueColorAssignments", "outline.strong")
    edge_role, edge_color, edge_ratio, edge_fallback = _select(
        (("outline", outline), ("outline.strong", strong)),
        adjacent, request["minimumEdgeContrast"], "edge",
    )
    focus = None
    if focused:
        focus_color, _ = _role(theme, "opaqueColorAssignments", "focus")
        focus_role, focus_value, focus_ratio, focus_fallback = _select(
            (("focus", focus_color), ("outline.strong", strong)),
            surrounding, FOCUS_MINIMUM, "focus",
        )
        focus = (focus_role, focus_value, focus_ratio, focus_fallback)

    profile = request["appearance"]["pigmentProfiles"]["profiles"]["frost"]
    side = [channel * (1.0 - profile["sideShade"]) for channel in body]
    highlight = [profile["highlightLift"] + channel * (1.0 - profile["highlightLift"])
                 for channel in body]
    pigment = {
        "schemaVersion": "0.1.0",
        "materialFamily": "frost",
        "profile": profile,
        "body": color(body, body_alpha),
        "side": color(side, body_alpha),
        "highlight": color(highlight, body_alpha),
    }
    edge = {
        "schemaVersion": "0.1.0",
        "colorRole": edge_role,
        "color": color(edge_color),
        "contrastRatio": edge_ratio,
        "fallbackApplied": edge_fallback,
    }
    shape = load_json(ROOT / OPAQUE_SURFACE_EXPECTED)
    states = {
        "schemaVersion": "0.1.0",
        "states": [state for state in ("rest", "focused") if state in surface["states"]["states"]],
    }
    identity = {
        "materialRole": surface["materialRole"],
        "colorRole": role,
        "materialFamily": "frost",
        "form": surface["form"],
        "states": states,
    }
    focus_ir = None
    if focus is not None:
        focus_ir = load_json(ROOT / FOCUS_EXPECTED)
        preliminary = _binding(
            request, theme, select_representation(environment),
            color(portable, alpha) if select_representation(environment) != "opaqueDimensional"
            else color(opaque_body), opaque_body,
        )
        focus_ir["indicator"].update({
            "binding": preliminary,
            "colorRole": focus[0],
            "color": color(focus[1]),
            "contrastRatio": focus[2],
            "fallbackApplied": focus[3],
        })
    result = {"schemaVersion": "0.1.0", "frostRepresentation": representation}
    if representation == "opaqueDimensional":
        surface_ir = {
            "schemaVersion": "0.1.0",
            "representation": "opaque",
            **identity,
            "frostRepresentation": "opaqueDimensional",
            "geometry": shape["geometry"],
            "pigment": pigment,
            "lighting": shape["lighting"],
            "edgeWidth": shape["edgeWidth"],
            "highlightWidth": shape["highlightWidth"],
            "edge": edge,
            "foregroundRole": request["foregroundRole"],
            "foreground": color(foreground),
            "contentContrastRatio": ratio(foreground, opaque_body),
            "contentFallbackApplied": reason == "legibility",
        }
        paint = {"schemaVersion": "0.1.0", "body": surface_ir}
        if focus_ir is not None:
            paint["focus"] = focus_ir
        result.update({"appearance": "opaque", "fallbackReason": reason, "paint": paint})
        return result
    translucent = {
        **identity,
        "geometry": shape["geometry"],
        "pigment": pigment,
        "lighting": shape["lighting"],
        "edgeWidth": shape["edgeWidth"],
        "highlightWidth": shape["highlightWidth"],
        "edge": edge,
        "foregroundRole": request["foregroundRole"],
        "foreground": color(foreground),
        "backdrop": {
            "range": request["backdrop"],
            "compositedBody": {
                "lower": color(composited[0]),
                "upper": color(composited[1]),
            },
        },
        "contentContrastRatio": certified,
    }
    if representation != "translucentPigmented":
        translucent["diffusion"] = {"radius": request["diffusion"]["radius"]}
    result.update({"appearance": "translucent", "body": translucent})
    if focus_ir is not None:
        result["focus"] = focus_ir
    return result


def remove_members(document, pointers):
    result = copy.deepcopy(document)
    for pointer in pointers:
        segments = [part.replace("~1", "/").replace("~0", "~") for part in pointer[1:].split("/")]
        parent = result
        for segment in segments[:-1]:
            parent = parent[int(segment)] if isinstance(parent, list) else parent[segment]
        del parent[segments[-1]]
    return result
