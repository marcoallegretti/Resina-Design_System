from fractions import Fraction
import json

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json
from check_slider_adjustment_backend import adjustment_mismatch
from check_slider_layout_backend import layout_expected


def position_expected(request):
    layout = layout_expected(request["layout"])
    axis = "x" if layout["orientation"] == "horizontal" else "y"
    minimum_origin, maximum_origin, current_origin = (
        Fraction(layout[field][axis]) for field in
        ("minimumThumbBounds", "maximumThumbBounds", "thumbBounds"))
    desired = Fraction(float(request["desiredOrigin"]))
    current = request["layout"]["value"]
    minimum, maximum, value = (Fraction(float(current[field]))
                               for field in ("minimum", "maximum", "value"))
    accepted = request["enabled"] and not request["readOnly"]
    target = value
    if accepted and desired != current_origin:
        progress = min(Fraction(1), max(Fraction(0),
                       (desired - minimum_origin) / (maximum_origin - minimum_origin)))
        target = minimum + (maximum - minimum) * progress
    target = float(target)
    progress = float((Fraction(target) - minimum) / (maximum - minimum))
    return {
        "schemaVersion": "0.1.0",
        "value": {**current, "value": target, "progress": progress},
        "accepted": accepted,
        "changed": target != current["value"],
    }


def main():
    cases = load_json(ROOT / "conformance/interaction/slider-position-protocol-cases.json")
    for case in cases:
        if "expected" in case and case["expected"] != position_expected(case["request"]):
            raise ValueError(f"{case['name']}: public position differs from rational oracle")
    baseline = next(case["request"] for case in cases if "expected" in case)
    numeric = ("/desiredOrigin", "/layout/allocationSize/width", "/layout/allocationSize/height",
               "/layout/thumbSize/width", "/layout/thumbSize/height", "/layout/trackThickness",
               "/layout/insets/start", "/layout/insets/end", "/layout/insets/top",
               "/layout/insets/bottom", "/layout/value/minimum", "/layout/value/maximum",
               "/layout/value/value")
    pointers = numeric + ("/enabled", "/readOnly", "/layout", "/layout/schemaVersion",
                          "/layout/orientation", "/layout/layoutDirection",
                          "/layout/minimumPosition", "/layout/value/schemaVersion")
    failures = tuple(("duplicate " + pointer, duplicate_member_source(baseline, pointer))
                     for pointer in pointers)
    failures += tuple(("nonfinite " + pointer, nonfinite_member_source(baseline, pointer))
                      for pointer in numeric)
    successes = tuple(("escaped " + field,
                       json.dumps(baseline).replace('"' + field + '"', '"' + escaped + '"'),
                       position_expected(baseline))
                      for field, escaped in (("layout", "\\u006cayout"),
                                             ("desiredOrigin", "\\u0064esiredOrigin"),
                                             ("value", "\\u0076alue")))
    return check_backend(
        "slider position", "schemas/slider-position-case.schema.json",
        "schemas/slider-position-request.schema.json", "schemas/slider-adjustment-ir.schema.json",
        "conformance/interaction/slider-position-protocol-cases.json",
        extra_failures=failures, extra_successes=successes, compare=adjustment_mismatch,
    )


if __name__ == "__main__":
    raise SystemExit(main())
