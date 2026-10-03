import json
import math

from check_delta_backend import check_delta_backend
from check_headless_backend import mismatch
from check_schemas import ROOT, load_json


def spring_mismatch(actual, expected):
    if not isinstance(actual, dict):
        return "/"
    for field in ("position", "velocity"):
        value = actual.get(field)
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            return "/" + field
        tolerance = max(1e-9, 1e-9 * abs(expected[field]))
        if not math.isfinite(value) or abs(value - expected[field]) > tolerance:
            return "/" + field
    return mismatch(
        {**actual, "position": expected["position"], "velocity": expected["velocity"]}, expected,
    )


if __name__ == "__main__":
    baseline = json.dumps(load_json(ROOT / "conformance/motion/spring-request.json"))
    raise SystemExit(check_delta_backend(
        "spring", "schemas/spring-case.schema.json",
        "schemas/spring-request.schema.json", "schemas/spring-result.schema.json",
        "conformance/motion/spring-request.json", "conformance/motion/spring-expected.json",
        "conformance/motion/spring-cases.json",
        extra_failures=(
            ("duplicate root version", baseline.replace(
                '"schemaVersion": "0.1.0"',
                '"schemaVersion": "0.1.0", "schemaVersion": "0.1.0"', 1,
            )),
            ("duplicate spring coefficient", baseline.replace(
                '"mass": 1', '"mass": 1, "mass": 2', 1,
            )),
            ("nonfinite time", baseline.replace('"time": 1', '"time": 1e400', 1)),
            ("unknown root field", '{"unknown": 0, ' + baseline[1:]),
            ("missing reduced motion", baseline.replace(', "reducedMotion": false', '', 1)),
        ),
        compare=spring_mismatch,
    ))
