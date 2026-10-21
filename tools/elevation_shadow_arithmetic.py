import math
from fractions import Fraction

ROLES = ("embedded", "base", "raised", "floating", "overlay", "modal")
PENUMBRA_PER_DEPTH = 0.5
# Members in logical px compare absolutely or relatively; every other number, including the
# hull's unit radial parameters, absolutely (spec/92).
GEOMETRY = ("/finalDepth", "/sideOffset/", "/contactMargin", "/shadow/sweep/", "/shadow/hull/",
            "/shadow/bandThickness/", "/shadow/penumbraWidth", "/shadow/paintBounds/")
RADIAL = ("/start/x", "/start/y", "/end/x", "/end/y")

# spec/04 sRGB -> XYZ D65 (CSS Color 4 rational matrix) -> LMS -> Oklab.
RGB_XYZ = tuple(tuple(float(Fraction(*value)) for value in row) for row in (
    ((506752, 1228815), (87881, 245763), (12673, 70218)),
    ((87098, 409605), (175762, 245763), (12673, 175545)),
    ((7918, 409605), (87881, 737289), (1001167, 1053270)),
))
XYZ_LMS = (
    (0.8190224379967030, 0.3619062600528904, -0.1288737815209879),
    (0.0329836539323885, 0.9292868615863434, 0.0361446663506424),
    (0.0481771893596242, 0.2642395317527308, 0.6335478284694309),
)
LMS_OKLAB = (
    (0.2104542683093140, 0.7936177747023054, -0.0040720430116193),
    (1.9779985324311684, -2.4285922420485799, 0.4505937096174110),
    (0.0259040424655478, 0.7827717124575296, -0.8086757549230774),
)


def linear(channel):
    return channel / 12.92 if channel <= 0.04045 else ((channel + 0.055) / 1.055) ** 2.4


def product(matrix, vector):
    return [sum(weight * value for weight, value in zip(row, vector)) for row in matrix]


def oklab(components):
    lms = product(XYZ_LMS, product(RGB_XYZ, [linear(c) for c in components]))
    return product(LMS_OKLAB, [math.copysign(abs(value) ** (1 / 3), value) for value in lms])


def luminance(components):
    red, green, blue = (linear(c) for c in components)
    return 0.2126 * red + 0.7152 * green + 0.0722 * blue


def range_contrast(color, lower, upper):
    foreground = luminance(color)
    closest = min(max(foreground, luminance(lower)), luminance(upper))
    return (max(foreground, closest) + 0.05) / (min(foreground, closest) + 0.05)


def srgb(components):
    return {"colorSpace": "srgb", "components": list(components), "alpha": 1}


def unit_direction(direction):
    scale = max(abs(direction["x"]), abs(direction["y"]))
    x, y = direction["x"] / scale, direction["y"] / scale
    length = math.sqrt(x * x + y * y)
    return {"x": x / length, "y": y / length}


def physical_radii(size, radii, layout):
    sums = (
        (size["width"], radii["topStart"]["x"], radii["topEnd"]["x"]),
        (size["width"], radii["bottomStart"]["x"], radii["bottomEnd"]["x"]),
        (size["height"], radii["topStart"]["y"], radii["bottomStart"]["y"]),
        (size["height"], radii["topEnd"]["y"], radii["bottomEnd"]["y"]),
    )
    factor = 1.0
    for extent, first, second in sums:
        if first + second > 0:
            factor = min(factor, (extent / 2) / (first / 2 + second / 2))
    order = ("topStart", "topEnd", "bottomEnd", "bottomStart") if layout == "ltr" else (
        "topEnd", "topStart", "bottomStart", "bottomEnd")
    result = []
    for name in order:
        x, y = radii[name]["x"] * factor, radii[name]["y"] * factor
        result.append((x, y) if x > 0 and y > 0 else (0.0, 0.0))
    return result


def front_support(size, corners, normal):
    width, height = size["width"], size["height"]
    nx, ny = normal
    best = -math.inf
    signs = ((-1, -1), (1, -1), (1, 1), (-1, 1))
    for (rx, ry), (sx, sy) in zip(corners, signs):
        cx = rx if sx < 0 else width - rx
        cy = ry if sy < 0 else height - ry
        base = nx * cx + ny * cy
        ax, ay = rx * nx, ry * ny
        if ax * sx >= 0 and ay * sy >= 0:
            value = base + math.hypot(ax, ay)
        else:
            value = base + max(ax * sx, ay * sy)
        best = max(best, value)
    return best


def segment_support(segment, normal):
    nx, ny = normal
    if segment["kind"] == "line":
        return max(nx * point["x"] + ny * point["y"] for point in (segment["from"], segment["to"]))
    center, radii = segment["center"], segment["radii"]
    base = nx * center["x"] + ny * center["y"]
    ax, ay = radii["x"] * nx, radii["y"] * ny
    start = math.atan2(segment["start"]["y"], segment["start"]["x"])
    span = (math.atan2(segment["end"]["y"], segment["end"]["x"]) - start) % math.tau
    values = [base + ax * segment[end]["x"] + ay * segment[end]["y"] for end in ("start", "end")]
    if (ax or ay) and (math.atan2(ay, ax) - start) % math.tau <= span:
        values.append(base + math.hypot(ax, ay))
    return max(values)


def segment_endpoints(segment):
    if segment["kind"] == "line":
        return segment["from"], segment["to"]
    center, radii = segment["center"], segment["radii"]
    return tuple(
        {"x": center["x"] + radii["x"] * segment[end]["x"], "y": center["y"] + radii["y"] * segment[end]["y"]}
        for end in ("start", "end")
    )


def canonical_contour(size, corners, offset):
    """The spec/35 canonical record of the front swept along `offset`, built from the spec text."""
    width, height = size["width"], size["height"]
    if width == 0 or height == 0:
        return {"schemaVersion": "0.1.0", "bounds": None, "segments": []}
    vx, vy = offset["x"], offset["y"]
    radials = ((-1.0, 0.0), (0.0, -1.0), (1.0, 0.0), (0.0, 1.0))

    def center(index):
        rx, ry = corners[index]
        return (rx if index in (0, 3) else width - rx, ry if index < 2 else height - ry)

    def point(origin, radii, radial, shift):
        return {"x": origin[0] + shift[0] + radii[0] * radial[0], "y": origin[1] + shift[1] + radii[1] * radial[1]}

    pieces = []
    for index in range(4):
        rx, ry = corners[index]
        start, end = radials[index], radials[(index + 1) % 4]
        origin = center(index)
        if rx > 0 and ry > 0:
            # Outward normal at radial r is proportional to (ry r.x, rx r.y).
            def facing(radial):
                return ry * radial[0] * vx + rx * radial[1] * vy
            stops = [start]
            zero = (rx * vy, -ry * vx)
            length = math.hypot(*zero)
            if zero[0] and zero[1]:
                for sign in (1, -1):
                    radial = (sign * zero[0] / length, sign * zero[1] / length)
                    if radial[0] * (start[0] + end[0]) > 0 and radial[1] * (start[1] + end[1]) > 0:
                        stops.append(radial)
            stops.append(end)
            for first, second in zip(stops, stops[1:]):
                middle = math.atan2(first[1], first[0]) + ((math.atan2(second[1], second[0])
                                                            - math.atan2(first[1], first[0])) % math.tau) / 2
                shift = (vx, vy) if facing((math.cos(middle), math.sin(middle))) > 0 else (0.0, 0.0)
                arc_center = {"x": origin[0] + shift[0], "y": origin[1] + shift[1]}
                pieces.append({"kind": "arc", "center": arc_center, "radii": {"x": rx, "y": ry},
                               "start": {"x": first[0], "y": first[1]}, "end": {"x": second[0], "y": second[1]}})
        following = (index + 1) % 4
        shift = (vx, vy) if end[0] * vx + end[1] * vy > 0 else (0.0, 0.0)
        line_from = point(origin, corners[index], end, shift)
        line_to = point(center(following), corners[following], end, shift)
        if line_from != line_to:
            pieces.append({"kind": "line", "from": line_from, "to": line_to})
    segments = []
    for piece in pieces:
        start = segment_endpoints(piece)[0]
        if segments:
            previous = segment_endpoints(segments[-1])[1]
            if previous != start:
                segments.append({"kind": "line", "from": previous, "to": start})
        segments.append(piece)
    if segments:
        last, first = segment_endpoints(segments[-1])[1], segment_endpoints(segments[0])[0]
        if last != first:
            segments.append({"kind": "line", "from": last, "to": first})
    bounds = {"x": min(0.0, vx), "y": min(0.0, vy), "width": width + abs(vx), "height": height + abs(vy)}
    return {"schemaVersion": "0.1.0", "bounds": bounds, "segments": segments}


def hull_violation(hull, request, sweep, directions=720):
    """Check the hull record against the support identity h_H(n) = h_F(n) + max(0, n.v)."""
    size = request["size"]
    width, height = size["width"], size["height"]
    bounds = hull["bounds"]
    expected_bounds = (min(0.0, sweep["x"]), min(0.0, sweep["y"]),
                       width + abs(sweep["x"]), height + abs(sweep["y"]))
    scale = max(1.0, width, height, abs(sweep["x"]), abs(sweep["y"]))
    for name, value in zip(("x", "y", "width", "height"), expected_bounds):
        if not math.isclose(bounds[name], value, rel_tol=1e-12, abs_tol=1e-12):
            return f"/hull/bounds/{name}"
    segments = hull["segments"]
    for index, segment in enumerate(segments):
        end = segment_endpoints(segment)[1]
        start = segment_endpoints(segments[(index + 1) % len(segments)])[0]
        if math.dist((end["x"], end["y"]), (start["x"], start["y"])) > 1e-9 * scale:
            return f"/hull/segments/{index}/connectivity"
    corners = physical_radii(size, request["radii"], request["environment"]["layoutDirection"])
    for step in range(directions):
        angle = math.tau * step / directions
        normal = (math.cos(angle), math.sin(angle))
        actual = max(segment_support(segment, normal) for segment in segments)
        expected = front_support(size, corners, normal) + max(0.0, normal[0] * sweep["x"] + normal[1] * sweep["y"])
        if abs(actual - expected) > 1e-9 * scale:
            return f"/hull/support/{step}"
    return None


def shadow_mismatch(actual, expected, pointer=""):
    """First JSON pointer where `actual` differs, with spec/92 tolerance classes."""
    if isinstance(expected, dict):
        if not isinstance(actual, dict) or actual.keys() != expected.keys():
            return pointer or "/"
        for name, value in expected.items():
            found = shadow_mismatch(actual[name], value, f"{pointer}/{name}")
            if found:
                return found
        return None
    if isinstance(expected, list):
        if not isinstance(actual, list) or len(actual) != len(expected):
            return pointer or "/"
        for index, value in enumerate(expected):
            found = shadow_mismatch(actual[index], value, f"{pointer}/{index}")
            if found:
                return found
        return None
    if isinstance(expected, bool) or isinstance(actual, bool):
        return None if actual is expected else pointer or "/"
    if isinstance(expected, (int, float)) and isinstance(actual, (int, float)):
        relative = 1e-12 if pointer.startswith(GEOMETRY) and not pointer.endswith(RADIAL) else 0
        close = math.isfinite(actual) and math.isclose(actual, expected, rel_tol=relative, abs_tol=1e-12)
        return None if close else pointer or "/"
    return None if actual == expected else pointer or "/"


def select(candidates, lower, upper, minimum):
    for index, (role, color) in enumerate(candidates):
        ratio = range_contrast(color["components"], lower, upper)
        if ratio >= minimum:
            return {"colorRole": role, "color": color, "contrastRatio": ratio, "fallbackApplied": index == 1}
    return None


def expected_result(request, mutation=None):
    """Independent reference arithmetic for every result member except the hull record,
    which `canonical_contour` and `hull_violation` check.

    Returns None when resolution must fail. `mutation` names a deliberately
    nonconformant law used to prove that the checker rejects it.
    """
    profiles = request["profiles"]
    profile = profiles["roles"][request["elevation"]]
    shade, margin = profile["shadowShade"], profile["contactMargin"]
    environment = request["environment"]
    direction = dict(request["keyLight"]["direction"])
    if mutation == "mirror-rtl" and environment["layoutDirection"] == "rtl":
        direction["x"] = -direction["x"]
    key = unit_direction(direction)
    depth = request["elevationDepth"] * request["depthScale"]
    reach = 2 * depth + margin
    sweep = {"x": -key["x"] * reach, "y": -key["y"] * reach}
    adjacent = request["adjacentColor"]["components"]
    size = request["size"]
    present = depth > 0 and size["width"] > 0 and size["height"] > 0
    band = [c * (1 - shade) for c in adjacent]
    shadow = None
    lower = band if present else adjacent
    if present:
        capabilities = environment["rendererCapabilities"]
        high_contrast = environment["accessibilityPreferences"]["highContrast"]
        width = PENUMBRA_PER_DEPTH * depth if capabilities["advancedShadow"] and not high_contrast else 0.0
        extent = depth + margin
        shadow = {
            "sweep": sweep,
            "bandThickness": {
                "top": max(0.0, key["y"] * extent), "right": max(0.0, -key["x"] * extent),
                "bottom": max(0.0, -key["y"] * extent), "left": max(0.0, key["x"] * extent),
            },
            "bandColor": srgb(band),
            "colorRange": {"lower": srgb(band), "upper": srgb(adjacent)},
            "deltaEOk": math.dist(oklab(band), oklab(adjacent)),
            "penumbraWidth": width,
            "paintBounds": {
                "x": min(0.0, sweep["x"]) - width, "y": min(0.0, sweep["y"]) - width,
                "width": size["width"] + abs(sweep["x"]) + 2 * width,
                "height": size["height"] + abs(sweep["y"]) + 2 * width,
            },
        }
    if mutation == "skip-band-guard":
        lower = adjacent
    edge = request["edge"]
    guard = select((("outline", edge["outline"]), ("outline.strong", edge["outlineStrong"])),
                   lower, adjacent, edge["minimumContrast"])
    if guard is None:
        return None
    focus = None
    if request["focus"] is not None:
        candidates = (("focus", request["focus"]["color"]), ("outline.strong", edge["outlineStrong"]))
        if mutation == "no-focus-fallback":
            candidates = candidates[:1]
        focus = select(candidates, lower, adjacent, request["focus"]["minimumContrast"])
        if focus is None:
            return None
    samples = []
    for distance in request["distanceSamples"]:
        width = shadow["penumbraWidth"] if shadow else 0.0
        if shadow is None:
            weight = 0.0
        elif mutation == "gaussian" and width > 0:
            weight = 0.5 * math.erfc(distance / (width / 3 * math.sqrt(2)))
        elif distance <= 0:
            weight = 1.0
        elif distance < width:
            weight = 1 - distance / width
        else:
            weight = 0.0
        samples.append({"weight": weight, "color": srgb([c * (1 - shade * weight) for c in adjacent])})
    return {
        "schemaVersion": "0.1.0", "elevation": request["elevation"], "keyDirection": key,
        "finalDepth": depth, "sideOffset": {"x": -key["x"] * depth, "y": -key["y"] * depth},
        "shadowShade": shade, "contactMargin": margin, "shadow": shadow, "edge": guard,
        "focus": focus, "samples": samples,
    }
