import math

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_headless_backend import mismatch
from check_schemas import ROOT, load_json


def key_light_mismatch(actual, expected):
    if not isinstance(actual, dict):
        return "/"
    offset = actual.get("sideOffset")
    expected_offset = expected["sideOffset"]
    if not isinstance(offset, dict) or offset.keys() != expected_offset.keys():
        return "/sideOffset"
    for axis, value in expected_offset.items():
        component = offset[axis]
        if (
            isinstance(component, bool)
            or not isinstance(component, (int, float))
            or not math.isfinite(component)
            or not math.isclose(component, value, rel_tol=1e-12, abs_tol=1e-12)
        ):
            return f"/sideOffset/{axis}"
    return mismatch({**actual, "sideOffset": expected_offset}, expected)


if __name__ == "__main__":
    baseline = load_json(ROOT / "conformance/lighting/key-light-vectors.json")[0]["request"]
    raise SystemExit(
        check_backend(
            "key light",
            "schemas/key-light-case.schema.json",
            "schemas/key-light-request.schema.json",
            "schemas/key-light-result.schema.json",
            "conformance/lighting/key-light-vectors.json",
            extra_failures=(
                ("duplicate direction component", duplicate_member_source(baseline, "/keyLight/direction/x")),
                ("overflow direction component", nonfinite_member_source(baseline, "/keyLight/direction/x")),
            ),
            compare=key_light_mismatch,
        )
    )
