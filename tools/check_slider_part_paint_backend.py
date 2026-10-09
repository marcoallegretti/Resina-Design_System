import argparse
import copy
import json
import math
import sys

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_failure
from check_extruded_contour_backend import contour_mismatch
from check_headless_backend import run_backend
from check_schemas import ROOT, apply_changes, check_case, load_json, parse_json, validator_for


def luminance(color):
    linear = [c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4 for c in color["components"]]
    return sum(c * w for c, w in zip(linear, (0.2126, 0.7152, 0.0722)))


def contrast(a, b):
    a, b = luminance(a), luminance(b)
    return (max(a, b) + 0.05) / (min(a, b) + 0.05)


def range_contrast(color, ranges):
    result = []
    light = luminance(color)
    for bounds in ranges:
        low, high = luminance(bounds["lower"]), luminance(bounds["upper"])
        result.append(1 if low <= light <= high else min(contrast(color, bounds["lower"]), contrast(color, bounds["upper"])))
    return min(result)


def color(channels, alpha=1):
    return dict(colorSpace="srgb", components=channels, alpha=alpha)


def rectangle(width, height, depth=0, radius=0):
    def point(x, y):
        return dict(x=x, y=y)

    def line(a, b):
        return dict(kind="line", **{"from": point(*a), "to": point(*b)})

    def arc(x, y, start, end):
        return dict(kind="arc", center=point(x, y), radii=point(radius, radius), start=point(*start), end=point(*end))

    if radius:
        segments = [arc(radius, radius, (-1, 0), (0, -1)), line((radius, 0), (width - radius, 0)),
                    arc(width - radius, radius, (0, -1), (1, 0)), line((width, radius), (width, height - radius))]
        if depth:
            segments.append(line((width, height - radius), (width, height - radius + depth)))
        segments += [arc(width - radius, height - radius + depth, (1, 0), (0, 1)),
                     line((width - radius, height + depth), (radius, height + depth)),
                     arc(radius, height - radius + depth, (0, 1), (-1, 0))]
        if depth:
            segments.append(line((0, height - radius + depth), (0, height - radius)))
        segments.append(line((0, height - radius), (0, radius)))
    else:
        points = [(0, 0), (width, 0), (width, height)]
        if depth:
            points += [(width, height + depth), (0, height + depth), (0, height)]
        else:
            points.append((0, height))
        segments = [line(a, b) for a, b in zip(points, points[1:] + points[:1])]
    return dict(schemaVersion="0.1.0", bounds=dict(x=0, y=0, width=width, height=height + depth), segments=segments)


def expected_result(request):
    theme = parse_json(request["theme"]["themeSource"], "slider arithmetic theme")
    surface = request["surface"]
    states = [s for s in ("rest", "hover", "focused", "pressed", "disabled", "dragging") if s in surface["states"]["states"]]
    phase = next((s for s in ("disabled", "dragging", "pressed") if s in states),
                 ("readOnlyHover" if "hover" in states else "readOnly") if request["readOnly"] else
                 "hover" if "hover" in states else "rest")
    family = theme["materialAssignments"]["control"][surface["materialRole"].split(".")[1]]
    response = dict(bodyMix=0, depthScale=1) if phase == "rest" else request["interactionAppearance"][request["part"]][family][phase]

    def assigned(role, opaque=True):
        assignment = theme["opaqueColorAssignments" if opaque else "colorAssignments"]["roles"][role]
        token = theme["tokens"]
        for member in assignment.split("."):
            token = token[member]
        value = token["$value"]
        if value["colorSpace"] != "srgb":
            raise ValueError("arithmetic fixture requires explicit sRGB tokens")
        return color(value["components"], value.get("alpha", 1))

    mix = response["bodyMix"]
    def mixed(value):
        return color([c * (1 + mix) if mix < 0 else c + (1 - c) * mix for c in value["components"]], value["alpha"])

    foreground = assigned(request["foregroundRole"])
    pigment = mixed(assigned(surface["colorRole"]))
    source = assigned(surface["colorRole"], False)
    environment = request["theme"]["environment"]
    preferences = environment["accessibilityPreferences"]
    frost_representation = "opaqueDimensional"
    if environment["rendererCapabilities"]["translucentSurfaces"] and not (preferences["reducedTransparency"] or preferences["highContrast"]):
        capabilities = environment["rendererCapabilities"]
        frost_representation = "translucentPigmented"
        if environment["qualityPolicy"] != "economy" and capabilities["backdropEffect"] and capabilities["backdropBlur"]:
            frost_representation = "shapedBackdrop" if capabilities["shapedBackdrop"] and environment["qualityPolicy"] == "full" else "regularBackdrop"
    content_fallback = False
    if family == "frost" and environment["rendererCapabilities"]["translucentSurfaces"] and not (preferences["reducedTransparency"] or preferences["highContrast"]):
        backdrop = request["postTreatmentBackdrop"]
        alpha = source["alpha"] * theme["frostPigment"]["tintStrength"]
        composite = color([c * alpha + b * (1 - alpha) for c, b in zip(mixed(source)["components"], backdrop["components"])])
        content_fallback = contrast(foreground, composite) < request["minimumContentContrast"]
        if not content_fallback:
            raise ValueError("arithmetic success requires an actual opaque Frost representation")
    width, height = request["size"]["width"], request["size"]["height"]
    depth = 2 * response["depthScale"]
    body = copy.deepcopy(load_json(ROOT / "conformance/ir/opaque-surface-expected.json"))
    body.update(materialRole=surface["materialRole"], colorRole=surface["colorRole"], materialFamily=family,
                form=surface["form"], states=dict(schemaVersion="0.1.0", states=states),
                foregroundRole=request["foregroundRole"], foreground=foreground,
                contentContrastRatio=contrast(foreground, pigment), contentFallbackApplied=content_fallback)
    profile = request["appearance"]["pigmentProfiles"]["profiles"][family]
    body["pigment"] = dict(schemaVersion="0.1.0", materialFamily=family, profile=profile, body=pigment,
                           side=color([c * (1 - profile["sideShade"]) for c in pigment["components"]]),
                           highlight=color([c + (1 - c) * profile["highlightLift"] for c in pigment["components"]]))
    body["lighting"]["sideOffset"] = dict(x=0, y=depth)
    body["geometry"] = dict(front=rectangle(width, height), silhouette=rectangle(width, height, depth),
                            edgeInterior=dict(offset=dict(x=1, y=1), contour=rectangle(width - 2, height - 2, depth)),
                            highlightOuter=dict(offset=dict(x=1, y=1), contour=rectangle(width - 2, height - 2)),
                            content=dict(offset=dict(x=2, y=2), contour=rectangle(width - 4, height - 4)))
    if family == "frost":
        body["frostRepresentation"] = "opaqueDimensional"
    def selection(role, background, minimum):
        candidate = assigned(role)
        ratio = range_contrast(candidate, background)
        fallback = ratio < minimum
        if fallback:
            role, candidate = "outline.strong", assigned("outline.strong")
            ratio = range_contrast(candidate, background)
        if ratio < minimum:
            raise ValueError("arithmetic success has insufficient common contrast")
        return dict(schemaVersion="0.1.0", colorRole=role, color=candidate, contrastRatio=ratio, fallbackApplied=fallback)
    body["edge"] = selection("outline", request["adjacentRanges"], request["minimumEdgeContrast"])
    paint = dict(schemaVersion="0.1.0", body=body)
    if request["part"] == "thumb" and "focused" in states:
        indicator = selection("focus", request["surroundingRanges"], 3)
        binding = copy.deepcopy(surface)
        binding.update(schemaVersion="0.4.0", materialFamily=family,
                       sourceColor=source, colorFallback=source,
                       opaqueColorFallback=assigned(surface["colorRole"]))
        assignment = theme["colorAssignments"]["roles"][surface["colorRole"]]
        token = theme["tokens"]
        for member in assignment.split("."):
            token = token[member]
        binding["sourceColor"] = token["$value"]
        if family == "frost":
            portable = assigned(surface["colorRole"]) if frost_representation == "opaqueDimensional" else color(source["components"], source["alpha"] * theme["frostPigment"]["tintStrength"])
            binding.update(frostRepresentation=frost_representation, frostPortableBody=portable)
        binding["states"] = body["states"]
        indicator.update(binding=binding, strokeWidth=2, gap=2)
        paint["focus"] = dict(schemaVersion="0.1.0", indicator=indicator, geometry=dict(
            silhouette=rectangle(width, height, depth),
            inner=dict(offset=dict(x=-2, y=-2), contour=rectangle(width + 4, height + 4, depth, 2)),
            outer=dict(offset=dict(x=-4, y=-4), contour=rectangle(width + 8, height + 8, depth, 4))))
    return dict(schemaVersion="0.1.0", part=request["part"], readOnly=request["readOnly"],
                layoutDirection=environment["layoutDirection"], phase=phase, response=response, paint=paint)


def check_success(command, request, timeout, name):
    expected = expected_result(request)
    validator = validator_for("schemas/slider-part-paint-ir.schema.json")
    previous = None
    for _ in range(2):
        completed = run_backend(command, json.dumps(request, allow_nan=False), timeout)
        if completed.returncode != 0 or completed.stderr:
            raise AssertionError(f"{name}: {completed.returncode}: {completed.stderr[:200]}")
        actual = parse_json(completed.stdout, name)
        check_case(validator, name, actual, True)
        difference = contour_mismatch(actual, expected)
        if difference:
            raise AssertionError(f"{name}: output differs at {difference}")
        if previous is not None and actual != previous:
            raise AssertionError(f"{name}: nondeterministic output")
        previous = actual


def main():
    parser = argparse.ArgumentParser(description="Check portable Slider part paint through its public protocol.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command and args.command[0] == "--" else args.command
    if not command or not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("provide a backend command and a positive finite timeout")
    base = load_json(ROOT / "conformance/ir/slider-part-paint-request.json")
    count, names = 0, set()
    try:
        for case in load_json(ROOT / "conformance/ir/slider-part-paint-cases.json"):
            name = case["name"]
            if name in names:
                raise ValueError(f"duplicate Slider part paint case: {name}")
            names.add(name)
            check_case(validator_for("schemas/slider-part-paint-case.schema.json"), name, case, True)
            request = apply_changes(base, case["requestChanges"])
            check_case(validator_for("schemas/slider-part-paint-request.schema.json"), name, request, case["requestSchemaValid"])
            if case.get("failure"):
                check_failure(command, json.dumps(request), args.timeout, name)
            else:
                if expected_result(request)["phase"] != case["expectedPhase"]:
                    raise ValueError(f"{name}: phase disagrees with states")
                check_success(command, request, args.timeout, name)
            count += 1
        phases = load_json(ROOT / "conformance/interaction/slider-phase-cases.json")
        for family in ("cast", "frost", "elastomer"):
            for part in ("track", "thumb"):
                for index, case in enumerate(phases):
                    request = copy.deepcopy(base)
                    theme = parse_json(request["theme"]["themeSource"], family)
                    theme["materialAssignments"]["control"]["interactive"] = family
                    request["theme"]["themeSource"] = json.dumps(theme)
                    request["theme"]["environment"]["layoutDirection"] = "ltr" if index % 2 else "rtl"
                    request["part"], request["readOnly"] = part, case["readOnly"]
                    request["surface"]["states"] = case["states"]
                    name = f"{family} {part} {case['name']}"
                    if "expected" in case:
                        check_success(command, request, args.timeout, name)
                    else:
                        check_failure(command, json.dumps(request), args.timeout, name)
                    count += 1
        failures = [("duplicate version", duplicate_member_source(base, "/schemaVersion")),
                    ("escaped duplicate range", duplicate_member_source(base, "/adjacentRanges/0/lower/alpha").replace('"alpha":1,"alpha":1', '"alpha":1,"\\u0061lpha":1', 1)),
                    ("positional root", json.dumps(list(base.values()))),
                    ("nonfinite threshold", nonfinite_member_source(base, "/minimumEdgeContrast"))]
        for part in ("track", "thumb"):
            request = copy.deepcopy(base)
            request.update(part=part)
            request.pop("postTreatmentBackdrop")
            request.pop("surroundingRanges")
            if part == "track":
                request["surface"]["states"]["states"] = ["rest", "focused"]
            check_success(command, request, args.timeout, f"{part} omitted optional inputs")
            count += 1
        request["surface"]["states"]["states"] = ["rest", "focused"]
        failures.append(("focused thumb without surroundings", json.dumps(request)))
        advanced = next(case for case in load_json(ROOT / "conformance/ir/slider-part-paint-cases.json")
                        if case["name"] == "translucentPigmented legibility fallback")
        request = apply_changes(base, advanced["requestChanges"])
        request.pop("postTreatmentBackdrop")
        failures.append(("Frost without actual backdrop", json.dumps(request)))
        for field in base:
            if field not in ("postTreatmentBackdrop", "surroundingRanges"):
                changed = copy.deepcopy(base)
                del changed[field]
                failures.append((f"missing {field}", json.dumps(changed)))
        for name, source in failures:
            check_failure(command, source, args.timeout, name)
            count += 1
    except (AssertionError, KeyError, TypeError, OSError, ValueError) as error:
        print(f"FAIL Slider part paint: {error}", file=sys.stderr)
        return 1
    print(f"Slider part paint backend passed {count} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
