from backend_source import duplicate_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json


def main():
    baseline = load_json(ROOT / "conformance/interaction/toggle-states-cases.json")[0]["request"]
    failures = tuple(
        ("duplicate " + path, duplicate_member_source(baseline, path))
        for path in ("/checked", "/hovered", "/activation/enabled", "/activation/hold")
    )
    return check_backend(
        "toggle states", "schemas/toggle-states-case.schema.json",
        "schemas/toggle-states-request.schema.json", "schemas/state-set.schema.json",
        "conformance/interaction/toggle-states-cases.json", extra_failures=failures,
    )


if __name__ == "__main__":
    raise SystemExit(main())
