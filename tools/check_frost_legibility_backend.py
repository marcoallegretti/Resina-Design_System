from check_color_guard_backend import check_backend


if __name__ == "__main__":
    raise SystemExit(
        check_backend(
            "Frost legibility",
            "schemas/frost-legibility-case.schema.json",
            "schemas/frost-legibility-request.schema.json",
            "schemas/frost-legibility-result.schema.json",
            "conformance/materials/frost-legibility-vectors.json",
        )
    )
