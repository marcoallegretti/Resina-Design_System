import copy
import json

from backend_source import duplicate_member_source, negative_underflow_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_material_light_backend import compare as compare_light
from check_schemas import ROOT, load_json
from check_surface_relief_backend import relief_mismatch
from opaque_request_source import pigment_coefficient_failures
from material_relief_arithmetic import rectangular_samples


def compare(actual, expected):
    found = relief_mismatch(actual, expected)
    if found:
        return found
    paint, target = actual["paint"], expected["paint"]
    light = lambda value: {"schemaVersion": "0.1.0", "pigment": value["pigment"], **value["light"], "colorRanges": value["colorRanges"][:2], "samples": []}
    found = compare_light(light(paint), light(target))
    if found:
        return "/paint" + found
    edge = paint["edge"]["color"]
    if edge != target["edge"]["color"] or paint["colorRanges"][2] != {"lower": edge, "upper": edge}:
        return "/paint/exactEdge"
    for index, (sample, wanted) in enumerate(zip(actual["samples"], expected["samples"])):
        for name, color in (("body", target["pigment"]["body"]), ("edge", target["edge"]["color"])):
            if wanted["color"] == color and sample["color"] != color:
                return f"/samples/{index}/exact{name.title()}"
    return None


def source_cases():
    baseline = next(case for case in load_json(ROOT / "conformance/ir/material-relief-paint-vectors.json") if case["name"] == "cast-convex-opaque-face-active-light")
    request = copy.deepcopy(baseline["request"])
    failures = list(pigment_coefficient_failures(request, "/pigment/profiles"))
    successes = []
    for pointer in ("/light/ambient", "/light/interaction/strength"):
        for suppressed in (False, True):
            current = copy.deepcopy(request)
            current["enabled"] = not suppressed
            current["environment"]["accessibilityPreferences"]["highContrast"] = suppressed
            source = negative_underflow_member_source(current, pointer)
            failures.append((f"negative original {pointer} suppressed {suppressed}", source))
            failures.append((f"escaped negative {pointer} suppressed {suppressed}", source.replace('"ambient"', '"\\u0061mbient"').replace('"strength"', '"\\u0073trength"')))
            for literal in ("-0", "-0e-400", "1e-400"):
                valid = source.replace("-1e-400", literal, 1)
                parsed = json.loads(valid)
                light, samples = rectangular_samples(parsed)
                expected = copy.deepcopy(baseline["expected"])
                expected["paint"]["light"] = {name: light[name] for name in ("keyDirection", "ambient", "interaction")}
                expected["paint"]["colorRanges"] = light["colorRanges"] + expected["paint"]["colorRanges"][2:]
                expected["samples"] = samples
                successes.append((f"valid {literal} {pointer} suppressed {suppressed}", valid, expected))
    for pointer in ("/light/ambient", "/light/interaction/strength", "/environment/qualityPolicy", "/edgeWidth", "/points/0/x"):
        failures.append((f"duplicate {pointer}", duplicate_member_source(request, pointer)))
    for pointer in ("/light/ambient", "/light/interaction/strength", "/edgeWidth", "/points/0/x"):
        failures.append((f"overflow {pointer}", nonfinite_member_source(request, pointer)))
    failures.append(("negative nested outline radius", negative_underflow_member_source(request, "/outline/radii/topStart/x")))
    failures.append(("positional request", json.dumps(list(request.values()))))
    for name in request:
        changed = copy.deepcopy(request)
        changed.pop(name)
        failures.append((f"missing request {name}", json.dumps(changed)))
    for name in ("outline", "profile", "pigment", "light", "environment", "edge", "foreground"):
        changed = copy.deepcopy(request)
        changed[name] = list(changed[name].values())
        failures.append((f"positional {name}", json.dumps(changed)))
    return tuple(failures), tuple(successes)


if __name__ == "__main__":
    failures, successes = source_cases()
    raise SystemExit(check_backend(
        "material relief paint", "schemas/material-relief-paint-case.schema.json",
        "schemas/material-relief-paint-request.schema.json", "schemas/material-relief-paint-result.schema.json",
        "conformance/ir/material-relief-paint-vectors.json", compare=compare, extra_failures=failures, extra_successes=successes,
    ))
