"""Independent oracle for the Command layout profile resolution."""
from fractions import Fraction
import math
import re

from command_label_arithmetic import extent_sum

INSET_ROLES = {"space.control.inset.inline", "space.control.inset.block"}
MODES = ("compact", "standard", "comfortable", "immersive")
ALIAS = re.compile(r"^\{([^{}]+)\}$")


def binary64_product(left, right):
    try:
        product = float(Fraction.from_float(float(left)) * Fraction.from_float(float(right)))
    except OverflowError:
        product = math.inf
    if not math.isfinite(product):
        raise ValueError("text-scaled maximum size is not finite")
    return product


def find_token(document, path):
    node, group_type = document, None
    for segment in path.split("."):
        if not isinstance(node, dict) or segment.startswith("$") or segment not in node:
            return None, None
        group_type = node.get("$type", group_type)
        node = node[segment]
    if not isinstance(node, dict) or "$value" not in node:
        return None, None
    return node, node.get("$type", group_type)


def resolve_token(document, path, seen=(), label="command layout"):
    node, token_type = find_token(document, path)
    if node is None:
        return None
    value = node["$value"]
    alias = ALIAS.match(value) if isinstance(value, str) else None
    if alias:
        if path in seen or alias.group(1) in seen:
            raise ValueError(f"{label} token resolution failed: alias cycle")
        target = resolve_token(document, alias.group(1), seen + (path,), label)
        if target is None:
            raise ValueError(f"{label} token resolution failed: unresolved alias")
        return token_type or target[0], target[1]
    return token_type, value


def extent_token(document, path):
    token = resolve_token(document, path)
    if token is None:
        raise ValueError(f"missing extent token {path}")
    token_type, value = token
    if token_type != "dimension":
        raise ValueError(f"extent token {path} must be dimension, found {token_type}")
    if not (isinstance(value, dict) and value.keys() == {"value", "unit"}
            and isinstance(value["value"], (int, float)) and not isinstance(value["value"], bool)
            and value["unit"] in ("px", "rem")):
        raise ValueError("command layout token resolution failed: invalid dimension")
    if value["unit"] != "px":
        raise ValueError(f"extent token {path} must use px")
    if not value["value"] > 0:
        raise ValueError(f"extent token {path} must be finite and positive")
    return value["value"]


def resolve_extents(profile, document):
    extents = {}
    for mode in MODES:
        authored = profile["extents"][mode]
        minimum, maximum = (
            {axis: extent_token(document, authored[size][axis]) for axis in ("width", "height")}
            for size in ("minimumSize", "maximumSize")
        )
        if minimum["width"] > maximum["width"] or minimum["height"] > maximum["height"]:
            raise ValueError(f"{mode.capitalize()} minimumSize must not exceed maximumSize")
        extents[mode] = minimum, maximum
    return extents


def resolve(request):
    environment = request["environment"]
    density = environment["densityPreference"]
    profile = request["profile"]
    minimum, authored = resolve_extents(profile, request["tokens"])[density]
    available = request["availableSize"]

    def inset(edge):
        role = profile["padding"][edge]
        value = request["spatialBinding"][role]
        if role in INSET_ROLES:
            value = value[density]
        if value["unit"] != "px":
            raise ValueError(f"{role} must resolve to a px dimension")
        return value["value"]

    padding = {edge: inset(edge) for edge in ("start", "end", "top", "bottom")}
    factor = max(1, environment["textScale"])
    scaled = {axis: binary64_product(authored[axis], factor) for axis in ("width", "height")}
    if available["width"] < minimum["width"] or available["height"] < minimum["height"]:
        raise ValueError("availableSize is smaller than the profile minimum")
    maximum = {axis: min(scaled[axis], available[axis]) for axis in ("width", "height")}
    for left, right, axis in (("start", "end", "width"), ("top", "bottom", "height")):
        try:
            aggregate = extent_sum(padding[left], padding[right])
        except ValueError:
            raise ValueError("profile insets leave no label extent") from None
        if aggregate >= maximum[axis]:
            raise ValueError("profile insets leave no label extent")
    coarse = {"coarsePointer", "directTouch"} & set(environment["inputCapabilities"])
    target = 48 if coarse else 24
    return {
        "schemaVersion": "0.1.0",
        "densityPreference": density,
        "layoutDirection": environment["layoutDirection"],
        "minimumSize": dict(minimum),
        "maximumSize": maximum,
        "padding": padding,
        "targetMinimum": {"width": target, "height": target},
    }
