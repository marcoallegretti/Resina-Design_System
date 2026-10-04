from backend_source import duplicate_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json


def main():
    baseline = load_json(ROOT / "conformance/interaction/toggle-activation-cases.json")[0]["request"]
    failures = tuple(
        ("duplicate " + path, duplicate_member_source(baseline, path))
        for path in ("/checked", "/state/enabled", "/event/inside")
    )
    return check_backend(
        "toggle activation", "schemas/toggle-activation-case.schema.json",
        "schemas/toggle-activation-request.schema.json", "schemas/toggle-activation-result.schema.json",
        "conformance/interaction/toggle-activation-cases.json", extra_failures=failures,
    )


if __name__ == "__main__":
    raise SystemExit(main())
