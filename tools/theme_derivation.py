import copy
import math

SRGB_TO_XYZ = (
    (506752 / 1228815, 87881 / 245763, 12673 / 70218),
    (87098 / 409605, 175762 / 245763, 12673 / 175545),
    (7918 / 409605, 87881 / 737289, 1001167 / 1053270),
)
XYZ_TO_SRGB = (
    (12831 / 3959, -329 / 214, -1974 / 3959),
    (-851781 / 878810, 1648619 / 878810, 36519 / 878810),
    (705 / 12673, -2585 / 12673, 705 / 667),
)
XYZ_TO_LMS = (
    (0.8190224379967030, 0.3619062600528904, -0.1288737815209879),
    (0.0329836539323885, 0.9292868615863434, 0.0361446663506424),
    (0.0481771893596242, 0.2642395317527308, 0.6335478284694309),
)
LMS_TO_OKLAB = (
    (0.2104542683093140, 0.7936177747023054, -0.0040720430116193),
    (1.9779985324311684, -2.4285922420485799, 0.4505937096174110),
    (0.0259040424655478, 0.7827717124575296, -0.8086757549230774),
)
OKLAB_TO_LMS = (
    (1.0, 0.3963377773761749, 0.2158037573099136),
    (1.0, -0.1055613458156586, -0.0638541728258133),
    (1.0, -0.0894841775298119, -1.2914855480194092),
)
LMS_TO_XYZ = (
    (1.2268798758459243, -0.5578149944602171, 0.2813910456659647),
    (-0.0405757452148008, 1.1122868032803170, -0.0717110580655164),
    (-0.0763729366746601, -0.4214933324022432, 1.5869240198367816),
)

JND = 0.02
EPSILON = 0.0001
ACHROMATIC_CHROMA = 0.02
DERIVED_GROUP = "derivedColor"
DERIVED_ROLES = ("accent.primary", "accent.secondary", "accent.tertiary", "selection", "focus")
HUE_OFFSET_ROLES = ("accent.secondary", "accent.tertiary")
THEMES = ("light", "dark")
GUARD_BACKGROUNDS = ("surface.base", "surface.low", "surface.high", "surface.chrome")
GUARD_TEXT = (
    "content.primary", "content.secondary", "content.muted",
    "status.error", "status.warning", "status.success", "status.info",
)
TEXT_CONTRAST = 4.5
NON_TEXT_CONTRAST = 3.0


class DerivationError(ValueError):
    pass


def multiply(matrix, vector):
    return [row[0] * vector[0] + row[1] * vector[1] + row[2] * vector[2] for row in matrix]


def decode(channel):
    return channel / 12.92 if channel <= 0.04045 else ((channel + 0.055) / 1.055) ** 2.4


def encode(channel):
    magnitude = abs(channel)
    if magnitude > 0.0031308:
        return math.copysign(1.055 * magnitude ** (1 / 2.4) - 0.055, channel)
    return 12.92 * channel


def srgb_to_oklab(srgb):
    lms = multiply(XYZ_TO_LMS, multiply(SRGB_TO_XYZ, [decode(value) for value in srgb]))
    return multiply(LMS_TO_OKLAB, [math.copysign(abs(value) ** (1 / 3), value) for value in lms])


def oklab_to_srgb(oklab):
    lms = [value ** 3 for value in multiply(OKLAB_TO_LMS, oklab)]
    return [encode(value) for value in multiply(XYZ_TO_SRGB, multiply(LMS_TO_XYZ, lms))]


def normalized_hue(degrees):
    hue = math.fmod(degrees, 360.0)
    if hue < 0:
        hue += 360.0
    return 0.0 if hue >= 360.0 else hue


def oklch_from_srgb(srgb):
    lightness, a, b = srgb_to_oklab(srgb)
    return [lightness, math.hypot(a, b), normalized_hue(math.degrees(math.atan2(b, a)))]


def oklch_to_oklab(oklch):
    lightness, chroma, hue = oklch
    radians = math.radians(hue)
    return [lightness, chroma * math.cos(radians), chroma * math.sin(radians)]


def in_gamut(srgb):
    return all(0.0 <= value <= 1.0 for value in srgb)


def clip(oklch):
    return [min(1.0, max(0.0, value)) for value in oklab_to_srgb(oklch_to_oklab(oklch))]


def delta_eok(srgb, oklch):
    return math.dist(srgb_to_oklab(srgb), oklch_to_oklab(oklch))


def gamut_map(origin):
    lightness, chroma, hue = origin
    if lightness >= 1.0:
        return [1.0, 1.0, 1.0]
    if lightness <= 0.0:
        return [0.0, 0.0, 0.0]
    converted = oklab_to_srgb(oklch_to_oklab(origin))
    if in_gamut(converted):
        return converted
    current = [lightness, chroma, hue]
    clipped = clip(current)
    if delta_eok(clipped, current) < JND:
        return clipped
    low, high, low_in_gamut = 0.0, chroma, True
    while high - low > EPSILON:
        current[1] = (low + high) / 2
        if low_in_gamut and in_gamut(oklab_to_srgb(oklch_to_oklab(current))):
            low = current[1]
            continue
        clipped = clip(current)
        difference = delta_eok(clipped, current)
        if difference < JND:
            if JND - difference < EPSILON:
                return clipped
            low_in_gamut = False
            low = current[1]
        else:
            high = current[1]
    return clipped


def luminance(srgb):
    linear = [decode(value) for value in srgb]
    return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]


def contrast(foreground, background):
    first, second = luminance(foreground), luminance(background)
    return (max(first, second) + 0.05) / (min(first, second) + 0.05)


def guard_pairs():
    pairs = []
    for background in GUARD_BACKGROUNDS:
        pairs.extend((foreground, background, TEXT_CONTRAST) for foreground in GUARD_TEXT)
        pairs.append(("outline", background, NON_TEXT_CONTRAST))
        pairs.append(("focus", background, NON_TEXT_CONTRAST))
    pairs.append(("content.primary", "selection", TEXT_CONTRAST))
    pairs.extend(("content.inverse", accent, TEXT_CONTRAST)
                 for accent in ("accent.primary", "accent.secondary", "accent.tertiary"))
    return pairs


def legibility_guard(opaque):
    results, failures = [], []
    for foreground, background, minimum in guard_pairs():
        ratio = contrast(opaque[foreground], opaque[background])
        results.append({"foreground": foreground, "background": background,
                        "minimumContrast": minimum, "contrastRatio": ratio})
        if ratio < minimum:
            failures.append(f"{foreground} over {background}: {ratio} < {minimum}")
    return results, failures


def merge(target, source):
    for name, value in source.items():
        if isinstance(value, dict) and isinstance(target.get(name), dict) and "$value" not in value:
            merge(target[name], value)
        else:
            target[name] = copy.deepcopy(value)


def composed_tokens(theme, external_sources):
    if "tokens" in theme:
        return copy.deepcopy(theme["tokens"])
    resolver = theme["tokenResolver"]
    tokens = {}

    def add_source(source):
        if set(source) == {"$ref"}:
            reference = source["$ref"]
            if reference.startswith("#/sets/"):
                for nested in resolver["sets"][reference[len("#/sets/"):]]["sources"]:
                    add_source(nested)
            elif reference in external_sources:
                from check_schemas import parse_json
                merge(tokens, parse_json(external_sources[reference], reference))
            else:
                raise DerivationError(f"missing external source {reference}")
        else:
            merge(tokens, source)

    for entry in resolver["resolutionOrder"]:
        if set(entry) == {"$ref"}:
            add_source(entry)
        elif entry.get("type") == "set":
            for source in entry["sources"]:
                add_source(source)
        else:
            raise DerivationError("template modifiers are outside the checker's resolver subset")
    return tokens


def token_color(tokens, path, seen=()):
    if path in seen:
        raise DerivationError(f"alias cycle at {path}")
    node = tokens
    for segment in path.split("."):
        if not isinstance(node, dict) or segment not in node:
            raise DerivationError(f"missing token {path}")
        node = node[segment]
    value = node["$value"]
    if isinstance(value, str) and value.startswith("{") and value.endswith("}"):
        return token_color(tokens, value[1:-1], (*seen, path))
    if value.get("colorSpace") != "srgb":
        raise DerivationError(f"checker templates use numeric sRGB colors: {path}")
    return list(value["components"]), value.get("alpha", 1)


def contains_token(node):
    if not isinstance(node, dict):
        return False
    return "$value" in node or any(contains_token(value) for value in node.values())


def template_colors(theme, external_sources):
    tokens = composed_tokens(theme, external_sources)
    if contains_token(tokens.get(DERIVED_GROUP)):
        raise DerivationError(f"template already defines the {DERIVED_GROUP} group")
    roles = theme["colorAssignments"]["roles"]
    colors, opaque = {}, {}
    for role, path in roles.items():
        colors[role], alpha = token_color(tokens, path)
        override = theme["opaqueColorAssignments"]["roles"].get(role)
        if override is not None:
            opaque[role], _ = token_color(tokens, override)
        elif alpha == 1:
            opaque[role] = colors[role]
        if role in DERIVED_ROLES and (alpha != 1 or override is not None):
            raise DerivationError(f"derived role {role} must be opaque without an opaque override")
    return colors, opaque


def seed_oklch(seed):
    lightness, chroma, hue = oklch_from_srgb(seed)
    achromatic = chroma < ACHROMATIC_CHROMA
    return [lightness, chroma, 0.0 if achromatic else hue], achromatic


def derived_targets(seed, colors):
    (_, _, seed_hue), achromatic = seed_oklch(seed)
    template = {role: oklch_from_srgb(colors[role]) for role in DERIVED_ROLES}
    targets = {}
    for role in DERIVED_ROLES:
        lightness, chroma, hue = template[role]
        if achromatic:
            targets[role] = [lightness, 0.0, 0.0]
            continue
        offset = hue - template["accent.primary"][2] if role in HUE_OFFSET_ROLES else 0.0
        targets[role] = [lightness, chroma, normalized_hue(seed_hue + offset)]
    return targets


def derived_source(theme, colors):
    source = copy.deepcopy(theme)
    group = {}
    for role, components in colors.items():
        node = group
        segments = role.split(".")
        for segment in segments[:-1]:
            node = node.setdefault(segment, {})
        node[segments[-1]] = {"$type": "color",
                              "$value": {"colorSpace": "srgb", "components": components}}
    if "tokens" in source:
        source["tokens"][DERIVED_GROUP] = group
    else:
        source["tokenResolver"]["resolutionOrder"].append(
            {"type": "set", "name": DERIVED_GROUP, "sources": [{DERIVED_GROUP: group}]})
    for role in DERIVED_ROLES:
        source["colorAssignments"]["roles"][role] = f"{DERIVED_GROUP}.{role}"
    return source


def derive(request):
    from check_schemas import parse_json
    seed = request["seed"]["components"]
    oklch, achromatic = seed_oklch(seed)
    themes, failures = {}, []
    for name in THEMES:
        theme = parse_json(request["templates"][name], f"{name} template")
        try:
            colors, opaque = template_colors(theme, request["externalSources"])
        except DerivationError as error:
            failures.append(f"{name} theme: {error}")
            continue
        targets = derived_targets(seed, colors)
        mapped = {role: gamut_map(targets[role]) for role in DERIVED_ROLES}
        opaque.update(mapped)
        guards, failed = legibility_guard(opaque)
        if failed:
            failures.append(f"{name} theme: " + "; ".join(failed))
            continue
        themes[name] = {
            "source": derived_source(theme, mapped),
            "roles": {role: {"targetOklch": targets[role],
                             "color": {"colorSpace": "srgb", "components": mapped[role]}}
                      for role in DERIVED_ROLES},
            "legibilityGuard": guards,
        }
    if failures:
        raise DerivationError("\n".join(failures))
    return {"schemaVersion": "0.1.0",
            "seed": {"oklch": oklch, "achromatic": achromatic},
            "themes": themes}
