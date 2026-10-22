"""Independent lit control body arithmetic with circular corners and swept regions."""
import copy
import itertools
import math
from fractions import Fraction

from command_label_arithmetic import layout as label_layout
from command_layout_arithmetic import resolve as resolve_command_layout
from material_light_arithmetic import expected_result, pigment_result

EPSILON = 2.0 ** -52
PHYSICAL = ("topLeft", "topRight", "bottomRight", "bottomLeft")
LOGICAL = {
    "ltr": ("topStart", "topEnd", "bottomEnd", "bottomStart"),
    "rtl": ("topEnd", "topStart", "bottomStart", "bottomEnd"),
}
SIGNS = ((-1, -1), (1, -1), (1, 1), (-1, 1))
STATES = {"rest", "hover", "pressed", "disabled", "focused", "checked"}
QUARTERS = (((-1.0, 0.0), (0.0, -1.0)), ((0.0, -1.0), (1.0, 0.0)), ((1.0, 0.0), (0.0, 1.0)), ((0.0, 1.0), (-1.0, 0.0)))
EDGE_NORMALS = ((0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0))


def phase(states):
    unsupported = set(states) - STATES
    if unsupported:
        raise ValueError("lit control bodies support only rest, hover, pressed, disabled, focused and checked states")
    for name in ("disabled", "pressed", "hover"):
        if name in states:
            return name
    return "rest"


def physical_radii(source):
    width, height = source["size"]["width"], source["size"]["height"]
    radii = source["radii"]
    sums = (
        (radii["topStart"]["x"] + radii["topEnd"]["x"], width),
        (radii["bottomStart"]["x"] + radii["bottomEnd"]["x"], width),
        (radii["topStart"]["y"] + radii["bottomStart"]["y"], height),
        (radii["topEnd"]["y"] + radii["bottomEnd"]["y"], height),
    )
    factor = min([1.0] + [limit / total for total, limit in sums if total > 0])
    result = []
    for corner, name in zip(PHYSICAL, LOGICAL[source["environment"]["layoutDirection"]]):
        x, y = radii[name]["x"] * factor, radii[name]["y"] * factor
        if x == 0 or y == 0:
            x = y = 0.0
        if x != y:
            raise ValueError(f"{corner} corner must be circular with equal extents")
        result.append(x)
    return result


def number(value):
    """Shortest decimal form, without a fraction for integral values."""
    return str(int(value)) if float(value).is_integer() else repr(float(value))


def relief_profile_violation(profile):
    """Return the first semantic violation of one spec/82 relief profile, or None."""
    kind, thickness = profile["kind"], profile["bodyThickness"]
    amplitude, shoulder = profile["amplitude"], profile["shoulderWidth"]
    if not (math.isfinite(thickness) and thickness > 0):
        return "bodyThickness must be finite and positive"
    if not (math.isfinite(amplitude) and amplitude >= 0 and math.isfinite(shoulder) and shoulder >= 0):
        return "amplitude and shoulderWidth must be finite and nonnegative"
    if kind == "flat":
        return None if amplitude == 0 and shoulder == 0 else "flat requires zero amplitude and shoulderWidth"
    if not (0 < amplitude and 0 < shoulder and 2 * amplitude <= shoulder):
        return "relief requires positive amplitude no greater than shoulderWidth/2"
    if amplitude / shoulder == 0:
        return "amplitude/shoulderWidth must remain representable"
    relieved = thickness + amplitude if kind == "convex" else thickness - amplitude
    if not (math.isfinite(relieved) and relieved > 0) or relieved == thickness:
        return "relieved thickness must be representable and positive"
    return None


def smooth(t):
    if t <= 0:
        return 0.0, 0.0
    if t >= 1:
        return 1.0, 0.0
    return t ** 3 * (10 - 15 * t + 6 * t * t), 30 * t * t * (1 - t) ** 2


class Body:
    def __init__(self, source):
        self.source = source
        identity = source["identity"]
        self.phase = phase(identity["states"]["states"])
        family = identity["materialFamily"]
        if family not in ("cast", "frost", "elastomer"):
            raise ValueError(f"unsupported lit control material family {family}")
        if not identity["materialRole"].startswith("control."):
            raise ValueError("lit control bodies require a control material role")
        if ("frostRepresentation" in identity) != (family == "frost") or (identity["contentFallbackApplied"] and family != "frost"):
            raise ValueError("only Frost reports a representation or content fallback")
        self.width = source["size"]["width"]
        self.height = source["size"]["height"]
        self.radii = physical_radii(source)
        self.edge_width = source["edgeWidth"]
        self.depth = source["depth"]
        if not (self.width > 0 and self.height > 0):
            raise ValueError("size must enclose positive area")
        if not (self.edge_width > 0 and 2 * self.edge_width < min(self.width, self.height)):
            raise ValueError("edgeWidth must preserve a positive representable interior")
        if not (math.isfinite(self.depth) and self.depth >= 0):
            raise ValueError("depth must be finite and nonnegative")
        profile = source["relief"]
        self.kind = profile["kind"]
        self.amplitude = profile["amplitude"]
        self.shoulder = profile["shoulderWidth"]
        violation = relief_profile_violation(profile)
        if violation:
            raise ValueError(violation)
        if self.kind != "flat":
            for name, radius in zip(PHYSICAL, self.radii):
                if radius > 0 and 2 * (self.shoulder / radius + self.shoulder / radius) >= 1:
                    raise ValueError(
                        f"{name} corner radius {number(radius)} px cannot carry shoulderWidth {number(self.shoulder)} px: tau must be below 1"
                    )
        self.pigment = pigment_result({
            "materialFamily": identity["materialFamily"],
            "profiles": source["pigmentProfiles"],
            "body": source["body"],
        })
        self.light = expected_result({
            "pigment": {
                "materialFamily": identity["materialFamily"],
                "profiles": source["pigmentProfiles"],
                "body": source["body"],
            },
            "light": {"keyLight": source["keyLight"], **source["light"]},
            "environment": source["environment"],
            "enabled": True,
            "interactionActive": self.phase in ("hover", "pressed"),
            "normals": [],
        })
        key = self.light["keyDirection"]
        self.offset = (-key["x"] * self.depth, -key["y"] * self.depth)
        self.edge = edge_color(source["edge"])
        foreground = source["foreground"]
        if foreground["alpha"] != 1:
            raise ValueError("foreground must be opaque")
        self.content_contrast = contrast(foreground["components"], source["body"]["components"])
        if self.content_contrast < source["minimumContentContrast"]:
            raise ValueError("foreground contrast is insufficient on the uniform body")
        self.content_bounds = self.reserve_content()

    def constraints(self, inset):
        centers = (
            (0, 0), (self.width, 0), (self.width, self.height), (0, self.height),
        )
        result = []
        for (cx, cy), (sx, sy), radius in zip(centers, SIGNS, self.radii):
            if radius <= inset:
                continue
            center = (cx - sx * radius, cy - sy * radius)
            result.append((center, (sx, sy), radius - inset))
        return result

    def contains(self, point, inset):
        """Closed membership: binary64 slack decides clear cases, exact rationals decide the rest."""
        x, y = point
        scale = max(self.width, self.height, abs(x), abs(y))
        guard = 1e-9 * scale
        slack = min(x - inset, self.width - inset - x, y - inset, self.height - inset - y)
        for (cx, cy), (sx, sy), radius in self.constraints(inset):
            u = max(sx * (x - cx), 0.0)
            v = max(sy * (y - cy), 0.0)
            slack = min(slack, radius - math.hypot(u, v))
        if abs(slack) > guard:
            return slack > 0
        return self.exact_contains((Fraction(x), Fraction(y)), inset)

    def exact_contains(self, point, inset):
        """Closed edge interior membership of an exact rational point."""
        x, y = point
        inset = Fraction(inset)
        if not (inset <= x <= Fraction(self.width) - inset and inset <= y <= Fraction(self.height) - inset):
            return False
        for (cx, cy), (sx, sy), radius in self.exact_constraints(inset):
            u = max(sx * (x - cx), 0)
            v = max(sy * (y - cy), 0)
            if u * u + v * v > radius * radius:
                return False
        return True

    def exact_constraints(self, inset):
        centers = ((0, 0), (self.width, 0), (self.width, self.height), (0, self.height))
        result = []
        for (cx, cy), (sx, sy), radius in zip(centers, SIGNS, self.radii):
            radius = Fraction(radius)
            if radius <= inset:
                continue
            center = (Fraction(cx) - sx * radius, Fraction(cy) - sy * radius)
            result.append((center, (sx, sy), radius - inset))
        return result

    def uniform_contains(self, point):
        """Exact closed membership in E and the uniform relief plateau, where every relief gate is one.

        The plateau is the straight band at least the shoulder width from each side, intersected with
        a disc about every rounded corner's center whose squared radius is r*r*(1 - tau) = r*r - 4*w*r.
        """
        if not self.exact_contains(point, self.edge_width):
            return False
        if self.kind == "flat":
            return True
        x, y = point
        w = Fraction(self.shoulder)
        if not (w <= x <= Fraction(self.width) - w and w <= y <= Fraction(self.height) - w):
            return False
        for (cx, cy), (sx, sy), radius in self.exact_constraints(0):
            u = max(sx * (x - cx), 0)
            v = max(sy * (y - cy), 0)
            if u * u + v * v > radius * radius - 4 * w * radius:
                return False
        return True

    def uniform_contains_box(self, bounds):
        """spec/46 lit content: the four vertices of the closed box, with exact endpoints, lie in E and the plateau."""
        x, y = Fraction(bounds["x"]), Fraction(bounds["y"])
        right, bottom = x + Fraction(bounds["width"]), y + Fraction(bounds["height"])
        return all(self.uniform_contains((px, py)) for px in (x, right) for py in (y, bottom))

    def swept(self, point, inset, band=0.0):
        """Return True, False or None when the decision lies within band of a boundary."""
        decisions = {self.sweep_interval(point, inset, slack) for slack in (-band, band)} if band else {self.sweep_interval(point, inset, 0.0)}
        return decisions.pop() if len(decisions) == 1 else None

    def sweep_interval(self, point, inset, slack):
        dx, dy = self.offset
        lower, upper = 0.0, 1.0

        def linear(start, slope, bound):
            if slope > 0:
                return (start - bound) / slope, math.inf
            if slope < 0:
                return -math.inf, (start - bound) / slope
            return (-math.inf, math.inf) if start <= bound else (math.inf, -math.inf)

        def intersect(*intervals):
            return max(i[0] for i in intervals), min(i[1] for i in intervals)

        for coordinate, slope, length in ((point[0], dx, self.width), (point[1], dy, self.height)):
            low = linear(-coordinate, -slope, -(inset - slack))
            high = linear(coordinate, slope, length - inset + slack)
            lower, upper = intersect((lower, upper), low, high)
        for (cx, cy), (sx, sy), radius in self.constraints(inset):
            radius += slack
            x0, y0 = sx * (point[0] - cx), sy * (point[1] - cy)
            sdx, sdy = sx * dx, sy * dy
            pieces = [
                intersect(linear(x0, sdx, 0.0), linear(y0, sdy, radius)),
                intersect(linear(y0, sdy, 0.0), linear(x0, sdx, radius)),
            ]
            a = sdx * sdx + sdy * sdy
            b = -2 * (x0 * sdx + y0 * sdy)
            c = x0 * x0 + y0 * y0 - radius * radius
            if a == 0:
                pieces.append((-math.inf, math.inf) if c <= 0 else (math.inf, -math.inf))
            else:
                discriminant = b * b - 4 * a * c
                if discriminant >= 0:
                    root = math.sqrt(discriminant)
                    pieces.append(((-b - root) / (2 * a), (-b + root) / (2 * a)))
            pieces = [intersect(piece, (0.0, 1.0)) for piece in pieces]
            pieces = [piece for piece in pieces if piece[0] <= piece[1]]
            if not pieces:
                return False
            lower, upper = intersect((lower, upper), (min(p[0] for p in pieces), max(p[1] for p in pieces)))
        return lower <= upper

    def relief_height(self, point):
        if self.kind == "flat":
            return 0.0
        x, y = point
        w = self.shoulder
        value = math.prod(smooth(distance / w)[0] for distance in (x, self.width - x, y, self.height - y))
        locals_ = ((x, y), (self.width - x, y), (self.width - x, self.height - y), (x, self.height - y))
        for radius, (lx, ly) in zip(self.radii, locals_):
            if radius:
                u = min(max((radius - lx) / radius, 0.0), 1.0)
                v = min(max((radius - ly) / radius, 0.0), 1.0)
                value *= smooth((1 - u * u - v * v) / (4 * w / radius))[0]
        return (-1 if self.kind == "recessed" else 1) * self.amplitude * value

    def normal(self, point):
        if self.kind == "flat":
            return 0.0, 0.0, 1.0
        x, y = point
        w = self.shoulder
        gates = []
        for distance, sx, sy in ((x, 1, 0), (self.width - x, -1, 0), (y, 0, 1), (self.height - y, 0, -1)):
            value, slope = smooth(distance / w)
            gates.append((value, slope * sx, slope * sy))
        locals_ = ((x, y, 1, 1), (self.width - x, y, -1, 1), (self.width - x, self.height - y, -1, -1), (x, self.height - y, 1, -1))
        for radius, (lx, ly, sx, sy) in zip(self.radii, locals_):
            if radius == 0:
                continue
            ratio = w / radius
            tau = 2 * (ratio + ratio)
            u = min(max((radius - lx) / radius, 0.0), 1.0)
            v = min(max((radius - ly) / radius, 0.0), 1.0)
            value, slope = smooth((1 - u * u - v * v) / tau)
            gates.append((value, slope * 2 * u * ratio / tau * sx, slope * 2 * v * ratio / tau * sy))
        sign = -1 if self.kind == "recessed" else 1
        dzx = dzy = 0.0
        for index, gate in enumerate(gates):
            others = math.prod(g[0] for j, g in enumerate(gates) if j != index)
            dzx += gate[1] * others
            dzy += gate[2] * others
        dzx *= sign * self.amplitude / w
        dzy *= sign * self.amplitude / w
        length = math.sqrt(1 + (dzx * dzx + dzy * dzy))
        return -dzx / length, -dzy / length, 1 / length

    def face_color(self, point):
        nx, ny, nz = self.normal(point)
        result = expected_result({
            "pigment": {"materialFamily": self.source["identity"]["materialFamily"], "profiles": self.source["pigmentProfiles"], "body": self.source["body"]},
            "light": {"keyLight": self.source["keyLight"], **self.source["light"]},
            "environment": self.source["environment"],
            "enabled": True,
            "interactionActive": self.phase in ("hover", "pressed"),
            "normals": [{"x": nx, "y": ny, "z": nz}],
        })
        return result["samples"][0]["color"]

    def classify(self, point, band=0.0):
        """Return the set of region rules the point may take; more than one only within band of a swept boundary."""
        inside = self.contains(point, 0)
        if self.offset[0] == 0 and self.offset[1] == 0:
            return {1} if inside else {7}
        rules = set()
        if inside:
            if self.contains(point, self.edge_width):
                return {2}
            interior = self.swept(point, self.edge_width, band)
            for value in ((True, False) if interior is None else (interior,)):
                rules.add(3 if value else 4)
            return rules
        silhouette = self.swept(point, 0.0, band)
        for inside in ((True, False) if silhouette is None else (silhouette,)):
            if not inside:
                rules.add(7)
                continue
            interior = self.swept(point, self.edge_width, band)
            for value in ((True, False) if interior is None else (interior,)):
                rules.add(5 if value else 6)
        return rules

    def rule_color(self, rule, point):
        if rule in (1, 2, 3):
            if rule == 1 and not self.contains(point, self.edge_width):
                return self.edge
            return self.face_color(point)
        if rule in (4, 6):
            return self.edge
        if rule == 5:
            return self.pigment["side"]
        return None

    def sample(self, point):
        rules = self.classify((point["x"], point["y"]))
        (rule,) = rules
        return rule, self.rule_color(rule, (point["x"], point["y"]))

    def reserve_content(self):
        inset = self.edge_width
        outline_g = 1 - 2 ** -0.5 + 4 * EPSILON
        relief_pads = []
        tl, tr, br, bl = self.radii
        for first, second in ((tl, bl), (tr, br), (tl, tr), (bl, br)):
            relief_pads.append(max(inset, math.nextafter(outline_g * first, math.inf) if first else 0.0,
                                   math.nextafter(outline_g * second, math.inf) if second else 0.0))
        if self.kind != "flat":
            relief_pads = [max(pad, self.shoulder) for pad in relief_pads]
            for index, radius in enumerate(self.radii):
                if radius == 0:
                    continue
                tau = 2 * (self.shoulder / radius + self.shoulder / radius)
                diagonal = 1 - math.sqrt((1 - tau) * 0.5) + 8 * EPSILON
                pad = math.nextafter(radius * diagonal, math.inf)
                horizontal, vertical = ((0, 2), (1, 2), (1, 3), (0, 3))[index]
                relief_pads[horizontal] = max(relief_pads[horizontal], pad)
                relief_pads[vertical] = max(relief_pads[vertical], pad)
            if not (self.width - relief_pads[1] > relief_pads[0] and self.height - relief_pads[3] > relief_pads[2]):
                raise ValueError("content plateau must have representable positive area")
        diagonal = 1 - math.sqrt(0.5) + 8 * EPSILON

        def edge_pad(axis):
            return inset if axis <= inset else math.nextafter(inset + (axis - inset) * diagonal, math.inf)

        left = max(edge_pad(tl), edge_pad(bl), relief_pads[0])
        top = max(edge_pad(tl), edge_pad(tr), relief_pads[2])
        right = min(self.width - max(edge_pad(tr), edge_pad(br)), self.width - relief_pads[1])
        bottom = min(self.height - max(edge_pad(br), edge_pad(bl)), self.height - relief_pads[3])
        if not (right > left and bottom > top):
            raise ValueError("content rectangle must preserve representable positive area")
        bounds = {"x": left, "y": top, "width": right - left, "height": bottom - top}
        for x in (left, right):
            for y in (top, bottom):
                if not self.contains((x, y), inset) or self.normal((x, y)) != (0.0, 0.0, 1.0):
                    raise ValueError("content rectangle cannot certify uniform body containment")
        return bounds

    def color_ranges(self):
        return self.light["colorRanges"] + [{"lower": self.edge, "upper": self.edge}]


def luminance(components):
    def linear(value):
        return value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4

    red, green, blue = (linear(value) for value in components)
    return 0.2126 * red + 0.7152 * green + 0.0722 * blue


def contrast(first, second):
    lighter, darker = sorted((luminance(first), luminance(second)), reverse=True)
    return (lighter + 0.05) / (darker + 0.05)


def edge_color(request):
    adjacent = request["adjacentColor"]["components"]
    for name in ("outline", "outlineStrong"):
        if contrast(request[name]["components"], adjacent) >= request["minimumContrast"]:
            return request[name]
    raise ValueError("no edge candidate meets the minimum contrast")


def samples(source):
    body = Body(source)
    return body, [dict(zip(("rule", "color"), body.sample(point)), point=point) for point in source["points"]]


def pixels(source, width, height, origin, scale, samples_per_axis, band=None, ring=None):
    """Supersampled RGBA8 bounds: one (lower, upper) pair per pixel, equal unless a subsample is tie-bracketed.

    A ring (color, gap, stroke) adds the spec/37 focus ring: the swept silhouette dilated by gap + stroke, minus
    its dilation by gap, painted only where the body paints nothing. The outer boundary is closed, the inner open.
    """
    body = Body(source)
    if band is None:
        extent = 0 if ring is None else ring[1] + ring[2]
        band = 64 * EPSILON * max(body.width + extent, body.height + extent,
                                  abs(body.offset[0]), abs(body.offset[1]), 1.0)

    def decisions(point, outset):
        decision = body.swept(point, -outset, band)
        return (True, False) if decision is None else (decision,)

    def ring_colors(point):
        color, gap, stroke = ring
        return {tuple(color) if outer and not inner else None
                for outer in decisions(point, gap + stroke) for inner in decisions(point, gap)}

    def paints(point):
        colors = set()
        for rule in body.classify(point, band):
            color = body.rule_color(rule, point)
            if color is not None:
                colors.add(tuple(color["components"]))
            elif ring is None:
                colors.add(None)
            else:
                colors |= ring_colors(point)
        return colors

    def linear(value):
        return value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4

    def encoded(value):
        channel = 12.92 * value if value <= 0.0031308 else 1.055 * value ** (1 / 2.4) - 0.055
        return math.floor(max(0, min(1, channel)) * 255 + 0.5)

    count = samples_per_axis * samples_per_axis
    result = []
    for row in range(height):
        for column in range(width):
            candidates = []
            for sy in range(samples_per_axis):
                for sx in range(samples_per_axis):
                    point = (origin[0] + (column + (sx + 0.5) / samples_per_axis) / scale,
                             origin[1] + (row + (sy + 0.5) / samples_per_axis) / scale)
                    candidates.append(sorted(paints(point), key=lambda c: (c is not None, c or ())))
            ambiguous = sum(len(options) > 1 for options in candidates)
            if ambiguous > 8:
                raise ValueError("too many tie-bracketed subsamples in one pixel")
            outcomes = []
            for choice in itertools.product(*candidates):
                covered = [color for color in choice if color is not None]
                if not covered:
                    outcomes.append((0, 0, 0, 0))
                else:
                    outcomes.append((*(encoded(sum(linear(c[i]) for c in covered) / len(covered)) for i in range(3)),
                                     math.floor(255 * len(covered) / count + 0.5)))
            lower = tuple(min(o[i] for o in outcomes) for i in range(4))
            upper = tuple(max(o[i] for o in outcomes) for i in range(4))
            result.append((lower, upper))
    return result


def swept_contour(width, height, radii, offset):
    """Canonical spec/35 segments of a circular rounded rectangle swept by offset."""
    dx, dy = offset
    centers = ((radii[0], radii[0]), (width - radii[1], radii[1]),
               (width - radii[2], height - radii[2]), (radii[3], height - radii[3]))
    corners = ((0.0, 0.0), (width, 0.0), (width, height), (0.0, height))

    def place(p, moved):
        return {"x": p[0] + dx, "y": p[1] + dy} if moved else {"x": p[0], "y": p[1]}

    def on_arc(index, radial):
        return (centers[index][0] + radii[index] * radial[0], centers[index][1] + radii[index] * radial[1])

    portions = []
    length = math.hypot(dx, dy)
    for index in range(4):
        radius = radii[index]
        if radius > 0:
            start, end = QUARTERS[index]
            radials = [start]
            if length:
                for candidate in ((dy / length, -dx / length), (-dy / length, dx / length)):
                    if all(c != 0 and math.copysign(1, c) == math.copysign(1, s + e)
                           for c, s, e in zip(candidate, start, end)):
                        radials.append(candidate)
            radials.append(end)
            for first, second in zip(radials, radials[1:]):
                moved = (first[0] + second[0]) * dx + (first[1] + second[1]) * dy > 0
                portions.append((moved, on_arc(index, first), on_arc(index, second), {
                    "kind": "arc", "center": place(centers[index], moved), "radii": {"x": radius, "y": radius},
                    "start": {"x": first[0], "y": first[1]}, "end": {"x": second[0], "y": second[1]}}))
        normal = EDGE_NORMALS[index]
        following = (index + 1) % 4
        begin = corners[index] if radius == 0 else on_arc(index, normal)
        finish = corners[following] if radii[following] == 0 else on_arc(following, normal)
        moved = normal[0] * dx + normal[1] * dy > 0
        if begin != finish:
            portions.append((moved, begin, finish, {"kind": "line", "from": place(begin, moved), "to": place(finish, moved)}))
        elif radius == 0 and radii[following] == 0:
            raise ValueError("a square edge cannot collapse")
    segments = []
    for index, (moved, _, finish, segment) in enumerate(portions):
        segments.append(segment)
        following = portions[(index + 1) % len(portions)]
        if following[0] != moved:
            segments.append({"kind": "line", "from": place(finish, moved), "to": place(following[1], following[0])})
    return segments


def contour_difference(actual, expected, path=""):
    """First path where a published contour differs beyond the spec/35 tolerances, or None."""
    if isinstance(expected, dict):
        if not isinstance(actual, dict) or actual.keys() != expected.keys():
            return path or "/"
        return next((found for name in expected
                     if (found := contour_difference(actual[name], expected[name], f"{path}/{name}"))), None)
    if isinstance(expected, list):
        if not isinstance(actual, list) or len(actual) != len(expected):
            return path or "/"
        return next((found for index, value in enumerate(expected)
                     if (found := contour_difference(actual[index], value, f"{path}/{index}"))), None)
    if isinstance(expected, (int, float)) and not isinstance(expected, bool):
        radial = path.rpartition("/")[0].rpartition("/")[2] in ("start", "end")
        if (isinstance(actual, bool) or not isinstance(actual, (int, float))
                or not math.isclose(actual, expected, rel_tol=0 if radial else 1e-12, abs_tol=1e-12)):
            return path
        return None
    return None if actual == expected else (path or "/")


def outline_content_bounds(size, radii, minimum_inset):
    """spec/81 circular content reservation with the reference padding of g."""
    g = 1 - 2 ** -0.5 + 4 * EPSILON
    tl, tr, br, bl = radii

    def pad(*extents):
        return max([minimum_inset] + [math.nextafter(g * extent, math.inf) for extent in extents if extent])

    left, right, top, bottom = pad(tl, bl), pad(tr, br), pad(tl, tr), pad(bl, br)
    return {"x": left, "y": top, "width": size["width"] - left - right, "height": size["height"] - top - bottom}


def body_invariants(ir):
    """Return the first violated cross-member invariant of a lit control body IR, or None."""
    face, lighting = ir["face"], ir["lighting"]
    if face["light"]["keyDirection"] != lighting["direction"]:
        return "face light keyDirection must equal lighting direction"
    if face["light"]["interaction"] is not None and phase(ir["states"]["states"]) not in ("hover", "pressed"):
        return "interaction light requires the hover or pressed phase"
    depth, offset = ir["depth"], lighting["sideOffset"]
    for axis in ("x", "y"):
        if abs(offset[axis] + lighting["direction"][axis] * depth) > 1e-12 * max(1.0, depth):
            return "side offset must follow the key direction and final depth"
    size, edge = face["surface"]["outline"]["size"], face["edgeWidth"]

    def swept(bounds, inset):
        expected = {"x": min(0.0, offset["x"]), "y": min(0.0, offset["y"]),
                    "width": size["width"] - 2 * inset + abs(offset["x"]),
                    "height": size["height"] - 2 * inset + abs(offset["y"])}
        return all(abs(bounds[name] - value) <= 1e-12 * max(1.0, abs(value)) for name, value in expected.items())

    outline = face["surface"]["outline"]
    if outline["curvature"] != "circular" or any(outline["corners"][name]["x"] != outline["corners"][name]["y"] for name in PHYSICAL):
        return "lit control bodies use circular outlines"
    radii = [outline["corners"][name]["x"] for name in PHYSICAL]
    profile = face["surface"]["profile"]
    violation = relief_profile_violation(profile)
    if violation:
        return f"face surface profile: {violation}"
    if profile["kind"] != "flat" and any(
            radius > 0 and not 2 * (profile["shoulderWidth"] / radius + profile["shoulderWidth"] / radius) < 1 for radius in radii):
        return "every rounded corner must carry the shoulder with tau below 1"
    if contour_difference(outline["contentBounds"], outline_content_bounds(size, radii, edge)):
        return "the outline content reservation must use the exterior edge width as minimum inset"
    sweep = (offset["x"], offset["y"])
    silhouette = ir["geometry"]["silhouette"]
    if not swept(silhouette["bounds"], 0) or contour_difference(
            silhouette["segments"], swept_contour(size["width"], size["height"], radii, sweep)):
        return "silhouette must sweep the outline by the side offset"
    interior = ir["geometry"]["edgeInterior"]
    inset = swept_contour(size["width"] - 2 * edge, size["height"] - 2 * edge, [max(radius - edge, 0.0) for radius in radii], sweep)
    if (interior["offset"] != {"x": edge, "y": edge} or not swept(interior["contour"]["bounds"], edge)
            or contour_difference(interior["contour"]["segments"], inset)):
        return "edge interior must sweep the inset outline by the side offset"
    expected_contrast = contrast(face["foreground"]["components"], face["pigment"]["body"]["components"])
    if not math.isclose(face["contentContrastRatio"], expected_contrast, rel_tol=1e-12, abs_tol=1e-12):
        return "contentContrastRatio must be the contrast of the face foreground against the body"
    content, plateau = face["contentBounds"], face["surface"]["contentBounds"]
    if (content["x"] < plateau["x"] or content["y"] < plateau["y"]
            or content["x"] + content["width"] > plateau["x"] + plateau["width"]
            or content["y"] + content["height"] > plateau["y"] + plateau["height"]):
        return "content must lie inside the relief plateau"
    probe = Body.__new__(Body)
    probe.width, probe.height = size["width"], size["height"]
    probe.radii = [outline["corners"][name]["x"] for name in PHYSICAL]
    for x in (content["x"], content["x"] + content["width"]):
        for y in (content["y"], content["y"] + content["height"]):
            if not probe.contains((x, y), edge):
                return "content must lie inside the edge interior"
    color = face["edge"]["color"]
    if face["colorRanges"][2] != {"lower": color, "upper": color}:
        return "the third color range must be the exact edge"
    return None


def filled_label_bounds(size, padding, label_height, direction):
    """spec/46 label layout whose supplied width advance fits a body of exactly `size`."""
    advance = {"width": size["width"] * 0.01, "height": label_height}
    resolved = label_layout({"padding": padding, "minimumSize": size, "maximumSize": size,
                             "natural": advance, "fitted": advance, "direction": direction})
    if resolved["size"] != size:
        raise ValueError("label layout must reproduce the supplied body size")
    return resolved["labelBounds"]


def lit_body_source(source, size, radius, relief, edge_width, direction):
    body = copy.deepcopy(source)
    body["size"] = dict(size)
    body["radii"] = {name: {"x": radius, "y": radius} for name in LOGICAL["ltr"]}
    body["relief"] = dict(relief)
    body["edgeWidth"] = edge_width
    body["environment"]["layoutDirection"] = direction
    return body


def lit_containment_outcomes(matrix, source):
    """Yield (name, direction, contained) for every case of the lit containment matrix."""
    for case in matrix["cases"]:
        relief = dict(matrix["profile"], shoulderWidth=case["shoulderWidth"])
        for direction in ("ltr", "rtl"):
            body = Body(lit_body_source(source, case["size"], case.get("radius", matrix["radius"]), relief,
                                        case.get("edgeWidth", matrix["edgeWidth"]), direction))
            bounds = filled_label_bounds(case["size"], case["padding"], case["labelHeight"], direction)
            yield case["name"], direction, body.uniform_contains_box(bounds)


def layout_lit_outcomes(layout_request, label, source, radius, relief, edge_width, scales, lines):
    """Run spec/89 layout, spec/46 label layout with supplied advances and the lit body for each density,
    text scale, line count and direction; yield (density, scale, lines, direction, label bounds, contained).

    `label` holds the label role's unscaled font size and line height; every line is one font size
    times the line height and the supplied width is four font sizes.
    """
    for density in ("compact", "standard", "comfortable", "immersive"):
        for scale in scales:
            for count in lines:
                for direction in ("ltr", "rtl"):
                    request = copy.deepcopy(layout_request)
                    environment = request["environment"]
                    environment.update(densityPreference=density, textScale=scale, layoutDirection=direction)
                    policy = resolve_command_layout(request)
                    font_size = label["fontSize"] * scale
                    measured = {"width": font_size * 4, "height": count * (font_size * label["lineHeight"])}
                    resolved = label_layout({"padding": policy["padding"], "minimumSize": policy["minimumSize"],
                                             "maximumSize": policy["maximumSize"], "natural": measured,
                                             "fitted": measured, "direction": direction})
                    body = Body(lit_body_source(source, resolved["size"], radius, relief, edge_width, direction))
                    bounds = resolved["labelBounds"]
                    yield density, scale, count, direction, bounds, body.uniform_contains_box(bounds)


def ir_uniform_contains_box(ir, bounds):
    """spec/46 lit content containment of a label box in a published lit control body IR."""
    face = ir["face"]
    outline, profile = face["surface"]["outline"], face["surface"]["profile"]
    probe = Body.__new__(Body)
    probe.width, probe.height = outline["size"]["width"], outline["size"]["height"]
    probe.radii = [outline["corners"][name]["x"] for name in PHYSICAL]
    probe.edge_width = face["edgeWidth"]
    probe.kind, probe.shoulder = profile["kind"], profile["shoulderWidth"]
    return probe.uniform_contains_box(bounds)


def toggle_thumb_footprints(case):
    """spec/55: the thumb silhouette rectangle placed at both logical endpoints of a Toggle layout.

    The silhouette is the thumb front swept by the key side offset `-l*D` of the final depth `D`.
    """
    key = case["keyDirection"]
    length = math.hypot(key["x"], key["y"])
    offset = (-key["x"] / length * case["depth"], -key["y"] / length * case["depth"])
    track, thumb, insets = case["trackSize"], case["thumbSize"], case["insets"]
    starts = (insets["start"], track["width"] - insets["end"] - thumb["width"])
    return [{"x": start + min(0.0, offset[0]), "y": insets["top"] + min(0.0, offset[1]),
             "width": thumb["width"] + abs(offset[0]), "height": thumb["height"] + abs(offset[1])}
            for start in starts]


def toggle_lit_containment_outcomes(matrix, source):
    """Yield (name, contained): both thumb footprints lie in the track's edge interior and uniform plateau."""
    for case in matrix["cases"]:
        track = case["trackSize"]
        shape = case["trackShape"]
        radius = min(track["width"], track["height"]) / 2 if shape["kind"] == "capsule" else shape["radius"]
        body = Body(lit_body_source(source, track, radius, case["trackRelief"], matrix["edgeWidth"], "ltr"))
        yield case["name"], all(body.uniform_contains_box(box) for box in toggle_thumb_footprints(case))
