import json

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json


if __name__ == "__main__":
    baseline = load_json(ROOT / "conformance/geometry/inset-contour-vectors.json")[0]["request"]
    fields = ("schemaVersion", "size", "radii", "inset")
    positional = [baseline[field] for field in fields]
    raise SystemExit(
        check_backend(
            "inset contour",
            "schemas/inset-contour-case.schema.json",
            "schemas/inset-contour-request.schema.json",
            "schemas/inset-contour-result.schema.json",
            "conformance/geometry/inset-contour-vectors.json",
            extra_failures=(
                ("positional request", json.dumps(positional, allow_nan=False)),
                (
                    "unsupported positional request",
                    json.dumps(["9.9.9", *positional[1:]], allow_nan=False),
                ),
                (
                    "duplicate corner radius",
                    duplicate_member_source(baseline, "/radii/topStart/x"),
                ),
                (
                    "nonfinite inset",
                    nonfinite_member_source(baseline, "/inset"),
                ),
            ),
        )
    )
