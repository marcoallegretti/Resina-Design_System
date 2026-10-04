import math
from fractions import Fraction

from backend_source import duplicate_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json


def slider_value_mismatch(actual, expected):
    for field in ("schemaVersion", "minimum", "maximum", "value"):
        if actual.get(field) != expected[field]:
            return "/" + field
    value = actual.get("progress")
    target = expected["progress"]
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        return "/progress"
    if target in (0, 1):
        return None if value == target else "/progress"
    if not 0 < value < 1 or abs(value - target) > 4 * math.ulp(target):
        return "/progress"
    return None


def main():
    cases = load_json(ROOT / "conformance/interaction/slider-value-cases.json")
    for case in cases:
        if "expected" in case:
            request = case["request"]
            minimum, maximum, value = (Fraction(float(request[key]))
                                      for key in ("minimum", "maximum", "value"))
            progress = float((value - minimum) / (maximum - minimum))
            expected = {**request, "progress": progress}
            if case["expected"] != expected:
                raise ValueError(f"{case['name']}: public result differs from rational oracle")
    baseline = cases[0]["request"]
    failures = tuple(("duplicate " + field, duplicate_member_source(baseline, "/" + field))
                     for field in ("minimum", "maximum", "value"))
    failures += (("nonfinite JSON", '{"schemaVersion":"0.1.0","minimum":0,"maximum":1e999,"value":0}'),)
    return check_backend(
        "slider value", "schemas/slider-value-case.schema.json",
        "schemas/slider-value-request.schema.json", "schemas/slider-value-ir.schema.json",
        "conformance/interaction/slider-value-cases.json", extra_failures=failures,
        compare=slider_value_mismatch,
    )


if __name__ == "__main__":
    raise SystemExit(main())
