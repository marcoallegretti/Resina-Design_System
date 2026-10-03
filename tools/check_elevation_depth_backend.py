from backend_source import duplicate_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json


def nested_duplicate_source():
    request = load_json(ROOT / "conformance/elevation/backend-cases.json")[0]["request"]
    return duplicate_member_source(request, "/tokens/depth/zero/$value")


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
