from check_color_guard_backend import check_backend


if __name__ == "__main__":
    raise SystemExit(
        check_backend(
            "edge contrast",
            "schemas/edge-contrast-case.schema.json",
            "schemas/edge-contrast-request.schema.json",
            "schemas/edge-contrast-result.schema.json",
            "conformance/color/edge-contrast-vectors.json",
        )
    )
