import json
import math

from backend_source import duplicate_member_source, negative_underflow_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json
from check_headless_backend import mismatch


def outline_mismatch(actual, expected, pointer=""):
    if isinstance(expected, dict) and isinstance(actual, dict) and set(actual) == set(expected):
        for key, value in expected.items():
            found = outline_mismatch(actual[key], value, f"{pointer}/{key}")
            if found:
                return found
        return None
    if type(expected) in (int, float) and type(actual) in (int, float):
        return None if math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12) else pointer or "/"
    return mismatch(actual, expected, pointer)


if __name__ == "__main__":
    baseline = load_json(ROOT / "conformance/geometry/surface-outline-vectors.json")[0]["request"]
    positional_size = dict(baseline, size=[120, 60])
    positional_corner = json.loads(json.dumps(baseline))
    positional_corner["radii"]["topStart"] = [20, 12]
    raise SystemExit(check_backend(
        "surface outline",
        "schemas/surface-outline-case.schema.json",
        "schemas/surface-outline-request.schema.json",
        "schemas/surface-outline-result.schema.json",
        "conformance/geometry/surface-outline-vectors.json",
        compare=outline_mismatch,
        extra_failures=(
            ("positional request", json.dumps(list(baseline.values()))),
            ("positional size", json.dumps(positional_size)),
            ("positional corner", json.dumps(positional_corner)),
            ("duplicate radius", duplicate_member_source(baseline, "/radii/topStart/x")),
            ("nonfinite inset", nonfinite_member_source(baseline, "/minimumInset")),
            ("negative underflowed radius", negative_underflow_member_source(baseline, "/radii/topStart/x")),
            ("negative underflowed inset", negative_underflow_member_source(baseline, "/minimumInset")),
        ),
    ))
