import math


def pigment_result(request):
    family = request["materialFamily"]
    profile = request["profiles"]["profiles"][family]
    body = request["body"]
    color = lambda values: {"colorSpace": "srgb", "components": values, "alpha": 1}
    return {
        "schemaVersion": "0.1.0", "materialFamily": family, "profile": profile, "body": body,
        "side": color([b * (1 - profile["sideShade"]) for b in body["components"]]),
        "highlight": color([b + profile["highlightLift"] * (1 - b) for b in body["components"]]),
    }


def unit_direction(direction):
    scale = max(abs(direction["x"]), abs(direction["y"]))
    x, y = direction["x"] / scale, direction["y"] / scale
    length = math.sqrt(x * x + y * y)
    return {"x": x / length, "y": y / length}


def expected_result(request):
    pigment = pigment_result(request["pigment"])
    light, environment = request["light"], request["environment"]
    preferences = environment["accessibilityPreferences"]
    ambient = 0 if preferences["highContrast"] else (light["ambient"] or 0)
    interaction = light["interaction"]
    allowed = (
        request["enabled"] and request["interactionActive"]
        and environment["rendererCapabilities"]["dynamicLighting"]
        and environment["qualityPolicy"] != "economy"
        and not preferences["highContrast"] and not preferences["reducedMotion"]
    )
    if not allowed or interaction is None or interaction["strength"] == 0:
        interaction = None
    else:
        interaction = {
            "direction": unit_direction(interaction["direction"]),
            "strength": interaction["strength"],
        }
    key = unit_direction(light["keyLight"]["direction"])
    body = pigment["body"]["components"]
    samples = []
    for normal in request["normals"]:
        k = max(-1, min(1, normal["x"] * key["x"] + normal["y"] * key["y"]))
        if interaction is not None:
            direction, strength = interaction["direction"], interaction["strength"]
            j = max(-1, min(1, normal["x"] * direction["x"] + normal["y"] * direction["y"]))
            k = (1 - strength) * k + strength * j
        weight = max(-1, min(1, (1 - ambient) * k))
        endpoint = pigment["highlight" if weight > 0 else "side"]["components"]
        components = body.copy() if weight == 0 else [
            value + abs(weight) * (tip - value) for value, tip in zip(body, endpoint)
        ]
        samples.append({"weight": weight, "color": {"colorSpace": "srgb", "components": components, "alpha": 1}})
    ranges = []
    for name in ("highlight", "side"):
        tip = pigment[name]["components"]
        lower = [max(0, math.nextafter(min(b, t) - 4 * math.ulp(1.0), -math.inf)) for b, t in zip(body, tip)]
        upper = [min(1, math.nextafter(max(b, t) + 4 * math.ulp(1.0), math.inf)) for b, t in zip(body, tip)]
        color = lambda values: {"colorSpace": "srgb", "components": values, "alpha": 1}
        ranges.append({"lower": color(lower), "upper": color(upper)})
    return {"schemaVersion": "0.1.0", "pigment": pigment, "keyDirection": key, "ambient": ambient,
            "interaction": interaction, "colorRanges": ranges, "samples": samples}
