import json

from backend_source import duplicate_member_source, negative_underflow_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json
from check_surface_outline_backend import outline_mismatch


def relief_mismatch(actual, expected, pointer=""):
    if isinstance(expected, dict) and isinstance(actual, dict) and set(actual) == set(expected):
        for key, value in expected.items():
            found = relief_mismatch(actual[key], value, f"{pointer}/{key}")
            if found:
                return found
        return None
    if isinstance(expected, list) and isinstance(actual, list) and len(actual) == len(expected):
        for index, (a, e) in enumerate(zip(actual, expected)):
            found = relief_mismatch(a, e, f"{pointer}/{index}")
            if found:
                return found
        return None
    return outline_mismatch(actual, expected, pointer)


if __name__ == "__main__":
    baseline = load_json(ROOT / "conformance/geometry/surface-relief-vectors.json")[1]["request"]
    raise SystemExit(check_backend(
        "surface relief",
        "schemas/surface-relief-case.schema.json",
        "schemas/surface-relief-request.schema.json",
        "schemas/surface-relief-result.schema.json",
        "conformance/geometry/surface-relief-vectors.json",
        compare=relief_mismatch,
        extra_failures=(
            ("positional request", json.dumps(list(baseline.values()))),
            ("positional profile", json.dumps(dict(baseline, profile=list(baseline["profile"].values())))),
            ("duplicate amplitude", duplicate_member_source(baseline, "/profile/amplitude")),
            ("nonfinite amplitude", nonfinite_member_source(baseline, "/profile/amplitude")),
            ("nonfinite sample point", nonfinite_member_source(baseline, "/points/0/x")),
            ("negative nested radius", negative_underflow_member_source(baseline, "/outline/radii/topStart/x")),
        ),
    ))
