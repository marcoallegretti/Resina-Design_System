import json

from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json


if __name__ == "__main__":
    baseline = load_json(ROOT / "conformance/color/edge-contrast-vectors.json")[0]["request"]
    fields = (
        "schemaVersion",
        "outline",
        "outlineStrong",
        "adjacentColor",
        "minimumContrast",
    )
    positional = [baseline[field] for field in fields]
    raise SystemExit(
        check_backend(
            "edge contrast",
            "schemas/edge-contrast-case.schema.json",
            "schemas/edge-contrast-request.schema.json",
            "schemas/edge-contrast-result.schema.json",
            "conformance/color/edge-contrast-vectors.json",
            extra_failures=(
                ("positional request", json.dumps(positional, allow_nan=False)),
                (
                    "unsupported positional request",
                    json.dumps(["9.9.9", *positional[1:]], allow_nan=False),
                ),
            ),
        )
    )
