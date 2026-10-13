import copy
import json
import math

from backend_source import duplicate_member_source, negative_underflow_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_headless_backend import mismatch
from check_schemas import ROOT, load_json, replace_at_pointer
from material_light_arithmetic import expected_result
from opaque_request_source import pigment_coefficient_failures


def baseline():
    return load_json(ROOT / "conformance/lighting/material-light-vectors.json")[0]["request"]


def compare(actual, expected):
    found = mismatch(actual, expected)
    if found:
        return found
    if actual["pigment"]["body"] != expected["pigment"]["body"]:
        return "/pigment/body"
    body = actual["pigment"]["body"]["components"]
    for index, (sample, target) in enumerate(zip(actual["samples"], expected["samples"])):
        if target["weight"] == 0 and (sample["weight"] != 0 or sample["color"] != actual["pigment"]["body"]):
            return f"/samples/{index}/exactBody"
    for index, name in enumerate(("highlight", "side")):
        endpoint = actual["pigment"][name]["components"]
        bounds = actual["colorRanges"][index]
        for channel, (b, e) in enumerate(zip(body, endpoint)):
            lower = max(0, math.nextafter(min(b, e) - 4 * math.ulp(1.0), -math.inf))
            upper = min(1, math.nextafter(max(b, e) + 4 * math.ulp(1.0), math.inf))
            if bounds["lower"]["components"][channel] > lower or bounds["upper"]["components"][channel] < upper:
                return f"/colorRanges/{index}/{channel}"
    return None


def source_cases():
    request = baseline()
    request["light"]["ambient"] = 0.5
    request["light"]["interaction"] = {"direction": {"x": -1, "y": 0}, "strength": 0.25}
    failures, successes = [], []
    for pointer in ("/light/ambient", "/light/interaction/strength"):
        for suppressed in (False, True):
            changed = copy.deepcopy(request)
            changed["enabled"] = not suppressed
            if pointer == "/light/ambient":
                changed["environment"]["accessibilityPreferences"]["highContrast"] = suppressed
            source = negative_underflow_member_source(changed, pointer)
            failures.append((f"negative source {pointer} suppressed {suppressed}", source))
            failures.append((f"escaped negative {pointer} suppressed {suppressed}", source.replace('"ambient"', '"\\u0061mbient"').replace('"strength"', '"\\u0073trength"')))
            for literal in ("-0", "-0e-400", "1e-400"):
                valid = source.replace("-1e-400", literal, 1)
                successes.append((f"valid {literal} {pointer} suppressed {suppressed}", valid, expected_result(json.loads(valid))))
    for pointer in ("/light/ambient", "/light/keyLight/direction/x", "/light/interaction/strength", "/environment/qualityPolicy", "/normals/0/x"):
        failures.append((f"duplicate {pointer}", duplicate_member_source(request, pointer)))
    for pointer in ("/light/ambient", "/light/interaction/strength"):
        member = pointer.rsplit("/", 1)[1]
        alias = '"\\u%04x%s"' % (ord(member[0]), member[1:])
        source = duplicate_member_source(request, pointer).replace(json.dumps(member), alias, 1)
        failures.append((f"escaped duplicate {pointer}", source))
    source = json.dumps(request).replace('"ambient"', '"\\u0061mbient"').replace('"strength"', '"\\u0073trength"')
    successes.append(("escaped light coefficients", source, expected_result(request)))
    for pointer in ("/light/ambient", "/light/interaction/strength", "/normals/0/x"):
        failures.append((f"overflow {pointer}", nonfinite_member_source(request, pointer)))
    for pointer in ("/light", "/light/interaction", "/normals/0", "/environment"):
        value = request
        for segment in pointer.strip("/").split("/"):
            value = value[int(segment)] if isinstance(value, list) else value[segment]
        for member in value:
            changed = copy.deepcopy(request)
            parent = changed
            for segment in pointer.strip("/").split("/"):
                parent = parent[int(segment)] if isinstance(parent, list) else parent[segment]
            parent.pop(member)
            failures.append((f"missing {pointer}/{member}", json.dumps(changed)))
        changed = copy.deepcopy(request)
        replace_at_pointer(changed, pointer, list(value.values()))
        failures.append((f"positional {pointer}", json.dumps(changed)))
    for member in request:
        changed = copy.deepcopy(request)
        changed.pop(member)
        failures.append((f"missing request {member}", json.dumps(changed)))
    failures.extend(pigment_coefficient_failures(request, "/pigment/profiles"))
    for family in ("cast", "frost", "elastomer", "gel"):
        for ambient in (None, 0, 0.5, 0.999):
            for strength in (0, 0.25, 0.999):
                changed = copy.deepcopy(request)
                changed["pigment"]["materialFamily"] = family
                changed["light"]["ambient"] = ambient
                changed["light"]["interaction"]["strength"] = strength
                successes.append((f"{family} ambient {ambient} strength {strength}", json.dumps(changed), expected_result(changed)))
    return tuple(failures), tuple(successes)


if __name__ == "__main__":
    failures, successes = source_cases()
    raise SystemExit(check_backend(
        "material light", "schemas/material-light-case.schema.json",
        "schemas/material-light-request.schema.json", "schemas/material-light-result.schema.json",
        "conformance/lighting/material-light-vectors.json", extra_failures=failures,
        extra_successes=successes, compare=compare,
    ))
