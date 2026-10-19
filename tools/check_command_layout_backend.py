from backend_source import duplicate_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json


def main():
    baseline = next(
        case["request"] for case in load_json(ROOT / "conformance/layout/command-layout-cases.json")
        if "expected" in case
    )
    failures = tuple(
        ("duplicate " + path, duplicate_member_source(baseline, path))
        for path in (
            "/profile/component",
            "/profile/extents/standard",
            "/profile/extents/compact/maximumSize/height",
            "/tokens/command/extent/compact",
            "/spatialBinding/space.control.inset.inline/standard",
            "/spatialBinding/space.page/value",
            "/availableSize/width",
            "/environment/densityPreference",
        )
    )
    return check_backend(
        "command layout", "schemas/command-layout-case.schema.json",
        "schemas/command-layout-request.schema.json", "schemas/command-layout-ir.schema.json",
        "conformance/layout/command-layout-cases.json", extra_failures=failures,
    )


if __name__ == "__main__":
    raise SystemExit(main())
