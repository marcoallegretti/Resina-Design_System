from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json


if __name__ == "__main__":
    baseline = load_json(ROOT / "conformance/materials/opaque-pigment-vectors.json")[0]["request"]
    raise SystemExit(
        check_backend(
            "opaque pigment",
            "schemas/opaque-pigment-case.schema.json",
            "schemas/opaque-pigment-request.schema.json",
            "schemas/opaque-pigment-result.schema.json",
            "conformance/materials/opaque-pigment-vectors.json",
            extra_failures=(
                (
                    "duplicate profile coefficient",
                    duplicate_member_source(baseline, "/profiles/profiles/cast/sideShade"),
                ),
                (
                    "nonfinite coefficient",
                    nonfinite_member_source(baseline, "/profiles/profiles/cast/sideShade"),
                ),
            ),
        )
    )
