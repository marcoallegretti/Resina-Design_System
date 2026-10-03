from backend_source import duplicate_member_source, nonfinite_member_source
from check_delta_backend import check_delta_backend
from check_extruded_contour_backend import contour_mismatch
from check_headless_backend import mismatch
from check_key_light_backend import key_light_mismatch
from check_schemas import ROOT, load_json


def opaque_surface_mismatch(actual, expected):
    if not isinstance(actual, dict):
        return "/"
    for name, compare in (("geometry", contour_mismatch), ("lighting", key_light_mismatch)):
        difference = compare(actual.get(name), expected[name])
        if difference:
            return f"/{name}" + (difference if difference != "/" else "")
    return mismatch(
        {**actual, "geometry": expected["geometry"], "lighting": expected["lighting"]},
        expected,
    )


if __name__ == "__main__":
    baseline = load_json(ROOT / "conformance/ir/opaque-surface-request.json")
    raise SystemExit(check_delta_backend(
        "opaque surface", "schemas/opaque-surface-case.schema.json",
        "schemas/opaque-surface-request.schema.json", "schemas/opaque-surface-ir.schema.json",
        "conformance/ir/opaque-surface-request.json", "conformance/ir/opaque-surface-expected.json",
        "conformance/ir/opaque-surface-cases.json",
        extra_failures=(
            ("duplicate root version", duplicate_member_source(baseline, "/schemaVersion")),
            ("duplicate nested band", duplicate_member_source(baseline, "/appearance/bands/cast/edgeWidth")),
            ("nonfinite surface dimension", nonfinite_member_source(baseline, "/size/width")),
        ),
        compare=opaque_surface_mismatch,
    ))
