from check_color_guard_backend import check_backend


if __name__ == "__main__":
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
                    '{"profiles":{"profiles":{"cast":{"sideShade":0.1,"sideShade":0.2}}}}',
                ),
                (
                    "nonfinite coefficient",
                    '{"profiles":{"profiles":{"cast":{"sideShade":1e400}}}}',
                ),
            ),
        )
    )
