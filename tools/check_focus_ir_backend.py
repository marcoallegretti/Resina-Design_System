from check_delta_backend import check_delta_backend
from check_extruded_contour_backend import contour_mismatch
from check_headless_backend import mismatch


def focus_ir_mismatch(actual, expected):
    if not isinstance(actual, dict):
        return "/"
    difference = contour_mismatch(actual.get("geometry"), expected["geometry"])
    if difference:
        return "/geometry" + (difference if difference != "/" else "")
    return mismatch({**actual, "geometry": expected["geometry"]}, expected)


if __name__ == "__main__":
    raise SystemExit(check_delta_backend(
        "focus IR", "schemas/focus-ir-case.schema.json",
        "schemas/focus-ir-request.schema.json", "schemas/focus-indicator-ir.schema.json",
        "conformance/ir/focus-ir-request.json", "conformance/ir/focus-ir-expected.json",
        "conformance/ir/focus-ir-cases.json",
        extra_failures=(
            ("duplicate root version", '{"schemaVersion":"0.1.0","schemaVersion":"0.1.0"}'),
            ("duplicate light component", '{"keyLight":{"direction":{"x":1,"x":2}}}'),
            ("nonfinite size", '{"size":{"width":1e400}}'),
        ),
        compare=focus_ir_mismatch,
    ))
