import math

from check_color_guard_backend import check_backend


def contour_mismatch(actual, expected, path=""):
    if isinstance(expected, dict):
        if not isinstance(actual, dict) or actual.keys() != expected.keys():
            return path or "/"
        for name, value in expected.items():
            difference = contour_mismatch(actual[name], value, f"{path}/{name}")
            if difference:
                return difference
    elif isinstance(expected, list):
        if not isinstance(actual, list) or len(actual) != len(expected):
            return path or "/"
        for index, value in enumerate(expected):
            difference = contour_mismatch(actual[index], value, f"{path}/{index}")
            if difference:
                return difference
    elif isinstance(expected, (int, float)) and not isinstance(expected, bool):
        parent = path.rpartition("/")[0].rpartition("/")[2]
        radial = parent in ("start", "end")
        if (
            isinstance(actual, bool)
            or not isinstance(actual, (int, float))
            or not math.isfinite(actual)
            or not math.isclose(actual, expected, rel_tol=0 if radial else 1e-12, abs_tol=1e-12)
        ):
            return path
    elif type(actual) is not type(expected) or actual != expected:
        return path or "/"
    return None


if __name__ == "__main__":
    raise SystemExit(
        check_backend(
            "extruded contour",
            "schemas/extruded-contour-case.schema.json",
            "schemas/extruded-contour-request.schema.json",
            "schemas/extruded-contour-result.schema.json",
            "conformance/geometry/extruded-contour-vectors.json",
            extra_failures=(
                ("duplicate offset component", '{"offset":{"x":1,"x":2}}'),
                ("overflow offset component", '{"offset":{"x":1e400}}'),
            ),
            compare=contour_mismatch,
        )
    )
