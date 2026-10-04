from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_headless_backend import mismatch
from check_schemas import ROOT, load_json
from check_spring_backend import spring_mismatch


def trajectory_mismatch(actual, expected):
    if not isinstance(actual, dict):
        return "/"
    if actual.get("target") != expected["target"]:
        return "/target"
    difference = spring_mismatch(actual.get("state"), expected["state"])
    if difference:
        return "/state" + difference
    if actual.get("settled") is True and actual["state"]["position"] != actual.get("target"):
        return "/state/position"
    return mismatch({**actual, "state": expected["state"]}, expected)


if __name__ == "__main__":
    baseline = load_json(ROOT / "conformance/motion/spring-trajectory-request.json")
    raise SystemExit(check_backend(
        "spring trajectory", "schemas/spring-trajectory-case.schema.json",
        "schemas/spring-trajectory-request.schema.json", "schemas/spring-trajectory-result.schema.json",
        "conformance/motion/spring-trajectory-cases.json",
        extra_failures=(
            ("duplicate state position", duplicate_member_source(baseline, "/initial/position")),
            ("duplicate dynamics version", duplicate_member_source(baseline, "/dynamics/schemaVersion")),
            ("nonfinite target", nonfinite_member_source(baseline, "/target")),
        ), compare=trajectory_mismatch,
    ))
