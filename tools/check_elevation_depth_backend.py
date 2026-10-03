import json

from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json


def nested_duplicate_source():
    request = load_json(ROOT / "conformance/elevation/backend-cases.json")[0]["request"]
    source = json.dumps(request, separators=(",", ":"), ensure_ascii=False)
    original = '"zero":{"$value":{"value":0,"unit":"px"}}'
    duplicated = '"zero":{"$value":{"value":0,"unit":"px"},"$value":{"value":1,"unit":"px"}}'
    if source.count(original) != 1:
        raise ValueError("valid elevation fixture lacks one zero depth token")
    return source.replace(original, duplicated, 1)


if __name__ == "__main__":
    raise SystemExit(
        check_backend(
            "elevation depth",
            "schemas/elevation-depth-case.schema.json",
            "schemas/elevation-depth-request.schema.json",
            "schemas/elevation-depth-result.schema.json",
            "conformance/elevation/backend-cases.json",
            extra_failures=((
                "duplicate nested token member",
                nested_duplicate_source(),
            ),),
        )
    )
