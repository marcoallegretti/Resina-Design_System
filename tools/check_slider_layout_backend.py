from fractions import Fraction
import math

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json
from check_slider_value_backend import slider_value_mismatch


def layout_expected(request):
    number = lambda value: Fraction(float(value))
    width, height = (number(request["allocationSize"][field]) for field in ("width", "height"))
    thumb_width, thumb_height = (number(request["thumbSize"][field]) for field in ("width", "height"))
    start, end, top, bottom = (number(request["insets"][field])
                               for field in ("start", "end", "top", "bottom"))
    left, right = (start, end) if request["layoutDirection"] == "ltr" else (end, start)
    horizontal = request["orientation"] == "horizontal"
    if horizontal:
        low, high = left, width - right - thumb_width
        cross = top + (height - top - bottom - thumb_height) / 2
        track_cross = top + (height - top - bottom - number(request["trackThickness"])) / 2
        main_extent, cross_extent = thumb_width, thumb_height
    else:
        low, high = top, height - bottom - thumb_height
        cross = left + (width - left - right - thumb_width) / 2
        track_cross = left + (width - left - right - number(request["trackThickness"])) / 2
        main_extent, cross_extent = thumb_height, thumb_width
    minimum, maximum = low, high
    if horizontal and request["layoutDirection"] == "rtl":
        minimum, maximum = maximum, minimum
    if request["minimumPosition"] == "end":
        minimum, maximum = maximum, minimum
    value = request["value"]
    lower, upper, current = (number(value[field]) for field in ("minimum", "maximum", "value"))
    progress = (current - lower) / (upper - lower)

    def bounds(main, cross, main_extent, cross_extent):
        coordinates = (main, cross, main_extent, cross_extent) if horizontal else (
            cross, main, cross_extent, main_extent)
        return dict(zip(("x", "y", "width", "height"), map(float, coordinates)))

    return {
        "schemaVersion": "0.1.0",
        **{field: request[field] for field in ("layoutDirection", "orientation", "minimumPosition")},
        "value": {**value, "progress": float(progress)},
        "allocationBounds": {"x": 0, "y": 0, "width": float(width), "height": float(height)},
        "trackBounds": bounds(low + main_extent / 2, track_cross, high - low,
                               number(request["trackThickness"])),
        "minimumThumbBounds": bounds(minimum, cross, main_extent, cross_extent),
        "maximumThumbBounds": bounds(maximum, cross, main_extent, cross_extent),
        "thumbBounds": bounds(minimum + (maximum - minimum) * progress, cross,
                               main_extent, cross_extent),
    }


def layout_mismatch(actual, expected, request=None):
    for field in ("schemaVersion", "layoutDirection", "orientation", "minimumPosition", "allocationBounds"):
        if actual.get(field) != expected[field]:
            return "/" + field
    different = slider_value_mismatch(actual["value"], expected["value"])
    if different:
        return "/value" + different
    for field in ("minimumThumbBounds", "maximumThumbBounds", "thumbBounds"):
        for extent in ("width", "height"):
            if actual[field][extent] != expected[field][extent]:
                return f"/{field}/{extent}"
    axis = "x" if expected["orientation"] == "horizontal" else "y"
    cross_axis = "y" if axis == "x" else "x"
    cross_extent = "height" if axis == "x" else "width"
    if actual["trackBounds"][cross_extent] != expected["trackBounds"][cross_extent]:
        return "/trackBounds/" + cross_extent
    for field in ("maximumThumbBounds", "thumbBounds"):
        if actual[field][cross_axis] != actual["minimumThumbBounds"][cross_axis]:
            return f"/{field}/{cross_axis}"
    allocation = actual["allocationBounds"]
    interiors = [(Fraction(0), Fraction(0), Fraction(allocation["width"]),
                  Fraction(allocation["height"]))]
    if request is not None:
        insets = request["insets"]
        left = insets["start"] if request["layoutDirection"] == "ltr" else insets["end"]
        left, top = float(left), float(insets["top"])
        width = (float(allocation["width"]) - float(insets["start"])) - float(insets["end"])
        height = (float(allocation["height"]) - top) - float(insets["bottom"])
        interiors.append((Fraction(left), Fraction(top), Fraction(left) + Fraction(width),
                          Fraction(top) + Fraction(height)))
    for field in ("trackBounds", "minimumThumbBounds", "maximumThumbBounds", "thumbBounds"):
        bounds = actual[field]
        x, y, width, height = (Fraction(bounds[c]) for c in ("x", "y", "width", "height"))
        if any(x < left or y < top or x + width > right or y + height > bottom
               for left, top, right, bottom in interiors):
            return "/" + field
    minimum = actual["minimumThumbBounds"][axis]
    maximum = actual["maximumThumbBounds"][axis]
    expected_minimum = expected["minimumThumbBounds"][axis]
    expected_maximum = expected["maximumThumbBounds"][axis]
    if minimum == maximum or (minimum < maximum) != (expected_minimum < expected_maximum):
        return "/maximumThumbBounds/" + axis
    extent = "width" if axis == "x" else "height"
    half_thumb = actual["minimumThumbBounds"][extent] / 2
    if minimum + half_thumb == maximum + half_thumb:
        return "/maximumThumbBounds/" + axis
    progress = actual["value"]["progress"]
    if progress in (0, 1):
        endpoint = "minimumThumbBounds" if progress == 0 else "maximumThumbBounds"
        if actual["thumbBounds"] != actual[endpoint]:
            return "/thumbBounds"
    elif not min(minimum, maximum) < actual["thumbBounds"][axis] < max(minimum, maximum):
        return "/thumbBounds/" + axis
    for field in ("trackBounds", "minimumThumbBounds", "maximumThumbBounds", "thumbBounds"):
        for coordinate in ("x", "y", "width", "height"):
            if not math.isclose(actual[field][coordinate], expected[field][coordinate],
                                rel_tol=1e-12, abs_tol=1e-12):
                return f"/{field}/{coordinate}"
    return None


def main():
    cases = load_json(ROOT / "conformance/geometry/slider-layout-protocol-cases.json")
    for case in cases:
        if "expected" in case and case["expected"] != layout_expected(case["request"]):
            raise ValueError(f"{case['name']}: public geometry differs from rational oracle")
    baseline = next(case["request"] for case in cases if "expected" in case)
    numeric = ("/allocationSize/width", "/allocationSize/height", "/thumbSize/width",
               "/thumbSize/height", "/trackThickness", "/insets/start", "/insets/end",
               "/insets/top", "/insets/bottom", "/value/minimum", "/value/maximum", "/value/value")
    pointers = numeric + ("/layoutDirection", "/orientation", "/minimumPosition", "/value/schemaVersion")
    failures = tuple(("duplicate " + pointer, duplicate_member_source(baseline, pointer))
                     for pointer in pointers)
    failures += tuple(("nonfinite " + pointer, nonfinite_member_source(baseline, pointer))
                      for pointer in numeric)
    return check_backend(
        "slider layout", "schemas/slider-layout-case.schema.json",
        "schemas/slider-layout-request.schema.json", "schemas/slider-layout-ir.schema.json",
        "conformance/geometry/slider-layout-protocol-cases.json",
        extra_failures=failures, compare=layout_mismatch, compare_request=layout_mismatch,
    )


if __name__ == "__main__":
    raise SystemExit(main())
