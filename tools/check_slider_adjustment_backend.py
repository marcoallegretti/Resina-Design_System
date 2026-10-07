from fractions import Fraction
import json

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json
from check_slider_value_backend import slider_value_mismatch


def adjustment_expected(request):
    current = request["current"]
    minimum, maximum, value = (Fraction(float(current[field]))
                               for field in ("minimum", "maximum", "value"))
    accepted = request["enabled"] and not request["readOnly"]
    intent = request["adjustment"]
    kind = intent["kind"]
    target = value
    if accepted:
        if kind == "setValue":
            target = Fraction(float(intent["value"]))
        elif kind in ("minimum", "maximum"):
            target = minimum if kind == "minimum" else maximum
        else:
            amount = Fraction(float(intent["amount"]))
            target += amount if kind == "increase" else -amount
            target = min(maximum, max(minimum, target))
    target = float(target)
    progress = float((Fraction(target) - minimum) / (maximum - minimum))
    return {
        "schemaVersion": "0.1.0",
        "value": {**current, "value": target, "progress": progress},
        "accepted": accepted,
        "changed": target != current["value"],
    }


def adjustment_mismatch(actual, expected):
    for field in ("schemaVersion", "accepted", "changed"):
        if actual.get(field) != expected[field]:
            return "/" + field
    different = slider_value_mismatch(actual["value"], expected["value"])
    return "/value" + different if different else None


def main():
    cases = load_json(ROOT / "conformance/interaction/slider-adjustment-protocol-cases.json")
    for case in cases:
        if "expected" in case and case["expected"] != adjustment_expected(case["request"]):
            raise ValueError(f"{case['name']}: public result differs from rational oracle")
    baseline = next(case["request"] for case in cases if "expected" in case)
    pointers = ("/enabled", "/readOnly", "/current", "/current/schemaVersion",
                "/current/minimum", "/current/maximum", "/current/value",
                "/adjustment", "/adjustment/kind", "/adjustment/value")
    failures = tuple(("duplicate " + pointer, duplicate_member_source(baseline, pointer))
                     for pointer in pointers)
    failures += tuple(("nonfinite " + pointer, nonfinite_member_source(baseline, pointer))
                      for pointer in ("/current/minimum", "/current/maximum",
                                      "/current/value", "/adjustment/value"))
    amount_request = {**baseline, "adjustment": {"kind": "increase", "amount": 1}}
    failures += (("duplicate amount", duplicate_member_source(amount_request, "/adjustment/amount")),
                 ("nonfinite amount", nonfinite_member_source(amount_request, "/adjustment/amount")))
    successes = tuple(
        ("escaped " + field, json.dumps(request).replace('"' + field + '"', '"' + escaped + '"'),
         adjustment_expected(request))
        for request, field, escaped in (
            (baseline, "value", "\\u0076alue"),
            (baseline, "kind", "\\u006bind"),
            (amount_request, "amount", "\\u0061mount"),
        )
    )
    return check_backend(
        "slider adjustment", "schemas/slider-adjustment-case.schema.json",
        "schemas/slider-adjustment-request.schema.json", "schemas/slider-adjustment-ir.schema.json",
        "conformance/interaction/slider-adjustment-protocol-cases.json",
        extra_failures=failures, compare=adjustment_mismatch, extra_successes=successes,
    )


if __name__ == "__main__":
    raise SystemExit(main())
