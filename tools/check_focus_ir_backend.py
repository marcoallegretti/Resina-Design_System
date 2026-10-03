from backend_source import duplicate_member_source, nonfinite_member_source
from check_delta_backend import check_delta_backend
from check_extruded_contour_backend import contour_mismatch
from check_headless_backend import mismatch
from check_schemas import ROOT, load_json


def focus_ir_mismatch(actual, expected):
    if not isinstance(actual, dict):
        return "/"
    difference = contour_mismatch(actual.get("geometry"), expected["geometry"])
    if difference:
        return "/geometry" + (difference if difference != "/" else "")
    return mismatch({**actual, "geometry": expected["geometry"]}, expected)


if __name__ == "__main__":
    baseline = load_json(ROOT / "conformance/ir/focus-ir-request.json")
    raise SystemExit(check_delta_backend(
        "focus IR", "schemas/focus-ir-case.schema.json",
        "schemas/focus-ir-request.schema.json", "schemas/focus-indicator-ir.schema.json",
        "conformance/ir/focus-ir-request.json", "conformance/ir/focus-ir-expected.json",
        "conformance/ir/focus-ir-cases.json",
        extra_failures=(
            ("duplicate root version", duplicate_member_source(baseline, "/schemaVersion")),
            ("duplicate light component", duplicate_member_source(baseline, "/keyLight/direction/x")),
            ("nonfinite size", nonfinite_member_source(baseline, "/size/width")),
        ),
        compare=focus_ir_mismatch,
    ))
