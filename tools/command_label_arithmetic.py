"""Independent exact-rational oracle for the portable binary64 label policy."""
from fractions import Fraction
import math


def directed(exact, outward):
    rounded = float(exact)
    if not math.isfinite(rounded):
        raise ValueError("nonfinite extent")
    represented = Fraction.from_float(rounded)
    if (outward and represented < exact) or (not outward and represented > exact):
        rounded = math.nextafter(rounded, math.inf if outward else -math.inf)
    if not math.isfinite(rounded):
        raise ValueError("nonfinite directed extent")
    return rounded


def extent_sum(left, right):
    nearest = left + right
    if not math.isfinite(nearest) or (left > 0 and nearest <= right) or (right > 0 and nearest <= left):
        raise ValueError("lost positive term or nonfinite sum")
    return directed(Fraction.from_float(float(left)) + Fraction.from_float(float(right)), True)


def extent_remainder(total, padding):
    nearest = total - padding
    if not math.isfinite(nearest) or nearest <= 0 or (padding > 0 and nearest == total):
        raise ValueError("lost padding or nonpositive remainder")
    result = directed(Fraction.from_float(float(total)) - Fraction.from_float(float(padding)), False)
    if result <= 0:
        raise ValueError("nonpositive directed remainder")
    return result


def layout(case):
    padding = case["padding"]
    horizontal = extent_sum(padding["start"], padding["end"])
    vertical = extent_sum(padding["top"], padding["bottom"])
    minimum, maximum = case["minimumSize"], case["maximumSize"]
    if horizontal >= maximum["width"] or vertical >= maximum["height"]:
        raise ValueError("padding leaves no extent")
    width = min(max(extent_sum(case["natural"]["width"], horizontal), minimum["width"]), maximum["width"])
    offered = extent_remainder(width, horizontal)
    fitted = case["fitted"]
    if fitted["width"] > offered:
        raise ValueError("fitted label exceeds offer")
    height = max(extent_sum(fitted["height"], vertical), minimum["height"])
    if height > maximum["height"]:
        raise ValueError("complete label exceeds maximum height")
    remaining = extent_remainder(height, vertical)
    if remaining < fitted["height"]:
        raise ValueError("complete height lost")
    return {"size": {"width": width, "height": height},
            "labelBounds": {"x": padding["start"] if case["direction"] == "ltr" else padding["end"],
                            "y": padding["top"] + (remaining - fitted["height"]) * .5,
                            "width": offered, "height": fitted["height"]}}


def check_cases(document):
    names = set()
    for case in document["cases"]:
        if case["name"] in names:
            raise ValueError("duplicate label arithmetic case")
        names.add(case["name"])
        try:
            actual = layout(case)
        except (ValueError, OverflowError):
            if "error" not in case:
                raise
        else:
            if actual != case.get("expected"):
                raise ValueError(f"{case['name']}: expected outcome disagrees with exact arithmetic")
    return len(names)
