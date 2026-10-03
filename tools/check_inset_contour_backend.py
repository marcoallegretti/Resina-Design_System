from check_color_guard_backend import check_backend


if __name__ == "__main__":
    raise SystemExit(
        check_backend(
            "inset contour",
            "schemas/inset-contour-case.schema.json",
            "schemas/inset-contour-request.schema.json",
            "schemas/inset-contour-result.schema.json",
            "conformance/geometry/inset-contour-vectors.json",
            extra_failures=(
                (
                    "duplicate corner radius",
                    '{"radii":{"topStart":{"x":1,"x":2}}}',
                ),
                (
                    "nonfinite inset",
                    '{"inset":1e400}',
                ),
            ),
        )
    )
