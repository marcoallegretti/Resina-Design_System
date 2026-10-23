"""Independent oracle for the Toggle layout profile resolution."""
from fractions import Fraction
import math

from command_layout_arithmetic import MODES, resolve_token

LABEL = "toggle profile token resolution failed"


def rounded(value):
    """Round an exact rational to binary64; an unrepresentably large value is infinite."""
    try:
        return float(value)
    except OverflowError:
        return math.inf


def difference(left, right):
    """Binary64 subtraction as spec/53 requires: a positive term must not be lost."""
    if right > left:
        raise ValueError("thumb must fit within the inset track")
    result = rounded(Fraction(left) - Fraction(right))
    if right > 0 and result == left:
        raise ValueError("toggle layout exceeds representable arithmetic")
    return result


def dimension(document, path, allow_zero):
    token = resolve_token(document, path, label=LABEL)
    if token is None:
        raise ValueError(f"missing toggle token {path}")
    token_type, value = token
    if token_type != "dimension":
        raise ValueError(f"toggle token {path} must be dimension, found {token_type}")
    if not (isinstance(value, dict) and value.keys() == {"value", "unit"}
            and isinstance(value["value"], (int, float)) and not isinstance(value["value"], bool)
            and value["unit"] in ("px", "rem")):
        raise ValueError(f"{LABEL}: invalid dimension")
    if value["unit"] != "px":
        raise ValueError(f"toggle token {path} must use px")
    number = value["value"]
    try:
        float(number)
    except OverflowError:
        number = -1
    if not (number >= 0 if allow_zero else number > 0):
        raise ValueError(f"toggle token {path} must be finite and {'nonnegative' if allow_zero else 'positive'}")
    return number


def allocation(track, thumb, inset):
    """The checks of the horizontal Toggle part layout for a uniform inset, in its order."""
    interior_width = difference(difference(float(track["width"]), float(inset)), float(inset))
    interior_height = difference(difference(float(track["height"]), float(inset)), float(inset))
    travel = difference(interior_width, float(thumb["width"]))
    slack = difference(interior_height, float(thumb["height"]))
    if travel <= 0:
        raise ValueError("toggle requires distinct off/on travel endpoints")
    half = rounded(Fraction(slack) / 2)
    y = rounded(Fraction(float(inset)) + Fraction(half))
    if not math.isfinite(y) or (slack > 0 and (half == 0 or y == inset)):
        raise ValueError("toggle layout exceeds representable arithmetic")
    end = difference(difference(float(track["width"]), float(inset)), float(thumb["width"]))
    if inset == end:
        raise ValueError("toggle layout exceeds representable arithmetic")
    for x in (inset, end):
        right = rounded(Fraction(x) + Fraction(float(thumb["width"])))
        bottom = rounded(Fraction(y) + Fraction(float(thumb["height"])))
        if not (math.isfinite(right) and math.isfinite(bottom)) or right <= x or bottom <= y:
            raise ValueError("toggle layout exceeds representable arithmetic")
        if right > track["width"] or bottom > track["height"]:
            raise ValueError("thumb must fit within the inset track")


def resolve_parts(profile, document):
    parts = {}
    for mode in MODES:
        authored = profile["parts"][mode]
        track, thumb = (
            {axis: dimension(document, authored[name][axis], False) for axis in ("width", "height")}
            for name in ("trackSize", "thumbSize")
        )
        inset = dimension(document, authored["thumbInset"], True)
        try:
            allocation(track, thumb, inset)
        except ValueError as error:
            raise ValueError(f"{mode.capitalize()} track and thumb cannot be allocated: {error}") from None
        parts[mode] = track, thumb, inset
    return parts


def resolve(request):
    environment = request["environment"]
    available = request["availableSize"]
    parts = resolve_parts(request["profile"], request["tokens"])
    density = environment["densityPreference"]
    track, thumb, inset = parts[density]
    if available["width"] < track["width"] or available["height"] < track["height"]:
        raise ValueError("availableSize is smaller than the profile track")
    return {
        "schemaVersion": "0.1.0",
        "densityPreference": density,
        "layoutDirection": environment["layoutDirection"],
        "trackSize": dict(track),
        "thumbSize": dict(thumb),
        "insets": {edge: inset for edge in ("start", "end", "top", "bottom")},
    }
