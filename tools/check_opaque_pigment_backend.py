import copy
import json

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_schemas import ROOT, load_json, replace_at_pointer


def positional_profile_failures(baseline):
    profiles = baseline["profiles"]
    families = ("cast", "frost", "elastomer", "gel")
    mutations = (
        ("positional pigment document", "/profiles", [
            profiles["schemaVersion"], profiles["profiles"],
        ]),
        ("positional family pigments", "/profiles/profiles", [
            profiles["profiles"][family] for family in families
        ]),
        *((f"positional {family} pigment", f"/profiles/profiles/{family}", [
            profiles["profiles"][family]["sideShade"],
            profiles["profiles"][family]["highlightLift"],
        ]) for family in families),
    )
    failures = []
    for name, path, value in mutations:
        request = copy.deepcopy(baseline)
        replace_at_pointer(request, path, value)
        failures.append((name, json.dumps(request, allow_nan=False)))
    return tuple(failures)


if __name__ == "__main__":
    baseline = load_json(ROOT / "conformance/materials/opaque-pigment-vectors.json")[0]["request"]
    fields = (
        "schemaVersion",
        "materialFamily",
        "body",
        "profiles",
    )
    positional = [baseline[field] for field in fields]
    raise SystemExit(
        check_backend(
            "opaque pigment",
            "schemas/opaque-pigment-case.schema.json",
            "schemas/opaque-pigment-request.schema.json",
            "schemas/opaque-pigment-result.schema.json",
            "conformance/materials/opaque-pigment-vectors.json",
            extra_failures=positional_profile_failures(baseline) + (
                ("positional request", json.dumps(positional, allow_nan=False)),
                (
                    "unsupported positional request",
                    json.dumps(["9.9.9", *positional[1:]], allow_nan=False),
                ),
                (
                    "duplicate profile coefficient",
                    duplicate_member_source(baseline, "/profiles/profiles/cast/sideShade"),
                ),
                (
                    "nonfinite coefficient",
                    nonfinite_member_source(baseline, "/profiles/profiles/cast/sideShade"),
                ),
            ),
        )
    )
