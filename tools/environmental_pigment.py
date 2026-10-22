import copy
import math

from theme_derivation import (
    ACHROMATIC_CHROMA, THEMES, DerivationError, composed_tokens, contains_token, gamut_map,
    guard_pairs, legibility_guard, luminance, normalized_hue, oklch_from_srgb, token_color,
)

ADAPTED_GROUP = "adaptedColor"
CAST_ROLES = ("surface.base", "surface.low", "surface.high")
FROST_ROLE = "surface.chrome"
ADAPTABLE_ROLES = (*CAST_ROLES, FROST_ROLE)
MAX_CHROMA = 0.4


class PigmentError(DerivationError):
    pass


def is_number(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def check_profile(profile):
    for theme in THEMES:
        for role in ADAPTABLE_ROLES:
            ranges = profile["themes"][theme][role]
            hue, chroma = ranges["hue"], ranges["chroma"]
            if not (is_number(hue["min"]) and -180 <= hue["min"] <= 0
                    and is_number(hue["max"]) and 0 <= hue["max"] <= 180):
                raise PigmentError("invalid profile: hue range must contain 0 within [-180, 180]")
            if not all(is_number(chroma[bound]) and 0 <= chroma[bound] <= MAX_CHROMA
                       for bound in ("min", "max")):
                raise PigmentError("invalid profile: chroma bounds must be within [0, 0.4]")
            if chroma["min"] > chroma["max"]:
                raise PigmentError("invalid profile: chroma min exceeds chroma max")


def check_observation(observation):
    if observation is None:
        return
    color = observation["color"]["components"]
    for component in color:
        if not (is_number(component) and 0 <= component <= 1):
            raise PigmentError("invalid observation: color outside [0, 1]")
    if "range" not in observation:
        return
    lower = observation["range"]["lower"]["components"]
    upper = observation["range"]["upper"]["components"]
    if any(low > high for low, high in zip(lower, upper)):
        raise PigmentError("invalid observation: range lower exceeds upper")
    if any(not low <= value <= high for low, value, high in zip(lower, color, upper)):
        raise PigmentError("invalid observation: color outside its range")


def skipped_reason(request):
    preferences = request["accessibilityPreferences"]
    if preferences["highContrast"]:
        return "highContrast"
    if preferences["reducedTransparency"]:
        return "reducedTransparency"
    if "observation" not in request:
        return "noObservation"
    return None


def template_roles(theme, external_sources):
    tokens = composed_tokens(theme, external_sources)
    if contains_token(tokens.get(ADAPTED_GROUP)):
        raise PigmentError(f"template already defines the {ADAPTED_GROUP} group")
    assignments = theme["colorAssignments"]["roles"]
    overrides = theme["opaqueColorAssignments"]["roles"]
    colors, alphas, opaque = {}, {}, {}
    for role, path in assignments.items():
        colors[role], alphas[role] = token_color(tokens, path)
        if role in overrides:
            opaque[role], _ = token_color(tokens, overrides[role])
        elif alphas[role] == 1:
            opaque[role] = colors[role]
    for role in CAST_ROLES:
        if alphas[role] != 1:
            raise PigmentError(f"template role {role} must be opaque")
        if role in overrides:
            raise PigmentError(f"template role {role} must not have an opaque color assignment")
    if alphas[FROST_ROLE] == 1:
        raise PigmentError(f"template role {FROST_ROLE} must be translucent")
    if FROST_ROLE not in overrides:
        raise PigmentError(f"template role {FROST_ROLE} needs an opaque color assignment")
    return colors, alphas, opaque


def half_turn(degrees):
    wrapped = degrees % 360.0
    return wrapped - 360.0 if wrapped > 180.0 else wrapped


def clamp(value, lower, upper):
    return max(lower, min(upper, value))


def observed(color):
    lightness, chroma, hue = oklch_from_srgb(color)
    achromatic = chroma < ACHROMATIC_CHROMA
    return [lightness, chroma, 0.0 if achromatic else hue], achromatic


def select_target(template, ranges, observation, achromatic):
    lightness, _, hue = template
    offset = 0.0 if achromatic else clamp(
        half_turn(observation[2] - hue), ranges["hue"]["min"], ranges["hue"]["max"])
    chroma = clamp(observation[1], ranges["chroma"]["min"], ranges["chroma"]["max"])
    return [lightness, chroma, normalized_hue(hue + offset)]


def composite(frost, alpha, backdrop):
    return [channel * alpha + under * (1 - alpha) for channel, under in zip(frost, backdrop)]


def bound(foreground, lower, upper):
    first = luminance(foreground)
    closest = clamp(first, luminance(lower), luminance(upper))
    return (max(first, closest) + 0.05) / (min(first, closest) + 0.05)


def frost_certification(components, alpha, observation_range, opaque):
    lower = composite(components, alpha, observation_range["lower"]["components"])
    upper = composite(components, alpha, observation_range["upper"]["components"])
    bounds = []
    for foreground, background, minimum in guard_pairs():
        if background == FROST_ROLE:
            ratio = bound(opaque[foreground], lower, upper)
            bounds.append({"foreground": foreground, "minimumContrast": minimum,
                           "contrastBound": ratio, "passed": ratio >= minimum})
    return bounds


def color_value(components, alpha=None):
    color = {"colorSpace": "srgb", "components": components}
    if alpha is not None:
        color["alpha"] = alpha
    return color


def adapted_source(theme, roles):
    source = copy.deepcopy(theme)
    group = {}
    for role, adapted in roles.items():
        node = group
        segments = role.split(".")
        for segment in segments[:-1]:
            node = node.setdefault(segment, {})
        node[segments[-1]] = {"$type": "color", "$value": adapted["color"]}
    if "tokens" in source:
        source["tokens"][ADAPTED_GROUP] = group
    else:
        source["tokenResolver"]["resolutionOrder"].append(
            {"type": "set", "name": ADAPTED_GROUP, "sources": [{ADAPTED_GROUP: group}]})
    for role in roles:
        source["colorAssignments"]["roles"][role] = f"{ADAPTED_GROUP}.{role}"
    return source


def adapt(request):
    from check_schemas import parse_json
    check_profile(request["profile"])
    observation = request.get("observation")
    check_observation(observation)
    reason = skipped_reason(request)
    observed_oklch = achromatic = None
    if reason is None:
        observed_oklch, achromatic = observed(observation["color"]["components"])
    themes, failures = {}, []
    for name in THEMES:
        theme = parse_json(request["templates"][name], f"{name} template")
        try:
            colors, alphas, opaque = template_roles(theme, request["externalSources"])
            ranges = request["profile"]["themes"][name]
            template = {role: oklch_from_srgb(colors[role]) for role in ADAPTABLE_ROLES}
            for role in ADAPTABLE_ROLES:
                if not ranges[role]["chroma"]["min"] <= template[role][1] <= ranges[role]["chroma"]["max"]:
                    raise PigmentError(
                        f"authored chroma of {role} lies outside its profile chroma range")
        except DerivationError as error:
            failures.append(f"{name} theme: {error}")
            continue
        roles, certification = {}, None
        if reason is None:
            for role in ADAPTABLE_ROLES:
                target = select_target(template[role], ranges[role], observed_oklch, achromatic)
                components = gamut_map(target)
                if role == FROST_ROLE:
                    if "range" not in observation:
                        continue
                    certification = frost_certification(
                        components, alphas[role], observation["range"], opaque)
                    if not all(entry["passed"] for entry in certification):
                        continue
                    color = color_value(components, alphas[role])
                else:
                    color = color_value(components)
                    opaque[role] = components
                roles[role] = {"targetOklch": target, "color": color}
        guards, failed = legibility_guard(opaque)
        if failed:
            failures.append(f"{name} theme: " + "; ".join(failed))
            continue
        result = {"source": adapted_source(theme, roles) if roles else theme,
                  "roles": roles, "legibilityGuard": guards}
        if certification is not None:
            result["frostCertification"] = certification
        themes[name] = result
    if failures:
        raise PigmentError("\n".join(failures))
    adaptation = ({"applied": False, "reason": reason} if reason else
                  {"applied": True, "observation": {"oklch": observed_oklch, "achromatic": achromatic}})
    return {"schemaVersion": "0.1.0", "adaptation": adaptation, "themes": themes}
