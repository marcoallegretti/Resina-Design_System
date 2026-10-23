from backend_source import duplicate_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json


def main():
    baseline = next(
        case["request"] for case in load_json(ROOT / "conformance/layout/toggle-profile-resolution-cases.json")
        if "expected" in case
    )
    failures = tuple(
        ("duplicate " + path, duplicate_member_source(baseline, path))
        for path in (
            "/profile/component",
            "/profile/parts/standard",
            "/profile/parts/compact/trackSize/height",
            "/tokens/toggle/track/compact",
            "/availableSize/width",
            "/environment/densityPreference",
        )
    )
    return check_backend(
        "toggle profile", "schemas/toggle-profile-case.schema.json",
        "schemas/toggle-profile-request.schema.json", "schemas/toggle-profile-ir.schema.json",
        "conformance/layout/toggle-profile-resolution-cases.json", extra_failures=failures,
    )


if __name__ == "__main__":
    raise SystemExit(main())
