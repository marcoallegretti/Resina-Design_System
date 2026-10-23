import math
import sys
from decimal import Decimal, getcontext
from fractions import Fraction

from check_command_motion_backend import equation_sample
from check_schemas import ROOT, load_json

getcontext().prec = 60

LIMITS = load_json(ROOT / "definitions/motion/surface-deformation-limits.json")["limits"]
PADDING = {
    "circular": Fraction(1 - Decimal("0.5").sqrt()),
    "quartic": Fraction(1 - Decimal("0.5").sqrt().sqrt()),
}
PHYSICAL = {
    "ltr": ("topStart", "topEnd", "bottomEnd", "bottomStart"),
    "rtl": ("topEnd", "topStart", "bottomStart", "bottomEnd"),
}
MAXIMUM_FLOAT = Fraction(sys.float_info.max)


class Reject(Exception):
    pass


def exact(value):
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise Reject("a number is required")
    try:
        return Fraction(value)
    except (ValueError, OverflowError):
        raise Reject("numbers must be finite")


def valid_dynamics(dynamics):
    for name in ("mass", "stiffness", "positionThreshold", "velocityThreshold"):
        if exact(dynamics[name]) <= 0:
            raise Reject(f"{name} must be positive")
    if exact(dynamics["damping"]) < 0:
        raise Reject("damping must be nonnegative")


def valid_time(time):
    if exact(time) < 0:
        raise Reject("time must be finite and nonnegative")


def policy_of(request):
    if request["family"] == "gel":
        raise Reject("Gel has no calibrated dynamics")
    if request["reducedMotion"]:
        return "reducedMotion"
    return "castImmediate" if request["family"] == "cast" else "spring"


def project(value, lower, upper):
    if value < lower:
        return lower, "lowerBound"
    if value > upper:
        return upper, "upperBound"
    return value, "none"


def representable(value):
    return abs(value) <= MAXIMUM_FLOAT


def sample(channel, target, time, policy):
    dynamics = channel["dynamics"]
    valid_dynamics(dynamics)
    position, velocity = exact(channel["initial"]["position"]), exact(channel["initial"]["velocity"])
    if policy == "spring":
        omega = Fraction(math.sqrt(dynamics["stiffness"] / dynamics["mass"]))
        displacement = position - exact(target)
        energy_bound_squared = omega**2 * displacement**2 + velocity**2
        if (
            not representable(displacement)
            or not representable(velocity / omega)
            or not representable(omega * exact(time))
            or energy_bound_squared > MAXIMUM_FLOAT**2
        ):
            raise Reject("spring arithmetic exceeds the representable range")
    result = equation_sample(channel, target, time, policy != "spring")
    if not all(math.isfinite(result["state"][name]) for name in ("position", "velocity")):
        raise Reject("spring result exceeds the representable range")
    return result


def outline(curvature, direction, radii, inset, width, height):
    if width <= 0 or height <= 0 or inset < 0:
        raise Reject("an outline needs positive area")
    if width > MAXIMUM_FLOAT or height > MAXIMUM_FLOAT:
        raise Reject("width and height must be finite")
    sums = [
        (width, radii["topStart"][0] + radii["topEnd"][0]),
        (width, radii["bottomStart"][0] + radii["bottomEnd"][0]),
        (height, radii["topStart"][1] + radii["bottomStart"][1]),
        (height, radii["topEnd"][1] + radii["bottomEnd"][1]),
    ]
    scale = min([Fraction(1)] + [limit / total for limit, total in sums if total > limit])
    corners = {}
    for name, (x, y) in radii.items():
        x, y = x * scale, y * scale
        corners[name] = (Fraction(0), Fraction(0)) if x == 0 or y == 0 else (x, y)
    top_left, top_right, bottom_right, bottom_left = (corners[name] for name in PHYSICAL[direction])

    def guard(first, second):
        extent = max(first, second)
        return inset if extent == 0 else max(inset, PADDING[curvature] * extent)

    left = guard(top_left[0], bottom_left[0])
    right = guard(top_right[0], bottom_right[0])
    top = guard(top_left[1], top_right[1])
    bottom = guard(bottom_right[1], bottom_left[1])
    content = (left, top, width - right - left, height - bottom - top)
    if content[2] <= 0 or content[3] <= 0:
        raise Reject("content bounds need positive area")
    return {
        "schemaVersion": "0.1.0",
        "curvature": curvature,
        "size": {"width": width, "height": height},
        "corners": {
            "topLeft": top_left,
            "topRight": top_right,
            "bottomRight": bottom_right,
            "bottomLeft": bottom_left,
        },
        "contentBounds": dict(zip(("x", "y", "width", "height"), content)),
    }


def source_radii(outline_request):
    return {name: (exact(corner["x"]), exact(corner["y"])) for name, corner in outline_request["radii"].items()}


def rest_outline(outline_request, width, height, radii=None, inset=None):
    return outline(
        outline_request["curvature"],
        outline_request["layoutDirection"],
        source_radii(outline_request) if radii is None else radii,
        exact(outline_request["minimumInset"]) if inset is None else inset,
        width,
        height,
    )


def published(result):
    def number(value):
        return float(value)

    def convert(value):
        if isinstance(value, Fraction):
            return number(value)
        if isinstance(value, tuple):
            return {"x": number(value[0]), "y": number(value[1])}
        if isinstance(value, dict):
            return {key: convert(item) for key, item in value.items()}
        return value

    return convert(result)


def rectangle(box):
    return tuple(exact(box[name]) for name in ("x", "y", "width", "height"))


def contained(box, resolved, origin):
    x, y, width, height = box
    bounds = resolved["contentBounds"]
    left, top = origin[0] + bounds["x"], origin[1] + bounds["y"]
    return (
        x >= left
        and y >= top
        and x + width <= left + bounds["width"]
        and y + height <= top + bounds["height"]
    )


def reservation(rectangles, sweep):
    shifted = [(x + sweep[0], y + sweep[1], w, h) for x, y, w, h in rectangles]
    boxes = list(rectangles) + shifted
    left = min(box[0] for box in boxes)
    top = min(box[1] for box in boxes)
    right = max(box[0] + box[2] for box in boxes)
    bottom = max(box[1] + box[3] for box in boxes)
    return {"x": left, "y": top, "width": right - left, "height": bottom - top}


def sweep_of(request):
    return exact(request["sweepOffset"]["x"]), exact(request["sweepOffset"]["y"])


def deformation_domain(request):
    size = request["outline"]["size"]
    width, height = exact(size["width"]), exact(size["height"])
    limit = LIMITS.get(request["family"], 0.0)
    domains = {}
    for name in ("widthScale", "heightScale"):
        lower, upper = (exact(request["bounds"][name][side]) for side in ("lower", "upper"))
        if lower <= 0 or lower > 1 or upper < 1:
            raise Reject(f"{name} bounds must contain rest")
        if request["bounds"][name]["lower"] < 1.0 - limit or request["bounds"][name]["upper"] > 1.0 + limit:
            raise Reject(f"{name} bounds exceed the family deformation limit")
        target = exact(request["channels"][name]["target"])
        if target < lower or target > upper:
            raise Reject(f"{name} target is outside its bounds")
        domains[name] = (lower, upper)
    return width, height, domains


def place(request, width, height, scale_x, scale_y):
    sized = (width * scale_x, height * scale_y)
    origin = ((width - sized[0]) / 2, (height - sized[1]) / 2)
    return origin, rest_outline(request["outline"], sized[0], sized[1])


def deformation(request, accepted=None):
    policy = policy_of(request)
    if request["schemaVersion"] != "0.1.0":
        raise Reject("unsupported version")
    valid_time(request["time"])
    width, height, domains = deformation_domain(request)
    box = rectangle(request["content"])
    sweep = sweep_of(request)
    if box[2] <= 0 or box[3] <= 0:
        raise Reject("content needs positive area")
    for scale_x in domains["widthScale"]:
        for scale_y in domains["heightScale"]:
            origin, resolved = place(request, width, height, scale_x, scale_y)
            if not contained(box, resolved, origin):
                raise Reject("content leaves the uniform face inside the deformation domain")
    samples, positions, projection = {}, {}, {}
    for name in ("widthScale", "heightScale"):
        channel = request["channels"][name]
        oracle = sample(channel, channel["target"], request["time"], policy)
        samples[name] = oracle if accepted is None else accepted[name]
        position = exact(samples[name]["state"]["position"])
        positions[name], projection[name] = project(position, *domains[name])
    origin, resolved = place(request, width, height, positions["widthScale"], positions["heightScale"])
    maximum_origin, maximum = place(request, width, height, domains["widthScale"][1], domains["heightScale"][1])
    maximum_box = (*maximum_origin, maximum["size"]["width"], maximum["size"]["height"])
    return {
        "schemaVersion": "0.1.0",
        "policy": policy,
        "widthScale": samples["widthScale"],
        "heightScale": samples["heightScale"],
        "projection": projection,
        "origin": published({"x": origin[0], "y": origin[1]}),
        "outline": published(resolved),
        "reservation": published(reservation([maximum_box], sweep)),
    }


def lerp(first, second, weight):
    return first * (1 - weight) + second * weight


def emergence_ends(request):
    ends = [request["from"], request["to"]]
    if request["from"]["identity"] != request["to"]["identity"]:
        raise Reject("both states must name one identity")
    first, second = (end["outline"] for end in ends)
    if first["layoutDirection"] != second["layoutDirection"] or first["curvature"] != second["curvature"]:
        raise Reject("outlines must share direction and curvature")
    return ends


def emergence_outline(request, weight):
    first, second = (request[name]["outline"] for name in ("from", "to"))
    origins = [(exact(request[name]["origin"]["x"]), exact(request[name]["origin"]["y"])) for name in ("from", "to")]
    radii = {
        name: tuple(lerp(exact(first["radii"][name][axis]), exact(second["radii"][name][axis]), weight) for axis in "xy")
        for name in first["radii"]
    }
    origin = tuple(lerp(origins[0][axis], origins[1][axis], weight) for axis in (0, 1))
    size = tuple(lerp(exact(first["size"][axis]), exact(second["size"][axis]), weight) for axis in ("width", "height"))
    inset = lerp(exact(first["minimumInset"]), exact(second["minimumInset"]), weight)
    return origin, outline(first["curvature"], first["layoutDirection"], radii, inset, size[0], size[1])


def first_unfitted_weight(request, end, steps):
    box = rectangle(request[end]["content"])
    for step in range(steps + 1):
        weight = Fraction(step, steps)
        try:
            origin, resolved = emergence_outline(request, weight)
        except Reject:
            return weight
        if not contained(box, resolved, origin):
            return weight
    return None


def emergence(request, accepted=None):
    policy = policy_of(request)
    if request["schemaVersion"] != "0.1.0":
        raise Reject("unsupported version")
    valid_time(request["time"])
    ends = emergence_ends(request)
    for end in ends:
        end_size = end["outline"]["size"]
        resolved_end = rest_outline(end["outline"], exact(end_size["width"]), exact(end_size["height"]))
        end_box = rectangle(end["content"])
        if end_box[2] <= 0 or end_box[3] <= 0:
            raise Reject("content needs positive area")
        if not contained(end_box, resolved_end, (exact(end["origin"]["x"]), exact(end["origin"]["y"]))):
            raise Reject("content leaves the uniform face of its own outline")
    for name in ("progress", "opacity"):
        valid_dynamics(request["channels"][name]["dynamics"])
    fade = request["channels"]["opacity"]["dynamics"]
    if float(fade["damping"]) * float(fade["damping"]) < 4.0 * float(fade["mass"]) * float(fade["stiffness"]):
        raise Reject("opacity dynamics must not overshoot")
    target = 1 if request["emerged"] else 0
    samples, positions, projection = {}, {}, {}
    for name in ("progress", "opacity"):
        oracle = sample(request["channels"][name], target, request["time"], policy)
        samples[name] = oracle if accepted is None else accepted[name]
        positions[name], projection[name] = project(exact(samples[name]["state"]["position"]), Fraction(0), Fraction(1))
    origin, resolved = emergence_outline(request, positions["progress"])
    sizes = [(exact(end["outline"]["size"]["width"]), exact(end["outline"]["size"]["height"])) for end in ends]
    origins = [(exact(end["origin"]["x"]), exact(end["origin"]["y"])) for end in ends]
    opacity = positions["opacity"]
    contents = {}
    for name, end, visible in (("fromContent", ends[0], 1 - opacity), ("toContent", ends[1], opacity)):
        box = rectangle(end["content"])
        if visible == 0:
            contents[name] = None
            continue
        if not contained(box, resolved, origin):
            raise Reject("visible content leaves the uniform face")
        contents[name] = {"bounds": published(dict(zip(("x", "y", "width", "height"), box))), "opacity": float(visible)}
    covered = [(*origins[index], *sizes[index]) for index in (0, 1)]
    return {
        "schemaVersion": "0.1.0",
        "policy": policy,
        "identity": request["from"]["identity"],
        "progress": samples["progress"],
        "opacity": samples["opacity"],
        "projection": projection,
        "origin": published({"x": origin[0], "y": origin[1]}),
        "outline": published(resolved),
        "fromContent": contents["fromContent"],
        "toContent": contents["toContent"],
        "reservation": published(reservation(covered, sweep_of(request))),
    }
