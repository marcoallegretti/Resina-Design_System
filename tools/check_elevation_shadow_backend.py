import copy
import json

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_extruded_contour_backend import contour_mismatch
from check_schemas import ROOT, load_json, replace_at_pointer
from elevation_shadow_arithmetic import (
    ROLES, canonical_contour, expected_result, hull_violation, physical_radii, shadow_mismatch,
)

VECTORS = "conformance/elevation/shadow-vectors.json"
BASELINE = "light raised depthScale 1 on surface.base"


def baseline():
    return next(case["request"] for case in load_json(ROOT / VECTORS) if case["name"] == BASELINE)


def semantic_mismatch(actual, request):
    """Re-derive every result member from the request, independently of the reference."""
    expected = expected_result(request)
    if expected is None:
        return "/guard"
    trimmed = copy.deepcopy(actual)
    hull = trimmed["shadow"].pop("hull", None) if isinstance(trimmed.get("shadow"), dict) else None
    found = shadow_mismatch(trimmed, expected)
    if found:
        return found
    if hull is not None:
        corners = physical_radii(request["size"], request["radii"], request["environment"]["layoutDirection"])
        canonical = canonical_contour(request["size"], corners, expected["shadow"]["sweep"])
        found = contour_mismatch(hull, canonical, "/shadow/hull") or hull_violation(hull, request, actual["shadow"]["sweep"])
        if found:
            return found if found.startswith("/shadow/hull") else "/shadow" + found
    return None


def compare_vector(actual, expected, request):
    return shadow_mismatch(actual, expected) or semantic_mismatch(actual, request)


def source_cases():
    request = baseline()
    failures, successes = [], []
    for pointer in ("/schemaVersion", "/profiles/roles/raised/shadowShade", "/profiles/roles/raised/contactMargin",
                    "/edge/minimumContrast", "/focus/color", "/keyLight/direction/x", "/elevationDepth"):
        failures.append((f"duplicate {pointer}", duplicate_member_source(request, pointer)))
    for pointer in ("/elevationDepth", "/depthScale", "/profiles/roles/raised/shadowShade",
                    "/profiles/roles/modal/contactMargin", "/size/width",
                    "/edge/minimumContrast", "/focus/minimumContrast", "/keyLight/direction/y"):
        failures.append((f"overflow {pointer}", nonfinite_member_source(request, pointer)))
    for depth, scale, label in ((1e308, 2, "final depth"), (1e308, 1, "sweep")):
        changed = copy.deepcopy(request)
        changed["elevationDepth"], changed["depthScale"] = depth, scale
        failures.append((f"{label} beyond finite range", json.dumps(changed)))
    for pointer in ("", "/profiles", "/profiles/roles", "/profiles/roles/raised", "/edge", "/focus", "/size", "/radii",
                    "/keyLight", "/adjacentColor", "/environment"):
        value = request
        for segment in pointer.strip("/").split("/") if pointer else ():
            value = value[segment]
        for member in value:
            changed = copy.deepcopy(request)
            parent = changed
            for segment in pointer.strip("/").split("/") if pointer else ():
                parent = parent[segment]
            parent.pop(member)
            failures.append((f"missing {pointer}/{member}", json.dumps(changed)))
        changed = copy.deepcopy(request)
        positional = list(value.values())
        if pointer:
            replace_at_pointer(changed, pointer, positional)
        else:
            changed = positional
        failures.append((f"positional {pointer or 'request'}", json.dumps(changed)))
    for role in ROLES:
        for member, value in (("shadowShade", 1), ("shadowShade", -0.1), ("contactMargin", -1)):
            changed = copy.deepcopy(request)
            changed["profiles"]["roles"][role][member] = value
            failures.append((f"{role} {member} {value}", json.dumps(changed)))
    shape = {"topStart": {"x": 18, "y": 18}, "topEnd": {"x": 3, "y": 3},
             "bottomEnd": {"x": 14, "y": 5}, "bottomStart": {"x": 0, "y": 0}}
    for layout in ("ltr", "rtl"):
        for x, y in ((-0.34, -0.94), (-1, -1), (0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-3e-300, 5e-300)):
            for scale in (0.5, 2):
                changed = copy.deepcopy(request)
                changed["environment"]["layoutDirection"] = layout
                changed["keyLight"]["direction"] = {"x": x, "y": y}
                changed["radii"] = copy.deepcopy(shape)
                changed["depthScale"] = scale
                changed["distanceSamples"] = [-2, -1e-9, 0, 1e-9, 0.25, 1, 4]
                changed["environment"]["rendererCapabilities"]["advancedShadow"] = scale == 2
                successes.append((f"{layout} key ({x}, {y}) depthScale {scale}", json.dumps(changed), changed))
    for shade, margin in ((0, 0), (0.5, 3), (0.999, 0.25)):
        changed = copy.deepcopy(request)
        changed["profiles"]["roles"]["raised"] = {"shadowShade": shade, "contactMargin": margin}
        changed["environment"]["rendererCapabilities"]["advancedShadow"] = True
        changed["edge"]["minimumContrast"] = 1
        changed["focus"]["minimumContrast"] = 1
        successes.append((f"profile shade {shade} margin {margin}", json.dumps(changed), changed))
    for layout in ("ltr", "rtl"):
        changed = copy.deepcopy(request)
        changed["environment"]["layoutDirection"] = layout
        changed["radii"] = copy.deepcopy(shape)
        changed["focus"]["minimumContrast"] = 4.8
        successes.append((f"{layout} focus falls back to outline.strong", json.dumps(changed), changed))
    for pointer in ("edge", "focus"):
        changed = copy.deepcopy(request)
        changed[pointer]["minimumContrast"] = 5.5
        failures.append((f"{pointer} guard fails only beside the band", json.dumps(changed)))
    return tuple(failures), tuple(successes)


if __name__ == "__main__":
    failures, successes = source_cases()
    raise SystemExit(check_backend(
        "elevation shadow", "schemas/elevation-shadow-case.schema.json",
        "schemas/elevation-shadow-request.schema.json", "schemas/elevation-shadow-result.schema.json",
        VECTORS, extra_failures=failures, extra_successes=successes,
        compare=semantic_mismatch, compare_request=compare_vector,
    ))
