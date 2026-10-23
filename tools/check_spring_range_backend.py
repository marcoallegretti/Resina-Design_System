import json

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json
from check_spring_trajectory_backend import trajectory_mismatch

if __name__ == "__main__":
    baseline = load_json(ROOT / "conformance/motion/spring-trajectory-request.json")
    raise SystemExit(check_backend(
        "spring range", "schemas/spring-trajectory-case.schema.json",
        "schemas/spring-trajectory-request.schema.json", "schemas/spring-trajectory-result.schema.json",
        "conformance/motion/spring-range-cases.json",
        extra_failures=(
            ("duplicate state position", duplicate_member_source(baseline, "/initial/position")),
            ("nonfinite target", nonfinite_member_source(baseline, "/target")),
        ), compare=trajectory_mismatch,
    ))
