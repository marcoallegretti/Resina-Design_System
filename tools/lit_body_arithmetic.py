"""Independent lit control body arithmetic with circular corners and swept regions."""
import itertools
import math
from fractions import Fraction

from material_light_arithmetic import expected_result, pigment_result

EPSILON = 2.0 ** -52
PHYSICAL = ("topLeft", "topRight", "bottomRight", "bottomLeft")
LOGICAL = {
    "ltr": ("topStart", "topEnd", "bottomEnd", "bottomStart"),
    "rtl": ("topEnd", "topStart", "bottomStart", "bottomEnd"),
}
SIGNS = ((-1, -1), (1, -1), (1, 1), (-1, 1))
STATES = {"rest", "hover", "pressed", "disabled", "focused", "checked"}


def phase(states):
    unsupported = set(states) - STATES
    if unsupported:
        raise ValueError(f"unsupported lit control state {sorted(unsupported)[0]}")
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
    for name in LOGICAL[source["environment"]["layoutDirection"]]:
        x, y = radii[name]["x"] * factor, radii[name]["y"] * factor
        if x == 0 or y == 0:
            x = y = 0.0
        if x != y:
            raise ValueError("lit control bodies require circular corners with equal extents")
        result.append(x)
    return result


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
        if self.kind == "flat":
            if self.amplitude != 0 or self.shoulder != 0:
                raise ValueError("flat requires zero amplitude and shoulderWidth")
        elif not (0 < self.amplitude <= self.shoulder / 2):
            raise ValueError("relief requires positive amplitude no greater than shoulderWidth/2")
        elif self.kind == "recessed" and self.amplitude >= profile["bodyThickness"]:
            raise ValueError("recessed amplitude must stay below bodyThickness")
        if self.kind != "flat":
            for name, radius in zip(PHYSICAL, self.radii):
                if radius > 0 and 4 * self.shoulder / radius >= 1:
                    raise ValueError(
                        f"{name} corner radius {radius} px cannot carry shoulderWidth {self.shoulder} px: tau must be below 1"
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
        x, y = Fraction(x), Fraction(y)
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


def pixels(source, width, height, origin, scale, samples_per_axis, band=None):
    """Supersampled RGBA8 bounds: one (lower, upper) pair per pixel, equal unless a subsample is tie-bracketed."""
    body = Body(source)
    if band is None:
        band = 64 * EPSILON * max(body.width, body.height, abs(body.offset[0]), abs(body.offset[1]), 1.0)

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
                    colors = {None if (c := body.rule_color(rule, point)) is None else tuple(c["components"])
                              for rule in body.classify(point, band)}
                    candidates.append(sorted(colors, key=lambda c: (c is not None, c or ())))
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
