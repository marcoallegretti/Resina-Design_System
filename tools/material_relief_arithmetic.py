import math

from material_light_arithmetic import expected_result


def rectangular_normal(request, point):
    if any(axis != 0 for corner in request["outline"]["radii"].values() for axis in corner.values()):
        raise ValueError("rectangular proof requires square corners")
    size, profile = request["outline"]["size"], request["profile"]
    x, y = point["x"], point["y"]
    width, height = size["width"], size["height"]
    if not (0 <= x <= width and 0 <= y <= height):
        return None
    if profile["kind"] == "flat":
        return {"x": 0, "y": 0, "z": 1}
    shoulder = profile["shoulderWidth"]

    def gate(distance, sign):
        t = distance / shoulder
        if t <= 0:
            return 0, 0
        if t >= 1:
            return 1, 0
        return t ** 3 * (10 - 15 * t + 6 * t * t), sign * 30 * t * t * (1 - t) ** 2

    gates = [gate(x, 1), gate(width - x, -1), gate(y, 1), gate(height - y, -1)]
    amplitude = profile["amplitude"] * (-1 if profile["kind"] == "recessed" else 1)
    slopes = []
    for indices in ((0, 1), (2, 3)):
        slope = sum(gates[index][1] * math.prod(value for other, (value, _) in enumerate(gates) if other != index) for index in indices)
        slopes.append(amplitude / shoulder * slope)
    length = math.sqrt(1 + sum(value * value for value in slopes))
    return {"x": -slopes[0] / length, "y": -slopes[1] / length, "z": 1 / length}


def rectangular_samples(request):
    normals = [rectangular_normal(request, point) for point in request["points"]]
    light_request = {name: request[name] for name in ("schemaVersion", "light", "environment", "enabled", "interactionActive", "pigment")}
    light_request["normals"] = [normal for normal in normals if normal is not None]
    light = expected_result(light_request)
    colors = iter(sample["color"] for sample in light["samples"])
    width, height = (request["outline"]["size"][name] for name in ("width", "height"))
    inset = request["edgeWidth"]
    edge = request["edge"]["outline"]
    if request["edge"]["outlineStrong"] != edge:
        raise ValueError("rectangular proof requires an unambiguous edge candidate")
    result = []
    for point, normal in zip(request["points"], normals):
        color = None if normal is None else next(colors)
        if normal is not None and not (inset <= point["x"] <= width - inset and inset <= point["y"] <= height - inset):
            color = edge
        result.append({"point": point, "color": color})
    return light, result


def rectangular_pixels(request, width, height, origin, scale, samples):
    points = [
        {"x": origin[0] + (x + (sx + 0.5) / samples) / scale,
         "y": origin[1] + (y + (sy + 0.5) / samples) / scale}
        for y in range(height) for x in range(width)
        for sy in range(samples) for sx in range(samples)
    ]
    _, colors = rectangular_samples(dict(request, points=points))

    def linear(value):
        return value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4

    def encoded(value):
        channel = 12.92 * value if value <= 0.0031308 else 1.055 * value ** (1 / 2.4) - 0.055
        return math.floor(max(0, min(1, channel)) * 255 + 0.5)

    pixels = []
    count = samples * samples
    for start in range(0, len(colors), count):
        covered = [entry["color"]["components"] for entry in colors[start:start + count] if entry["color"] is not None]
        if not covered:
            pixels.append((0, 0, 0, 0))
        else:
            channels = [encoded(sum(linear(color[channel]) for color in covered) / len(covered)) for channel in range(3)]
            pixels.append((*channels, math.floor(255 * len(covered) / count + 0.5)))
    return pixels
