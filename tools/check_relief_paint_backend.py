import json

from backend_source import duplicate_member_source, negative_underflow_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json
from check_surface_relief_backend import relief_mismatch


if __name__ == "__main__":
    baseline = load_json(ROOT / "conformance/ir/relief-paint-vectors.json")[1]["request"]
    raise SystemExit(check_backend(
        "opaque relief paint",
        "schemas/relief-paint-case.schema.json",
        "schemas/relief-paint-request.schema.json",
        "schemas/relief-paint-result.schema.json",
        "conformance/ir/relief-paint-vectors.json",
        compare=relief_mismatch,
        extra_failures=(
            ("positional request", json.dumps(list(baseline.values()))),
            ("duplicate edge width", duplicate_member_source(baseline, "/edgeWidth")),
            ("nonfinite edge width", nonfinite_member_source(baseline, "/edgeWidth")),
            ("nonfinite paint point", nonfinite_member_source(baseline, "/points/0/x")),
            ("negative nested radius", negative_underflow_member_source(baseline, "/outline/radii/topStart/x")),
        ),
    ))
