import json
import math

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json


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
    baseline = load_json(ROOT / "conformance/geometry/extruded-contour-vectors.json")[0]["request"]
    fields = ("schemaVersion", "size", "radii", "layoutDirection", "offset")
    positional = [baseline[field] for field in fields]
    raise SystemExit(
        check_backend(
            "extruded contour",
            "schemas/extruded-contour-case.schema.json",
            "schemas/extruded-contour-request.schema.json",
            "schemas/extruded-contour-result.schema.json",
            "conformance/geometry/extruded-contour-vectors.json",
            extra_failures=(
                ("positional request", json.dumps(positional, allow_nan=False)),
                (
                    "unsupported positional request",
                    json.dumps(["9.9.9", *positional[1:]], allow_nan=False),
                ),
                ("duplicate offset component", duplicate_member_source(baseline, "/offset/x")),
                ("overflow offset component", nonfinite_member_source(baseline, "/offset/x")),
            ),
            compare=contour_mismatch,
        )
    )
