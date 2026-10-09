import json

from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json


if __name__ == "__main__":
    baseline = load_json(ROOT / "conformance/materials/frost-legibility-vectors.json")[0]["request"]
    fields = (
        "schemaVersion",
        "representation",
        "portableBody",
        "opaqueBody",
        "foreground",
        "postTreatmentBackdrop",
        "minimumContrast",
    )
    positional = [baseline[field] for field in fields]
    raise SystemExit(
        check_backend(
            "Frost legibility",
            "schemas/frost-legibility-case.schema.json",
            "schemas/frost-legibility-request.schema.json",
            "schemas/frost-legibility-result.schema.json",
            "conformance/materials/frost-legibility-vectors.json",
            extra_failures=(
                ("positional request", json.dumps(positional, allow_nan=False)),
                (
                    "unsupported positional request",
                    json.dumps(["9.9.9", *positional[1:]], allow_nan=False),
                ),
            ),
        )
    )
