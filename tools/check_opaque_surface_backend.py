from check_delta_backend import check_delta_backend
from check_extruded_contour_backend import contour_mismatch
from check_headless_backend import mismatch
from check_key_light_backend import key_light_mismatch


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
    raise SystemExit(check_delta_backend(
        "opaque surface", "schemas/opaque-surface-case.schema.json",
        "schemas/opaque-surface-request.schema.json", "schemas/opaque-surface-ir.schema.json",
        "conformance/ir/opaque-surface-request.json", "conformance/ir/opaque-surface-expected.json",
        "conformance/ir/opaque-surface-cases.json",
        extra_failures=(
            ("duplicate root version", '{"schemaVersion":"0.1.0","schemaVersion":"0.1.0"}'),
            ("duplicate nested band", '{"appearance":{"bands":{"cast":{"edgeWidth":1,"edgeWidth":2}}}}'),
            ("nonfinite surface dimension", '{"size":{"width":1e400}}'),
        ),
        compare=opaque_surface_mismatch,
    ))
