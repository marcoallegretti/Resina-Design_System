from backend_source import duplicate_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json


def main():
    baseline = load_json(ROOT / "conformance/geometry/toggle-layout-cases.json")[0]["request"]
    failures = tuple(
        ("duplicate " + path, duplicate_member_source(baseline, path))
        for path in ("/checked", "/insets/start", "/trackSize/width", "/thumbSize/height")
    )
    return check_backend(
        "toggle layout", "schemas/toggle-layout-case.schema.json",
        "schemas/toggle-layout-request.schema.json", "schemas/toggle-layout-ir.schema.json",
        "conformance/geometry/toggle-layout-cases.json", extra_failures=failures,
    )


if __name__ == "__main__":
    raise SystemExit(main())
