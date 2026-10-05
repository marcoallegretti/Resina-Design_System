import copy
import json
import math
import re
from functools import lru_cache
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[1]


def parse_json(source, label):
    def unique_members(pairs):
        result = {}
        for name, value in pairs:
            if name in result:
                raise ValueError(f"duplicate JSON member {name!r} in {label}")
            result[name] = value
        return result

    def invalid_constant(value):
        raise ValueError(f"invalid JSON number {value} in {label}")

    def finite_float(value):
        number = float(value)
        if not math.isfinite(number):
            raise ValueError(f"JSON number exceeds finite range in {label}")
        return number

    def finite_int(value):
        number = int(value)
        try:
            finite = math.isfinite(float(number))
        except OverflowError:
            finite = False
        if not finite:
            raise ValueError(f"JSON number exceeds finite range in {label}")
        return number

    return json.loads(
        source,
        object_pairs_hook=unique_members,
        parse_constant=invalid_constant,
        parse_float=finite_float,
        parse_int=finite_int,
    )


def load_json(path):
    with path.open(encoding="utf-8") as source:
        return parse_json(source.read(), path)


@lru_cache(maxsize=1)
def schema_registry():
    registry = Registry()
    for path in (ROOT / "schemas").rglob("*.schema.json"):
        schema = load_json(path)
        registry = registry.with_resource(schema["$id"], Resource.from_contents(schema))
    return registry


def validator_for(path):
    schema = load_json(ROOT / path)
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema, registry=schema_registry())


def check_case(validator, name, document, expected_valid):
    errors = list(validator.iter_errors(document))
    if bool(errors) == expected_valid:
        detail = errors[0].message if errors else "unexpectedly valid"
        raise AssertionError(f"{name}: {detail}")


def pointer_member(container, segment, pointer):
    if isinstance(container, dict):
        if segment in container:
            return segment
    elif isinstance(container, list) and re.fullmatch(r"0|[1-9][0-9]*", segment):
        index = int(segment)
        if index < len(container):
            return index
    raise ValueError(f"JSON Pointer does not name an existing member: {pointer!r}")


def replace_at_pointer(document, pointer, value):
    if not pointer.startswith("/"):
        raise ValueError(f"invalid JSON Pointer: {pointer!r}")
    segments = pointer[1:].split("/")
    if any(re.search(r"~(?![01])", segment) for segment in segments):
        raise ValueError(f"invalid JSON Pointer escape: {pointer!r}")
    decoded = [segment.replace("~1", "/").replace("~0", "~") for segment in segments]
    current = document
    for segment in decoded[:-1]:
        current = current[pointer_member(current, segment, pointer)]
    current[pointer_member(current, decoded[-1], pointer)] = value


def apply_changes(document, changes):
    result = copy.deepcopy(document)
    for change in changes:
        replace_at_pointer(result, change["path"], change["value"])
    return result


def check_vectors(schema_path, vector_path):
    validator = validator_for(schema_path)
    vectors = load_json(ROOT / vector_path)
    for vector in vectors:
        if ("expected" in vector) == ("error" in vector):
            raise ValueError(f"{vector_path}: {vector['name']} needs one outcome")
        check_case(
            validator,
            f"{vector_path}: {vector['name']}",
            vector["document"],
            "expected" in vector,
        )
    return len(vectors)


def check_label_expansion(expansion):
    check_case(
        validator_for("schemas/command-label-expansion.schema.json"),
        "label length expansion",
        expansion,
        True,
    )
    baseline_length = len(expansion["baselineText"])
    if not expansion["baselineText"].strip():
        raise ValueError("label expansion baseline must not be blank")
    targets, names = set(), set()
    for case in expansion["cases"]:
        if case["name"] in names or case["targetExpansion"] in targets:
            raise ValueError("duplicate label expansion case or target")
        names.add(case["name"])
        targets.add(case["targetExpansion"])
        if not case["text"].strip() or len(case["text"]) != case["scalarLength"]:
            raise ValueError("label expansion text disagrees with declared length")
        actual = case["scalarLength"] / baseline_length
        if abs(actual / case["targetExpansion"] - 1.0) > 0.05:
            raise ValueError("label expansion case is not within five percent of target")
    if targets != {1.0, 1.5, 2.0}:
        raise ValueError("label expansion needs 100, 150 and 200 percent coverage")
    return 1 + len(expansion["cases"])


def main():
    schema_paths = sorted((ROOT / "schemas").rglob("*.schema.json"))
    expected_paths = {
        "schemas/command-label-expansion.schema.json",
        "schemas/command-label-ir.schema.json",
        "schemas/command-accessibility-ir.schema.json",
        "schemas/toggle-snapshot-cases.schema.json",
        "schemas/toggle-travel-cases.schema.json",
        "schemas/toggle-part-body-ir.schema.json",
        "schemas/toggle-part-paint-ir.schema.json",
        "schemas/toggle-part-paint-request.schema.json",
        "schemas/toggle-part-paint-case.schema.json",
        "schemas/toggle-part-motion-request.schema.json",
        "schemas/toggle-part-motion-ir.schema.json",
        "schemas/toggle-part-motion-case.schema.json",
        "schemas/toggle-layout-request.schema.json",
        "schemas/toggle-layout-ir.schema.json",
        "schemas/toggle-layout-case.schema.json",
        "schemas/toggle-accessibility-ir.schema.json",
        "schemas/toggle-accessibility-case.schema.json",
        "schemas/command-accessibility-case.schema.json",
        "schemas/command-motion-request.schema.json",
        "schemas/command-motion-ir.schema.json",
        "schemas/command-motion-case.schema.json",
        "schemas/spring-dynamics.schema.json",
        "schemas/spring-state.schema.json",
        "schemas/spring-trajectory-request.schema.json",
        "schemas/spring-trajectory-result.schema.json",
        "schemas/spring-trajectory-case.schema.json",
        "schemas/command-appearance.schema.json",
        "schemas/slider-appearance.schema.json",
        "schemas/slider-phase-cases.schema.json",
        "schemas/command-paint-request.schema.json",
        "schemas/command-body-ir.schema.json",
        "schemas/command-paint-ir.schema.json",
        "schemas/command-paint-case.schema.json",
        "schemas/spring-parameters.schema.json",
        "schemas/spring-request.schema.json",
        "schemas/spring-result.schema.json",
        "schemas/spring-case.schema.json",
        "schemas/filled-contour.schema.json",
        "schemas/placed-contour.schema.json",
        "schemas/focus-ir-request.schema.json",
        "schemas/focus-indicator-ir.schema.json",
        "schemas/focus-ir-case.schema.json",
        "schemas/focus-paint-case.schema.json",
        "schemas/material-scene-manifest.schema.json",
        "schemas/opaque-surface-appearance.schema.json",
        "schemas/opaque-surface-request.schema.json",
        "schemas/opaque-surface-ir.schema.json",
        "schemas/opaque-surface-case.schema.json",
        "schemas/surface-paint-request.schema.json",
        "schemas/surface-paint-ir.schema.json",
        "schemas/surface-paint-case.schema.json",
        "schemas/hit-region-request.schema.json",
        "schemas/hit-region-ir.schema.json",
        "schemas/hit-region-case.schema.json",
        "schemas/hit-membership-case.schema.json",
        "schemas/focus-traversal-request.schema.json",
        "schemas/focus-traversal-result.schema.json",
        "schemas/focus-traversal-case.schema.json",
        "schemas/slider-value-request.schema.json",
        "schemas/slider-value-ir.schema.json",
        "schemas/slider-value-case.schema.json",
        "schemas/slider-adjustment-ir.schema.json",
        "schemas/slider-adjustment-cases.schema.json",
        "schemas/slider-accessibility-ir.schema.json",
        "schemas/slider-accessibility-case.schema.json",
        "schemas/slider-layout-ir.schema.json",
        "schemas/slider-layout-cases.schema.json",
        "schemas/slider-position-cases.schema.json",
        "schemas/slider-edit-session.schema.json",
        "schemas/slider-value-policy.schema.json",
        "schemas/slider-value-policy-fixture.schema.json",
        "schemas/slider-key-policy.schema.json",
        "schemas/slider-key-ir.schema.json",
        "schemas/slider-key-cases.schema.json",
        "schemas/slider-presentation.schema.json",
        "schemas/slider-presentation-cases.schema.json",
        "schemas/slider-states-cases.schema.json",
        "schemas/slider-edit-result.schema.json",
        "schemas/slider-edit-cases.schema.json",
        "schemas/slider-anchor-cases.schema.json",
        "schemas/slider-pointer-state.schema.json",
        "schemas/slider-pointer-result.schema.json",
        "schemas/slider-pointer-cases.schema.json",
        "schemas/slider-stops-ir.schema.json",
        "schemas/slider-stops-cases.schema.json",
        "schemas/slider-stop-adjustment-cases.schema.json",
        "schemas/toggle-activation-request.schema.json",
        "schemas/toggle-activation-result.schema.json",
        "schemas/toggle-activation-case.schema.json",
        "schemas/activation-state.schema.json",
        "schemas/activation-request.schema.json",
        "schemas/activation-result.schema.json",
        "schemas/activation-case.schema.json",
        "schemas/toggle-states-request.schema.json",
        "schemas/toggle-states-case.schema.json",
        "schemas/command-states-request.schema.json",
        "schemas/command-states-case.schema.json",
        "schemas/extruded-contour-request.schema.json",
        "schemas/extruded-contour-result.schema.json",
        "schemas/extruded-contour-case.schema.json",
        "schemas/physical-vector.schema.json",
        "schemas/edge-contrast-request.schema.json",
        "schemas/edge-contrast-result.schema.json",
        "schemas/edge-contrast-case.schema.json",
        "schemas/elevation-depth-assignments.schema.json",
        "schemas/elevation-depth-request.schema.json",
        "schemas/elevation-depth-result.schema.json",
        "schemas/elevation-depth-case.schema.json",
        "schemas/color-assignments.schema.json",
        "schemas/corner-radius-case.schema.json",
        "schemas/environment.schema.json",
        "schemas/frost-pigment.schema.json",
        "schemas/frost-legibility-request.schema.json",
        "schemas/frost-legibility-result.schema.json",
        "schemas/frost-legibility-case.schema.json",
        "schemas/frost-surface-readability-request.schema.json",
        "schemas/frost-surface-readability-result.schema.json",
        "schemas/frost-surface-readability-case.schema.json",
        "schemas/surface-readability-request.schema.json",
        "schemas/surface-readability-result.schema.json",
        "schemas/surface-readability-case.schema.json",
        "schemas/focus-indicator-request.schema.json",
        "schemas/focus-indicator-result.schema.json",
        "schemas/focus-indicator-case.schema.json",
        "schemas/headless-resolution.schema.json",
        "schemas/headless-conformance-case.schema.json",
        "schemas/headless-result.schema.json",
        "schemas/material-assignments.schema.json",
        "schemas/logical-corner-radii.schema.json",
        "schemas/opaque-color-assignments.schema.json",
        "schemas/opaque-srgb-fallback.schema.json",
        "schemas/opaque-pigment-profiles.schema.json",
        "schemas/opaque-pigment-request.schema.json",
        "schemas/opaque-pigment-result.schema.json",
        "schemas/opaque-pigment-case.schema.json",
        "schemas/inset-contour-request.schema.json",
        "schemas/inset-contour-result.schema.json",
        "schemas/inset-contour-case.schema.json",
        "schemas/key-light.schema.json",
        "schemas/key-light-request.schema.json",
        "schemas/key-light-result.schema.json",
        "schemas/key-light-case.schema.json",
        "schemas/shape-fallback-assignments.schema.json",
        "schemas/shape-fallback-assignment-case.schema.json",
        "schemas/shape-fallback-case.schema.json",
        "schemas/shape-fallback-request.schema.json",
        "schemas/shape-fallback-result.schema.json",
        "schemas/resolver-module-case.schema.json",
        "schemas/resolver-module-request.schema.json",
        "schemas/spatial-assignments.schema.json",
        "schemas/srgb-fallback.schema.json",
        "schemas/state-set.schema.json",
        "schemas/state-composition.schema.json",
        "schemas/surface-form.schema.json",
        "schemas/surface-size.schema.json",
        "schemas/surface-binding.schema.json",
        "schemas/surface-binding-result.schema.json",
        "schemas/surface-scenario.schema.json",
        "schemas/treatment-stack.schema.json",
        "schemas/typography-assignments.schema.json",
        "schemas/theme-source.schema.json",
        "schemas/theme-source-case.schema.json",
        "schemas/theme-resolution-case.schema.json",
        "schemas/theme-resolution-request.schema.json",
        "schemas/versions/material-assignments-0.1.0.schema.json",
        "schemas/versions/surface-binding-0.1.0.schema.json",
        "schemas/versions/surface-binding-result-0.1.0.schema.json",
        "schemas/versions/surface-scenario-0.1.0.schema.json",
        "schemas/versions/environment-0.1.0.schema.json",
        "schemas/versions/headless-result-0.1.0.schema.json",
        "schemas/versions/headless-resolution-0.1.0.schema.json",
        "schemas/versions/headless-result-0.2.0.schema.json",
        "schemas/versions/surface-scenario-0.2.0.schema.json",
        "schemas/versions/surface-binding-result-0.2.0.schema.json",
        "schemas/versions/headless-resolution-0.2.0.schema.json",
        "schemas/versions/headless-result-0.3.0.schema.json",
        "schemas/versions/surface-binding-result-0.3.0.schema.json",
        "schemas/versions/surface-scenario-0.3.0.schema.json",
    }
    actual_paths = {path.relative_to(ROOT).as_posix() for path in schema_paths}
    if actual_paths != expected_paths:
        raise ValueError(f"schema coverage differs: {actual_paths ^ expected_paths}")
    for path in schema_paths:
        validator_for(path.relative_to(ROOT))

    material_scenes = load_json(ROOT / "conformance/scenes/tier0-materials.json")
    material_scene_schema = validator_for("schemas/material-scene-manifest.schema.json")
    check_case(material_scene_schema, "authored material scenes", material_scenes, True)
    invalid_material_scenes = [
        ("wrong manifest version", "/schemaVersion", "0.2.0"),
        ("parent asset path", "/environmentSource", "../environment.json"),
        ("absolute asset path", "/environmentSource", "/environment.json"),
        ("drive asset path", "/environmentSource", "C:/environment.json"),
        ("remote asset path", "/environmentSource", "https://example.com/environment.json"),
        ("newline asset path", "/environmentSource", "environment.json\n"),
        ("zero surface width", "/size/width", 0),
        ("zero capture width", "/capture/width", 0),
        ("zero capture scale", "/capture/pixelsPerUnit", 0),
        ("unsupported sample grid", "/capture/samplesPerAxis", 9),
        ("empty scene catalog", "/scenarios", []),
        ("unknown scene kind", "/scenarios/0/kind", "component"),
        ("missing body guards", "/scenarios/1/kind", "opaqueSurface"),
        ("ignored ring guards", "/scenarios/0/kind", "focusRing"),
        ("invalid contrast", "/scenarios/0/minimumContentContrast", 22),
    ]
    for name, pointer, value in invalid_material_scenes:
        changed = copy.deepcopy(material_scenes)
        replace_at_pointer(changed, pointer, value)
        check_case(material_scene_schema, name, changed, False)
    check_case(material_scene_schema, "unknown scene manifest field", {**material_scenes, "extra": True}, False)

    command_request = load_json(ROOT / "conformance/ir/command-paint-request.json")
    slider_appearance = load_json(ROOT / "conformance/appearance/slider-appearance.json")
    slider_appearance_schema = validator_for("schemas/slider-appearance.schema.json")
    check_case(slider_appearance_schema, "slider arithmetic profiles", slider_appearance, True)
    for part in ("track", "thumb"):
        for family in ("cast", "frost", "elastomer"):
            for phase in ("hover", "pressed", "dragging", "disabled", "readOnly", "readOnlyHover"):
                missing = copy.deepcopy(slider_appearance)
                del missing[part][family][phase]
                check_case(slider_appearance_schema, "missing slider phase", missing, False)
                for key, value in (("bodyMix", -1.01), ("bodyMix", 1.01),
                                   ("depthScale", -0.01), ("depthScale", 1.01),
                                   ("unknown", 0)):
                    invalid = copy.deepcopy(slider_appearance)
                    invalid[part][family][phase][key] = value
                    check_case(slider_appearance_schema, "invalid slider response", invalid, False)
    phase_cases = load_json(ROOT / "conformance/interaction/slider-phase-cases.json")
    phase_schema = validator_for("schemas/slider-phase-cases.schema.json")
    check_case(phase_schema, "slider phase cases", phase_cases, True)
    if len({case["name"] for case in phase_cases}) != len(phase_cases):
        raise ValueError("duplicate slider phase case names")
    for key, value in (("readOnly", None), ("expected", "active"), ("extra", True)):
        invalid = copy.deepcopy(phase_cases)
        invalid[0][key] = value
        check_case(phase_schema, "invalid slider phase record", invalid, False)
    for name in ("light", "dark"):
        check_case(validator_for("schemas/command-appearance.schema.json"),
                   "authored command " + name,
                   load_json(ROOT / ("definitions/command-appearance-" + name + ".json")), True)
    command_request_schema = validator_for("schemas/command-paint-request.schema.json")
    for case in load_json(ROOT / "conformance/ir/command-paint-cases.json"):
        check_case(validator_for("schemas/command-paint-case.schema.json"), case["name"], case, True)
        check_case(command_request_schema, case["name"],
                   apply_changes(command_request, case["requestChanges"]), case["requestSchemaValid"])

    spring_request = load_json(ROOT / "conformance/motion/spring-request.json")
    spring_expected = load_json(ROOT / "conformance/motion/spring-expected.json")
    spring_cases = load_json(ROOT / "conformance/motion/spring-cases.json")
    spring_case_schema = validator_for("schemas/spring-case.schema.json")
    spring_request_schema = validator_for("schemas/spring-request.schema.json")
    spring_result_schema = validator_for("schemas/spring-result.schema.json")
    check_case(spring_request_schema, "spring baseline", spring_request, True)
    check_case(spring_result_schema, "spring baseline result", spring_expected, True)
    names = set()
    for case in spring_cases:
        check_case(spring_case_schema, case["name"], case, True)
        if case["name"] in names:
            raise ValueError(f"duplicate spring case: {case['name']}")
        names.add(case["name"])
        check_case(
            spring_request_schema, case["name"],
            apply_changes(spring_request, case["requestChanges"]), case["requestSchemaValid"],
        )
        if "errorContains" not in case:
            expected = case.get(
                "expected", apply_changes(spring_expected, case.get("expectedChanges", [])),
            )
            check_case(spring_result_schema, case["name"], expected, True)
    for name, changes in (
        ("settled position must be endpoint", {"settled": True}),
        ("immediate response must settle", {"representation": "immediate"}),
    ):
        check_case(spring_result_schema, name, {**spring_expected, **changes}, False)

    focus_ir_request = load_json(ROOT / "conformance/ir/focus-ir-request.json")
    focus_ir_expected = load_json(ROOT / "conformance/ir/focus-ir-expected.json")
    focus_ir_cases = load_json(ROOT / "conformance/ir/focus-ir-cases.json")
    focus_ir_case_schema = validator_for("schemas/focus-ir-case.schema.json")
    focus_ir_request_schema = validator_for("schemas/focus-ir-request.schema.json")
    focus_ir_result_schema = validator_for("schemas/focus-indicator-ir.schema.json")
    focus_paint_vectors = load_json(ROOT / "conformance/ir/focus-paint-vectors.json")
    focus_paint_schema = validator_for("schemas/focus-paint-case.schema.json")
    names = set()
    for vector in focus_paint_vectors:
        check_case(focus_paint_schema, vector["name"], vector, True)
        if vector["name"] in names:
            raise ValueError(f"duplicate focus paint case: {vector['name']}")
        names.add(vector["name"])
    for name, field, value in (
        ("numeric coverage", "covered", 1),
        ("extra point field", "point", {"x": 0, "y": 0, "z": 0}),
        ("unknown case field", "extra", True),
    ):
        check_case(focus_paint_schema, name, {**focus_paint_vectors[0], field: value}, False)
    names = set()
    for case in focus_ir_cases:
        check_case(focus_ir_case_schema, case["name"], case, True)
        if case["name"] in names:
            raise ValueError(f"duplicate focus IR case: {case['name']}")
        names.add(case["name"])
        check_case(
            focus_ir_request_schema, case["name"],
            apply_changes(focus_ir_request, case["requestChanges"]), case["requestSchemaValid"],
        )
        if "errorContains" not in case:
            expected = case.get(
                "expected", apply_changes(focus_ir_expected, case.get("expectedChanges", []))
            )
            check_case(focus_ir_result_schema, case["name"], expected, True)
    invalid_focus_ir_results = (
        ("missing focus state", "/indicator/binding/states/states", ["selected"]),
        ("translucent focus pigment", "/indicator/color/alpha", 0.5),
        ("empty silhouette", "/geometry/silhouette/bounds", None),
        ("empty outer contour", "/geometry/outer/contour/segments", []),
        ("inconsistent fallback", "/indicator/fallbackApplied", True),
        ("zero stroke width", "/indicator/strokeWidth", 0),
        ("incorrect inner translation", "/geometry/inner/offset/x", 0),
        ("incorrect outer translation", "/geometry/outer/offset/y", -2),
    )
    for name, pointer, value in invalid_focus_ir_results:
        invalid = copy.deepcopy(focus_ir_expected)
        replace_at_pointer(invalid, pointer, value)
        check_case(focus_ir_result_schema, name, invalid, False)

    appearance_schema = validator_for("schemas/opaque-surface-appearance.schema.json")
    check_case(
        appearance_schema, "authored opaque surface appearance",
        load_json(ROOT / "definitions/tier0-surface-appearance.json"), True,
    )
    check_case(
        validator_for("schemas/elevation-depth-assignments.schema.json"),
        "authored Tier 0 depth assignments",
        load_json(ROOT / "definitions/tier0-depth.json"), True,
    )
    ir_request = load_json(ROOT / "conformance/ir/opaque-surface-request.json")
    ir_expected = load_json(ROOT / "conformance/ir/opaque-surface-expected.json")
    ir_cases = load_json(ROOT / "conformance/ir/opaque-surface-cases.json")
    ir_case_schema = validator_for("schemas/opaque-surface-case.schema.json")
    ir_request_schema = validator_for("schemas/opaque-surface-request.schema.json")
    ir_result_schema = validator_for("schemas/opaque-surface-ir.schema.json")
    ir_names = set()
    for case in ir_cases:
        check_case(ir_case_schema, case["name"], case, True)
        if case["name"] in ir_names:
            raise ValueError(f"duplicate opaque surface case: {case['name']}")
        ir_names.add(case["name"])
        check_case(
            ir_request_schema, case["name"],
            apply_changes(ir_request, case["requestChanges"]), case["requestSchemaValid"],
        )
        if "errorContains" not in case:
            result = case.get(
                "expected", apply_changes(ir_expected, case.get("expectedChanges", []))
            )
            check_case(ir_result_schema, case["name"], result, True)
    invalid_ir_results = (
        ("nonopaque representation", "/representation", "translucent"),
        ("non-rest state", "/states/states", ["pressed"]),
        ("translucent body", "/pigment/body/alpha", 0.5),
        ("empty content", "/geometry/content/contour/bounds", None),
        ("unresolved Frost representation on Cast", "/materialFamily", "frost"),
        ("zero exterior edge width", "/edgeWidth", 0),
        ("negative highlight width", "/highlightWidth", -1),
        ("inconsistent pigment family", "/pigment/materialFamily", "gel"),
    )
    for name, pointer, value in invalid_ir_results:
        invalid = copy.deepcopy(ir_expected)
        replace_at_pointer(invalid, pointer, value)
        check_case(ir_result_schema, name, invalid, False)
    invalid = copy.deepcopy(ir_expected)
    invalid["materialFamily"] = "gel"
    invalid["pigment"]["materialFamily"] = "gel"
    check_case(ir_result_schema, "structural role cannot carry Gel", invalid, False)

    check_case(
        validator_for("schemas/key-light.schema.json"),
        "reference key light",
        load_json(ROOT / "definitions/key-light.json"),
        True,
    )
    extrusion_cases = load_json(ROOT / "conformance/geometry/extruded-contour-vectors.json")
    extrusion_case_schema = validator_for("schemas/extruded-contour-case.schema.json")
    extrusion_request_schema = validator_for("schemas/extruded-contour-request.schema.json")
    extrusion_names = set()
    for case in extrusion_cases:
        check_case(extrusion_case_schema, case["name"], case, True)
        if case["name"] in extrusion_names:
            raise ValueError(f"duplicate extruded contour case: {case['name']}")
        extrusion_names.add(case["name"])
        check_case(extrusion_request_schema, case["name"], case["request"], case["requestSchemaValid"])
    extrusion_result_schema = validator_for("schemas/extruded-contour-result.schema.json")
    arc_result = next(
        case["expected"] for case in extrusion_cases
        if "expected" in case and case["expected"]["segments"]
        and case["expected"]["segments"][0]["kind"] == "arc"
    )
    invalid_extrusion_results = (
        ("null bounds with filled contour", "/bounds", None),
        ("filled bounds with empty contour", "/segments", []),
        ("zero width bounds", "/bounds/width", 0),
        ("zero arc radius", "/segments/0/radii/x", 0),
        ("zero radial direction", "/segments/0/start", {"x": 0, "y": 0}),
        ("radial outside unit range", "/segments/0/end/x", 2),
        ("unknown segment kind", "/segments/0/kind", "circle"),
        ("unknown vector member", "/segments/0/center", {"x": 2, "y": 1, "z": 0}),
    )
    for name, pointer, value in invalid_extrusion_results:
        invalid = copy.deepcopy(arc_result)
        replace_at_pointer(invalid, pointer, value)
        check_case(extrusion_result_schema, name, invalid, False)
    key_light_cases = load_json(ROOT / "conformance/lighting/key-light-vectors.json")
    key_light_case_schema = validator_for("schemas/key-light-case.schema.json")
    key_light_request_schema = validator_for("schemas/key-light-request.schema.json")
    key_light_names = set()
    for case in key_light_cases:
        check_case(key_light_case_schema, case["name"], case, True)
        if case["name"] in key_light_names:
            raise ValueError(f"duplicate key light case: {case['name']}")
        key_light_names.add(case["name"])
        check_case(key_light_request_schema, case["name"], case["request"], case["requestSchemaValid"])
    contour_cases = load_json(ROOT / "conformance/geometry/inset-contour-vectors.json")
    contour_case_schema = validator_for("schemas/inset-contour-case.schema.json")
    contour_request_schema = validator_for("schemas/inset-contour-request.schema.json")
    contour_names = set()
    for case in contour_cases:
        check_case(contour_case_schema, case["name"], case, True)
        if case["name"] in contour_names:
            raise ValueError(f"duplicate inset contour case: {case['name']}")
        contour_names.add(case["name"])
        check_case(contour_request_schema, case["name"], case["request"], case["requestSchemaValid"])
    pigment_profile_schema = validator_for("schemas/opaque-pigment-profiles.schema.json")
    check_case(
        pigment_profile_schema,
        "reference opaque pigment profiles",
        load_json(ROOT / "definitions/tier0-pigment.json"),
        True,
    )
    pigment_profile_vectors = load_json(
        ROOT / "conformance/materials/opaque-pigment-profile-vectors.json"
    )
    pigment_profile_names = set()
    for vector in pigment_profile_vectors:
        if (
            set(vector) != {"name", "document", "valid"}
            or not isinstance(vector["name"], str)
            or not vector["name"]
            or not isinstance(vector["valid"], bool)
            or vector["name"] in pigment_profile_names
        ):
            raise ValueError("invalid or duplicate opaque pigment profile vector")
        pigment_profile_names.add(vector["name"])
        check_case(pigment_profile_schema, vector["name"], vector["document"], vector["valid"])
    pigment_cases = load_json(ROOT / "conformance/materials/opaque-pigment-vectors.json")
    pigment_case_schema = validator_for("schemas/opaque-pigment-case.schema.json")
    pigment_request_schema = validator_for("schemas/opaque-pigment-request.schema.json")
    pigment_names = set()
    for case in pigment_cases:
        if case["name"] in pigment_names:
            raise ValueError(f"duplicate opaque pigment case: {case['name']}")
        pigment_names.add(case["name"])
        check_case(pigment_case_schema, case["name"], case, True)
        check_case(pigment_request_schema, case["name"], case["request"], case["requestSchemaValid"])

    resolver_case_schema = validator_for("schemas/resolver-module-case.schema.json")
    resolver_cases = load_json(ROOT / "conformance/tokens/resolver-module-vectors.json")
    resolver_case_names = set()
    for case in resolver_cases:
        check_case(resolver_case_schema, f"resolver module case: {case.get('name')}", case, True)
        if case["name"] in resolver_case_names:
            raise ValueError(f"duplicate resolver module case name: {case['name']}")
        resolver_case_names.add(case["name"])
    resolver_request_schema = validator_for("schemas/resolver-module-request.schema.json")
    resolver_request = {
        "schemaVersion": "0.1.0",
        "resolver": resolver_cases[0]["resolver"],
        "input": resolver_cases[0]["input"],
        "externalSources": resolver_cases[0]["externalSources"],
    }
    check_case(resolver_request_schema, "resolver module request", resolver_request, True)
    for name, field, value in [
        ("unsupported version", "schemaVersion", "0.2.0"),
        ("invalid input source", "input", {}),
        ("invalid external source", "externalSources", {"file.json": 1}),
        ("unknown member", "unknown", True),
    ]:
        invalid_request = copy.deepcopy(resolver_request)
        invalid_request[field] = value
        check_case(resolver_request_schema, f"resolver module request {name}", invalid_request, False)
    missing_input = copy.deepcopy(resolver_request)
    del missing_input["input"]
    check_case(resolver_request_schema, "resolver module request missing input", missing_input, False)
    valid_resolver_case = next((case for case in resolver_cases if "expected" in case), None)
    if valid_resolver_case is None:
        raise ValueError("resolver module cases need a successful example")
    for name, change in [
        ("missing input", ("input", None)),
        ("wrong external source type", ("externalSources", {"file.json": 5})),
        ("empty diagnostic", ("errorContains", "")),
        ("ambiguous outcome", ("errorContains", "invalid")),
    ]:
        field, value = change
        example = copy.deepcopy(valid_resolver_case)
        if name == "empty diagnostic":
            del example["expected"]
        if value is None:
            del example[field]
        else:
            example[field] = value
        check_case(resolver_case_schema, f"resolver module {name}", example, False)

    theme_source_path = ROOT / "conformance/themes/valid-source.json"
    theme_source_text = theme_source_path.read_text(encoding="utf-8")
    theme_source = parse_json(theme_source_text, theme_source_path)
    theme_resolution_case_schema = validator_for("schemas/theme-resolution-case.schema.json")
    theme_resolution_cases = load_json(ROOT / "conformance/themes/resolution-cases.json")
    theme_resolution_names = set()
    for case in theme_resolution_cases:
        check_case(
            theme_resolution_case_schema,
            f"theme resolution case: {case.get('name')}",
            case,
            True,
        )
        if case["name"] in theme_resolution_names:
            raise ValueError(f"duplicate theme resolution case name: {case['name']}")
        theme_resolution_names.add(case["name"])
    theme_request_schema = validator_for("schemas/theme-resolution-request.schema.json")
    theme_request = {
        "schemaVersion": "0.1.0",
        "themeSource": theme_source_text,
        "externalSources": {},
        "environment": load_json(ROOT / "conformance/headless/valid-request.json")["environment"],
    }
    check_case(theme_request_schema, "theme resolution request", theme_request, True)
    for field, value in [
        ("schemaVersion", "0.2.0"),
        ("themeSource", {}),
        ("externalSources", {"file.json": 1}),
        ("unknown", True),
    ]:
        invalid_request = copy.deepcopy(theme_request)
        invalid_request[field] = value
        check_case(theme_request_schema, f"theme request invalid {field}", invalid_request, False)
    theme_schema = validator_for("schemas/theme-source.schema.json")
    theme_case_schema = validator_for("schemas/theme-source-case.schema.json")
    theme_cases = load_json(ROOT / "conformance/themes/source-cases.json")
    theme_case_names = set()
    resolver_theme = copy.deepcopy(theme_source)
    embedded_tokens = resolver_theme.pop("tokens")
    resolver_theme["tokenResolver"] = {
        "version": "2025.10",
        "resolutionOrder": [{"type": "set", "name": "theme", "sources": [embedded_tokens]}],
    }
    resolver_theme["tokenInput"] = {}
    for case in theme_cases:
        check_case(theme_case_schema, "theme source case", case, True)
        if case["name"] in theme_case_names:
            raise ValueError(f"duplicate theme source case name: {case['name']}")
        theme_case_names.add(case["name"])
        base = resolver_theme if case.get("sourceVariant") == "resolverBackedInline" else theme_source
        base_text = json.dumps(base, ensure_ascii=False) if base is resolver_theme else theme_source_text
        if "sourceReplace" in case:
            replacement = case["sourceReplace"]
            if base_text.count(replacement["find"]) != 1:
                raise ValueError(f"theme source replacement must match exactly once: {case['name']}")
            source = base_text.replace(replacement["find"], replacement["with"], 1)
            try:
                document = parse_json(source, case["name"])
            except ValueError:
                if case["schemaValid"]:
                    raise AssertionError(f"{case['name']}: source unexpectedly failed to parse")
                continue
        else:
            document = apply_changes(base, case.get("sourceChanges", []))
        check_case(theme_schema, f"theme source: {case['name']}", document, case["schemaValid"])

    check_case(theme_schema, "resolver-backed theme source", resolver_theme, True)
    for name in ("light", "dark"):
        authored = load_json(ROOT / "tokens" / "themes" / f"{name}.json")
        check_case(theme_schema, f"authored {name} theme", authored, True)
    frost_cases = load_json(ROOT / "conformance/materials/frost-legibility-vectors.json")
    frost_case_validator = validator_for("schemas/frost-legibility-case.schema.json")
    frost_request_validator = validator_for("schemas/frost-legibility-request.schema.json")
    frost_names = set()
    for case in frost_cases:
        check_case(frost_case_validator, f"Frost legibility case: {case.get('name')}", case, True)
        if case["name"] in frost_names:
            raise ValueError(f"duplicate Frost legibility case: {case['name']}")
        frost_names.add(case["name"])
        check_case(
            frost_request_validator,
            f"Frost legibility request: {case['name']}",
            case["request"],
            case["requestSchemaValid"],
        )
        if "expected" in case and not case["requestSchemaValid"]:
            raise ValueError(f"successful Frost case has invalid request: {case['name']}")
    frost_result_validator = validator_for("schemas/frost-legibility-result.schema.json")
    invalid_fallback_result = copy.deepcopy(frost_cases[0]["expected"])
    invalid_fallback_result["fallbackApplied"] = True
    check_case(frost_result_validator, "translucent result cannot claim opaque fallback", invalid_fallback_result, False)
    edge_cases = load_json(ROOT / "conformance/color/edge-contrast-vectors.json")
    edge_case_validator = validator_for("schemas/edge-contrast-case.schema.json")
    edge_request_validator = validator_for("schemas/edge-contrast-request.schema.json")
    edge_names = set()
    for case in edge_cases:
        check_case(edge_case_validator, f"edge contrast case: {case.get('name')}", case, True)
        if case["name"] in edge_names:
            raise ValueError(f"duplicate edge contrast case: {case['name']}")
        edge_names.add(case["name"])
        check_case(
            edge_request_validator,
            f"edge contrast request: {case['name']}",
            case["request"],
            case["requestSchemaValid"],
        )
        if "expected" in case and not case["requestSchemaValid"]:
            raise ValueError(f"successful edge case has invalid request: {case['name']}")
    edge_result_validator = validator_for("schemas/edge-contrast-result.schema.json")
    invalid_edge_result = copy.deepcopy(edge_cases[0]["expected"])
    invalid_edge_result["fallbackApplied"] = True
    check_case(edge_result_validator, "normal edge cannot claim strong fallback", invalid_edge_result, False)
    invalid_strong_result = copy.deepcopy(edge_cases[1]["expected"])
    invalid_strong_result["fallbackApplied"] = False
    check_case(edge_result_validator, "strong edge must report fallback", invalid_strong_result, False)
    readability_resolution = load_json(ROOT / "conformance/headless/valid-request.json")
    readability_resolution["colorAssignments"]["roles"]["content.primary"] = "palette.opaqueAlt"
    readability_resolution["opaqueColorAssignments"]["roles"]["outline.strong"] = "palette.opaqueAlt"
    readability_surface = copy.deepcopy(load_json(ROOT / "conformance/surfaces/binding-vectors.json")[0]["document"])
    readability_surface["states"]["states"] = ["rest"]
    readability_surface["treatmentStack"]["treatments"] = ["none"]
    readability_request = {
        "schemaVersion": "0.1.0",
        "scenario": {"schemaVersion": "0.4.0", "resolution": readability_resolution, "surface": readability_surface},
        "foregroundRole": "content.primary",
        "postTreatmentBackdrop": {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1},
        "adjacentColor": {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1},
        "minimumContentContrast": 3,
        "minimumEdgeContrast": 3,
    }
    readability_cases = load_json(ROOT / "conformance/surfaces/frost-readability-cases.json")
    readability_case_validator = validator_for("schemas/frost-surface-readability-case.schema.json")
    readability_request_validator = validator_for("schemas/frost-surface-readability-request.schema.json")
    readability_names = set()
    for case in readability_cases:
        check_case(readability_case_validator, f"Frost surface readability case: {case.get('name')}", case, True)
        if case["name"] in readability_names:
            raise ValueError(f"duplicate Frost surface readability case: {case['name']}")
        readability_names.add(case["name"])
        request = apply_changes(readability_request, case["changes"])
        check_case(
            readability_request_validator,
            f"Frost surface readability request: {case['name']}",
            request,
            case["requestSchemaValid"],
        )
    surface_readability_request = copy.deepcopy(readability_request)
    surface_readability_request["scenario"]["resolution"]["opaqueColorAssignments"]["roles"][
        "content.primary"
    ] = "palette.opaqueAlt"
    surface_readability_cases = load_json(ROOT / "conformance/surfaces/readability-cases.json")
    surface_readability_case_schema = validator_for("schemas/surface-readability-case.schema.json")
    surface_readability_request_schema = validator_for("schemas/surface-readability-request.schema.json")
    surface_readability_names = set()
    for case in surface_readability_cases:
        name = case["name"]
        check_case(surface_readability_case_schema, f"surface readability case: {name}", case, True)
        if name in surface_readability_names:
            raise ValueError(f"duplicate surface readability case: {name}")
        surface_readability_names.add(name)
        request = apply_changes(surface_readability_request, case["changes"])
        if case.get("omitBackdrop", False):
            del request["postTreatmentBackdrop"]
        check_case(
            surface_readability_request_schema,
            f"surface readability request: {name}",
            request,
            case["requestSchemaValid"],
        )
    surface_readability_result_schema = validator_for("schemas/surface-readability-result.schema.json")
    sample = {
        "schemaVersion": "0.1.0",
        "binding": load_json(ROOT / "conformance/surfaces/binding-vectors.json")[0]["expected"],
        "foregroundRole": "content.primary",
        "foreground": {"colorSpace": "srgb", "components": [0.8, 0.7, 0.6], "alpha": 1},
        "body": {"colorSpace": "srgb", "components": [0.15, 0.2, 0.3], "alpha": 1},
        "compositedBody": {"colorSpace": "srgb", "components": [0.15, 0.2, 0.3], "alpha": 1},
        "contentContrastRatio": 5,
        "contentFallbackApplied": True,
        "edge": load_json(ROOT / "conformance/color/edge-contrast-vectors.json")[0]["expected"],
        "frostRepresentation": "opaqueDimensional",
    }
    check_case(surface_readability_result_schema, "Frost readability result shape", sample, True)
    translucent_sample = copy.deepcopy(sample)
    translucent_sample["frostRepresentation"] = "translucentPigmented"
    translucent_sample["body"]["alpha"] = 0.55
    translucent_sample["contentFallbackApplied"] = False
    check_case(surface_readability_result_schema, "translucent Frost result shape", translucent_sample, True)
    translucent_sample["contentFallbackApplied"] = True
    check_case(
        surface_readability_result_schema,
        "Frost fallback requires opaque representation",
        translucent_sample,
        False,
    )
    frost_without_representation = copy.deepcopy(sample)
    del frost_without_representation["frostRepresentation"]
    check_case(surface_readability_result_schema, "Frost representation required", frost_without_representation, False)
    opaque_sample = copy.deepcopy(sample)
    opaque_sample["binding"] = load_json(ROOT / "conformance/surfaces/binding-vectors.json")[1]["expected"]
    opaque_sample["contentFallbackApplied"] = False
    del opaque_sample["frostRepresentation"]
    check_case(surface_readability_result_schema, "opaque readability result shape", opaque_sample, True)
    opaque_sample["frostRepresentation"] = "opaqueDimensional"
    check_case(surface_readability_result_schema, "non-Frost representation rejected", opaque_sample, False)
    focus_binding = load_json(ROOT / "conformance/surfaces/binding-vectors.json")[0]
    focus_request = {
        "schemaVersion": "0.1.0",
        "scenario": {
            "schemaVersion": "0.4.0",
            "resolution": load_json(ROOT / "conformance/headless/valid-request.json"),
            "surface": focus_binding["document"],
        },
        "surroundingColor": {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1},
    }
    focus_cases = load_json(ROOT / "conformance/states/focus-indicator-cases.json")
    focus_case_validator = validator_for("schemas/focus-indicator-case.schema.json")
    focus_request_validator = validator_for("schemas/focus-indicator-request.schema.json")
    focus_names = set()
    for case in focus_cases:
        check_case(focus_case_validator, f"focus indicator case: {case.get('name')}", case, True)
        if case["name"] in focus_names:
            raise ValueError(f"duplicate focus indicator case: {case['name']}")
        focus_names.add(case["name"])
        request = apply_changes(focus_request, case["changes"])
        check_case(
            focus_request_validator,
            f"focus indicator request: {case['name']}",
            request,
            case["requestSchemaValid"],
        )
    for name, expected in (
        (
            "invalid expected states",
            {"colorRole": "focus", "fallbackApplied": False, "states": ["focused", "unknown"]},
        ),
        ("inconsistent fallback role", {"colorRole": "focus", "fallbackApplied": True}),
    ):
        invalid = copy.deepcopy(focus_cases[0])
        invalid["expected"] = expected
        check_case(focus_case_validator, f"focus indicator case {name}", invalid, False)
    focus_result_validator = validator_for("schemas/focus-indicator-result.schema.json")
    focus_result = {
        "schemaVersion": "0.1.0",
        "binding": focus_binding["expected"],
        "colorRole": "focus",
        "color": {"colorSpace": "srgb", "components": [0.15, 0.2, 0.3], "alpha": 1},
        "contrastRatio": 16,
        "fallbackApplied": False,
        "strokeWidth": 2,
        "gap": 2,
    }
    check_case(focus_result_validator, "focus indicator result", focus_result, True)
    for name, field, value in (
        ("nonfocus state", ("binding", "states", "states"), ["rest"]),
        ("translucent color", ("color", "alpha"), 0.5),
        ("weak contrast", ("contrastRatio",), 2.99),
        ("narrow stroke", ("strokeWidth",), 1),
        ("incorrect fallback role", ("fallbackApplied",), True),
    ):
        invalid = copy.deepcopy(focus_result)
        target = invalid
        for key in field[:-1]:
            target = target[key]
        target[field[-1]] = value
        check_case(focus_result_validator, f"focus indicator result {name}", invalid, False)
    missing_input = copy.deepcopy(resolver_theme)
    del missing_input["tokenInput"]
    check_case(theme_schema, "resolver-backed theme missing input", missing_input, False)
    ambiguous_source = copy.deepcopy(resolver_theme)
    ambiguous_source["tokens"] = embedded_tokens
    check_case(theme_schema, "ambiguous theme token source", ambiguous_source, False)
    invalid_input = copy.deepcopy(resolver_theme)
    invalid_input["tokenInput"] = {"theme": True}
    check_case(theme_schema, "non-string resolver input", invalid_input, False)

    backend_case_schema = validator_for("schemas/headless-conformance-case.schema.json")
    backend_cases = load_json(ROOT / "conformance/headless/backend-cases.json")
    names = set()
    for case in backend_cases:
        check_case(backend_case_schema, "headless backend case", case, True)
        name = case["name"]
        if name in names:
            raise ValueError(f"duplicate headless backend case name: {name}")
        names.add(name)

    surface_backend_cases = load_json(ROOT / "conformance/surfaces/scenario-cases.json")
    names = set()
    for case in surface_backend_cases:
        check_case(backend_case_schema, "surface backend case", case, True)
        name = case["name"]
        if name in names:
            raise ValueError(f"duplicate surface backend case name: {name}")
        names.add(name)

    material_schema = load_json(ROOT / "schemas/material-assignments.schema.json")
    color_schema = load_json(ROOT / "schemas/color-assignments.schema.json")
    opaque_color_schema = load_json(ROOT / "schemas/opaque-color-assignments.schema.json")
    headless_result_schema = load_json(ROOT / "schemas/headless-result.schema.json")
    binding_schema = load_json(ROOT / "schemas/surface-binding.schema.json")
    binding_result_schema = load_json(ROOT / "schemas/surface-binding-result.schema.json")
    material_roles = {
        f"{group}.{role}"
        for group in ("surface", "control", "feedback")
        for role in material_schema["properties"][group]["properties"]
    }
    structural_roles = {
        f"{group}.{role}"
        for group in ("surface", "control", "feedback")
        for role, rule in material_schema["properties"][group]["properties"].items()
        if rule["$ref"] == "#/$defs/structuralMaterialFamily"
    }
    color_roles = set(color_schema["properties"]["roles"]["properties"])
    if set(opaque_color_schema["properties"]["roles"]["propertyNames"]["enum"]) != color_roles:
        raise ValueError("opaque fallback roles differ from semantic color roles")
    if set(headless_result_schema["properties"]["opaqueColorFallbacks"]["required"]) != color_roles:
        raise ValueError("resolved opaque fallback roles differ from semantic color roles")
    if set(headless_result_schema["properties"]["opaqueColorFallbacks"]["propertyNames"]["enum"]) != color_roles:
        raise ValueError("resolved opaque fallback property names differ from semantic color roles")
    if set(binding_schema["properties"]["materialRole"]["enum"]) != material_roles:
        raise ValueError("surface binding material roles differ from assignments")
    if set(binding_schema["properties"]["colorRole"]["enum"]) != color_roles:
        raise ValueError("surface binding color roles differ from assignments")
    bound_structural_roles = set(
        binding_result_schema["allOf"][0]["if"]["properties"]["materialRole"]["enum"]
    )
    if bound_structural_roles != structural_roles:
        raise ValueError("surface binding structural roles differ from assignments")
    if set(binding_result_schema["properties"]["materialFamily"]["enum"]) != set(
        material_schema["$defs"]["materialFamily"]["enum"]
    ):
        raise ValueError("surface binding material families differ from assignments")

    checked = (
        len(spring_cases) + 4
        + len(phase_cases) + 5 + 36 * 6
        + len(invalid_material_scenes) + 2
        + len(focus_ir_cases) + len(invalid_focus_ir_results)
        + len(focus_paint_vectors) + 3
        + len(ir_cases) + len(invalid_ir_results) + 3
        + len(backend_cases)
        + len(surface_backend_cases)
        + len(theme_cases)
        + len(theme_resolution_cases)
        + len(resolver_cases)
        + len(frost_cases)
        + len(edge_cases)
        + len(readability_cases)
        + len(surface_readability_cases)
        + len(pigment_profile_vectors)
        + len(pigment_cases)
        + len(contour_cases)
        + len(key_light_cases)
        + len(extrusion_cases)
        + len(invalid_extrusion_results)
        + 1
        + 1
        + len(focus_cases)
        + 6
        + 8
        + 2
    )
    for schema, vectors in (
        ("schemas/color-assignments.schema.json", "conformance/color/role-assignment-vectors.json"),
        ("schemas/opaque-color-assignments.schema.json", "conformance/color/opaque-assignment-vectors.json"),
        ("schemas/material-assignments.schema.json", "conformance/materials/role-assignment-vectors.json"),
        ("schemas/frost-pigment.schema.json", "conformance/materials/frost-pigment-vectors.json"),
        ("schemas/elevation-depth-assignments.schema.json", "conformance/elevation/depth-assignment-vectors.json"),
        ("schemas/spatial-assignments.schema.json", "conformance/spatial/assignment-vectors.json"),
        ("schemas/state-set.schema.json", "conformance/states/state-set-vectors.json"),
        ("schemas/surface-form.schema.json", "conformance/geometry/surface-form-vectors.json"),
        ("schemas/surface-binding.schema.json", "conformance/surfaces/binding-vectors.json"),
        ("schemas/treatment-stack.schema.json", "conformance/materials/treatment-stack-vectors.json"),
        ("schemas/typography-assignments.schema.json", "conformance/typography/assignment-vectors.json"),
    ):
        checked += check_vectors(schema, vectors)

    corner_case_schema = validator_for("schemas/corner-radius-case.schema.json")
    corner_size_schema = validator_for("schemas/surface-size.schema.json")
    corner_radii_schema = validator_for("schemas/logical-corner-radii.schema.json")
    corner_cases = load_json(ROOT / "conformance/geometry/corner-radius-vectors.json")
    corner_names = set()
    for case in corner_cases:
        name = case["name"]
        if name in corner_names:
            raise ValueError(f"duplicate corner radius case: {name}")
        corner_names.add(name)
        check_case(corner_case_schema, f"corner radius case: {name}", case, True)
        invalid_at = case.get("invalidAt")
        check_case(
            corner_size_schema, f"corner radius size: {name}", case["size"], invalid_at != "size"
        )
        check_case(
            corner_radii_schema,
            f"corner radius input: {name}",
            case["radii"],
            invalid_at != "radii",
        )
        if "expected" in case:
            check_case(
                corner_radii_schema, f"corner radius result: {name}", case["expected"], True
            )
        else:
            swapped = {**case, "invalidAt": "radii" if invalid_at == "size" else "size"}
            check_case(
                corner_case_schema, f"corner radius mislabeled invalid case: {name}", swapped, False
            )
        checked += 1

    shape_assignments_schema = validator_for("schemas/shape-fallback-assignments.schema.json")
    shape_assignment_case_schema = validator_for("schemas/shape-fallback-assignment-case.schema.json")
    shape_case_schema = validator_for("schemas/shape-fallback-case.schema.json")
    check_case(
        shape_assignments_schema,
        "authored Tier 0 shape fallbacks",
        load_json(ROOT / "definitions/tier0-shapes.json"),
        True,
    )
    checked += 1
    assignment_cases = load_json(
        ROOT / "conformance/geometry/shape-fallback-assignment-vectors.json"
    )
    assignment_names = set()
    for case in assignment_cases:
        name = case["name"]
        if name in assignment_names:
            raise ValueError(f"duplicate shape fallback assignment case: {name}")
        assignment_names.add(name)
        check_case(shape_assignment_case_schema, f"shape assignment case: {name}", case, True)
        check_case(
            shape_assignments_schema,
            f"shape assignment document: {name}",
            case["document"],
            "expected" in case,
        )
        checked += 1
    shape_cases = load_json(ROOT / "conformance/geometry/shape-fallback-vectors.json")
    shape_names = set()
    for case in shape_cases:
        name = case["name"]
        if name in shape_names:
            raise ValueError(f"duplicate shape fallback case: {name}")
        shape_names.add(name)
        check_case(shape_case_schema, f"shape fallback case: {name}", case, True)
        checked += 1
    shape_request_schema = validator_for("schemas/shape-fallback-request.schema.json")
    shape_result_schema = validator_for("schemas/shape-fallback-result.schema.json")
    shape_request = {
        "schemaVersion": "0.1.0",
        "tokens": load_json(ROOT / "tokens/foundation.json"),
        "assignments": load_json(ROOT / "definitions/tier0-shapes.json"),
        "shape": "structural",
        "size": {"width": 200, "height": 80},
    }
    for case in shape_cases:
        request = {**shape_request, "shape": case["shape"], "size": case["size"]}
        result = {"schemaVersion": "0.1.0", "radii": case["expected"]}
        check_case(shape_request_schema, f"shape request: {case['name']}", request, True)
        check_case(shape_result_schema, f"shape result: {case['name']}", result, True)
        checked += 2
    check_case(
        shape_request_schema,
        "shape request invalid name",
        {**shape_request, "shape": "pill"},
        False,
    )
    checked += 1

    elevation_assignments = validator_for("schemas/elevation-depth-assignments.schema.json")
    for vector in load_json(ROOT / "conformance/elevation/depth-resolution-vectors.json"):
        check_case(
            elevation_assignments,
            f"elevation depth resolution: {vector['name']}",
            vector["assignments"],
            True,
        )
        checked += 1

    elevation_cases = load_json(ROOT / "conformance/elevation/backend-cases.json")
    elevation_case_schema = validator_for("schemas/elevation-depth-case.schema.json")
    elevation_request_schema = validator_for("schemas/elevation-depth-request.schema.json")
    elevation_result_schema = validator_for("schemas/elevation-depth-result.schema.json")
    elevation_names = set()
    for case in elevation_cases:
        name = case["name"]
        if name in elevation_names:
            raise ValueError(f"duplicate elevation depth backend case: {name}")
        elevation_names.add(name)
        check_case(elevation_case_schema, f"elevation depth case: {name}", case, True)
        check_case(
            elevation_request_schema,
            f"elevation depth request: {name}",
            case["request"],
            case["requestSchemaValid"],
        )
        if "expected" in case:
            check_case(elevation_result_schema, f"elevation depth result: {name}", case["expected"], True)
        checked += 1

    composition_validator = validator_for("schemas/state-composition.schema.json")
    state_set_validator = validator_for("schemas/state-set.schema.json")
    composition_vectors = load_json(ROOT / "conformance/states/composition-vectors.json")
    composition_names = set()
    for vector in composition_vectors:
        if vector["name"] in composition_names:
            raise ValueError(f"duplicate state composition vector: {vector['name']}")
        composition_names.add(vector["name"])
        check_case(state_set_validator, f"state composition input: {vector['name']}", vector["states"], True)
        check_case(composition_validator, f"state composition: {vector['name']}", vector["expected"], True)
        checked += 2
    for field, value in (
        ("validation", ["focused"]),
        ("activity", ["busy", "busy"]),
        ("schemaVersion", "0.2.0"),
        ("validation", ["warning", "error"]),
        ("selection", ["selected", "active"]),
        ("interaction", ["pressed", "hover"]),
    ):
        invalid_composition = copy.deepcopy(composition_vectors[0]["expected"])
        invalid_composition[field] = value
        check_case(composition_validator, f"invalid state composition: {field}", invalid_composition, False)
        checked += 1
    empty_composition = copy.deepcopy(composition_vectors[0]["expected"])
    empty_composition["base"] = []
    check_case(composition_validator, "invalid state composition: empty", empty_composition, False)
    checked += 1

    surface_results = validator_for("schemas/surface-binding-result.schema.json")
    surface_vectors = load_json(ROOT / "conformance/surfaces/binding-vectors.json")
    for vector in surface_vectors:
        if "expected" in vector:
            check_case(surface_results, f"surface result: {vector['name']}", vector["expected"], True)
            checked += 1
    for name, source_index, change in (
        ("Frost representation omitted", 0, lambda result: result.pop("frostRepresentation")),
        ("Frost portable body omitted", 0, lambda result: result.pop("frostPortableBody")),
        ("Frost portable body fully opaque", 0, lambda result: result["frostPortableBody"].update({"alpha": 1.0})),
        ("Frost portable body invisible", 0, lambda result: result["frostPortableBody"].update({"alpha": 0.0})),
        (
            "Frost representation on Elastomer",
            1,
            lambda result: result.update({"frostRepresentation": "opaqueDimensional"}),
        ),
        ("Gel as structural control", 1, lambda result: result.update({"materialFamily": "gel"})),
        ("opaque fallback missing", 0, lambda result: result.pop("opaqueColorFallback")),
        ("opaque fallback translucent", 0, lambda result: result["opaqueColorFallback"].update({"alpha": 0.5})),
        ("Frost portable body on Elastomer", 1, lambda result: result.update({"frostPortableBody": surface_vectors[0]["expected"]["frostPortableBody"]})),
    ):
        result = copy.deepcopy(surface_vectors[source_index]["expected"])
        change(result)
        check_case(surface_results, f"surface result: {name}", result, False)
        checked += 1

    archived_binding = copy.deepcopy(surface_vectors[0]["document"])
    archived_binding["schemaVersion"] = "0.1.0"
    archived_binding.pop("treatmentStack")
    check_case(
        validator_for("schemas/versions/surface-binding-0.1.0.schema.json"),
        "archived surface binding",
        archived_binding,
        True,
    )
    check_case(validator_for("schemas/surface-binding.schema.json"), "current binding rejects archive", archived_binding, False)
    archived_surface = copy.deepcopy(surface_vectors[0]["expected"])
    archived_surface["schemaVersion"] = "0.1.0"
    archived_surface.pop("treatmentStack")
    archived_surface.pop("opaqueColorFallback")
    archived_surface.pop("frostPortableBody")
    check_case(
        validator_for("schemas/versions/surface-binding-result-0.1.0.schema.json"),
        "archived surface result",
        archived_surface,
        True,
    )
    check_case(surface_results, "current surface result rejects archive", archived_surface, False)
    checked += 4

    previous_surface = copy.deepcopy(surface_vectors[0]["expected"])
    previous_surface["schemaVersion"] = "0.2.0"
    previous_surface.pop("opaqueColorFallback")
    previous_surface.pop("frostPortableBody")
    check_case(
        validator_for("schemas/versions/surface-binding-result-0.2.0.schema.json"),
        "surface result 0.2.0 archive",
        previous_surface,
        True,
    )
    check_case(surface_results, "current surface result rejects 0.2.0 archive", previous_surface, False)
    checked += 2

    previous_surface = copy.deepcopy(surface_vectors[0]["expected"])
    previous_surface["schemaVersion"] = "0.3.0"
    previous_surface.pop("frostPortableBody")
    check_case(
        validator_for("schemas/versions/surface-binding-result-0.3.0.schema.json"),
        "surface result 0.3.0 archive",
        previous_surface,
        True,
    )
    check_case(surface_results, "current surface result rejects 0.3.0 archive", previous_surface, False)
    checked += 2

    headless = validator_for("schemas/headless-resolution.schema.json")
    valid_headless = load_json(ROOT / "conformance/headless/valid-request.json")
    check_case(headless, "headless valid request", valid_headless, True)
    checked += 1
    previous_headless = copy.deepcopy(valid_headless)
    previous_headless["schemaVersion"] = "0.1.0"
    previous_headless.pop("opaqueColorAssignments")
    previous_headless.pop("frostPigment")
    check_case(
        validator_for("schemas/versions/headless-resolution-0.1.0.schema.json"),
        "headless request 0.1.0 archive",
        previous_headless,
        True,
    )
    check_case(headless, "current headless request rejects archive", previous_headless, False)
    checked += 2
    previous_headless_0_2 = copy.deepcopy(valid_headless)
    previous_headless_0_2["schemaVersion"] = "0.2.0"
    previous_headless_0_2.pop("frostPigment")
    check_case(
        validator_for("schemas/versions/headless-resolution-0.2.0.schema.json"),
        "headless request 0.2.0 archive",
        previous_headless_0_2,
        True,
    )
    check_case(headless, "current headless request rejects 0.2.0 archive", previous_headless_0_2, False)
    checked += 2
    for name, change in (
        ("unknown member", lambda document: document.update({"rendererName": "example"})),
        ("missing input", lambda document: document.pop("environment")),
        ("nested role", lambda document: document["colorAssignments"]["roles"].pop("focus")),
        ("missing opaque assignments", lambda document: document.pop("opaqueColorAssignments")),
        ("missing Frost pigment", lambda document: document.pop("frostPigment")),
    ):
        document = copy.deepcopy(valid_headless)
        change(document)
        check_case(headless, f"headless {name}", document, False)
        checked += 1

    scenario_schema = validator_for("schemas/surface-scenario.schema.json")
    for vector in surface_vectors:
        scenario = {
            "schemaVersion": "0.4.0",
            "resolution": valid_headless,
            "surface": vector["document"],
        }
        check_case(
            scenario_schema,
            f"surface scenario: {vector['name']}",
            scenario,
            "expected" in vector,
        )
        checked += 1
    valid_scenario = {
        "schemaVersion": "0.4.0",
        "resolution": valid_headless,
        "surface": surface_vectors[0]["document"],
    }
    previous_scenario = {
        "schemaVersion": "0.1.0",
        "resolution": previous_headless,
        "surface": archived_binding,
    }
    check_case(
        validator_for("schemas/versions/surface-scenario-0.1.0.schema.json"),
        "archived surface scenario",
        previous_scenario,
        True,
    )
    check_case(scenario_schema, "current scenario rejects archive", previous_scenario, False)
    checked += 2
    previous_scenario = {
        "schemaVersion": "0.3.0",
        "resolution": previous_headless_0_2,
        "surface": surface_vectors[0]["document"],
    }
    check_case(
        validator_for("schemas/versions/surface-scenario-0.3.0.schema.json"),
        "surface scenario 0.3.0 archive",
        previous_scenario,
        True,
    )
    check_case(scenario_schema, "current scenario rejects 0.3.0 archive", previous_scenario, False)
    checked += 2
    previous_scenario = {
        "schemaVersion": "0.2.0",
        "resolution": previous_headless,
        "surface": surface_vectors[0]["document"],
    }
    check_case(
        validator_for("schemas/versions/surface-scenario-0.2.0.schema.json"),
        "surface scenario 0.2.0 archive",
        previous_scenario,
        True,
    )
    check_case(scenario_schema, "current scenario rejects 0.2.0 archive", previous_scenario, False)
    checked += 2
    for name, change in (
        ("unsupported version", lambda document: document.update({"schemaVersion": "0.1.0"})),
        ("missing surface", lambda document: document.pop("surface")),
        ("unknown member", lambda document: document.update({"backend": "example"})),
        (
            "invalid resolution",
            lambda document: document["resolution"]["colorAssignments"]["roles"].pop("focus"),
        ),
    ):
        document = copy.deepcopy(valid_scenario)
        change(document)
        check_case(scenario_schema, f"surface scenario: {name}", document, False)
        checked += 1

    result_schema = validator_for("schemas/headless-result.schema.json")
    expected_result = load_json(ROOT / "conformance/headless/expected-resolution.json")
    check_case(result_schema, "headless result", expected_result, True)
    checked += 1
    for name, path, value in (
        ("version", ("schemaVersion",), "0.1.0"),
        ("structural gel", ("materials", "surface.base"), "gel"),
        ("color range", ("colors", "focus", "components", 0), 1.5),
        ("negative space", ("space", "space.page", "value"), -1),
        ("font unit", ("typography", "body", "fontSize", "unit"), "rem"),
        ("weight range", ("typography", "body", "fontWeight"), 1001),
        ("hit target pair", ("minimumHitTarget", "minimumHeight"), 24),
        ("unknown member", ("rendererName",), "example"),
        ("fallback channel", ("colorFallbacks", "focus", "components", 0), 1.5),
        ("fallback space", ("colorFallbacks", "focus", "colorSpace"), "display-p3"),
        ("opaque fallback alpha", ("opaqueColorFallbacks", "focus", "alpha"), 0.5),
        ("opaque fallback channel", ("opaqueColorFallbacks", "focus", "components", 0), 1.5),
        ("Frost tint strength", ("frostTintStrength",), 1.0),
        (
            "unknown fallback role",
            ("colorFallbacks", "unknown"),
            expected_result["colorFallbacks"]["focus"],
        ),
        (
            "unknown opaque fallback role",
            ("opaqueColorFallbacks", "unknown"),
            expected_result["opaqueColorFallbacks"]["focus"],
        ),
    ):
        document = copy.deepcopy(expected_result)
        target = document
        for key in path[:-1]:
            target = target[key]
        target[path[-1]] = value
        check_case(result_schema, f"headless result {name}", document, False)
        checked += 1
    missing_color = copy.deepcopy(expected_result)
    missing_color["colors"].pop("focus")
    check_case(result_schema, "headless result missing color", missing_color, False)
    checked += 1

    missing_fallback = copy.deepcopy(expected_result)
    missing_fallback["colorFallbacks"].pop("focus")
    check_case(result_schema, "headless result missing fallback role", missing_fallback, False)
    checked += 1

    missing_opaque_fallback = copy.deepcopy(expected_result)
    missing_opaque_fallback["opaqueColorFallbacks"].pop("focus")
    check_case(result_schema, "headless result missing opaque fallback role", missing_opaque_fallback, False)
    checked += 1

    previous_result = validator_for("schemas/versions/headless-result-0.1.0.schema.json")
    archived_result = copy.deepcopy(expected_result)
    archived_result["schemaVersion"] = "0.1.0"
    archived_result.pop("colorFallbacks")
    archived_result.pop("opaqueColorFallbacks")
    archived_result.pop("frostTintStrength")
    check_case(previous_result, "headless result 0.1.0 archive", archived_result, True)
    check_case(result_schema, "current result rejects 0.1.0 archive", archived_result, False)
    checked += 2

    previous_result = validator_for("schemas/versions/headless-result-0.2.0.schema.json")
    archived_result = copy.deepcopy(expected_result)
    archived_result["schemaVersion"] = "0.2.0"
    archived_result.pop("opaqueColorFallbacks")
    archived_result.pop("frostTintStrength")
    check_case(previous_result, "headless result 0.2.0 archive", archived_result, True)
    check_case(result_schema, "current result rejects 0.2.0 archive", archived_result, False)
    checked += 2

    previous_result = validator_for("schemas/versions/headless-result-0.3.0.schema.json")
    archived_result = copy.deepcopy(expected_result)
    archived_result["schemaVersion"] = "0.3.0"
    archived_result.pop("frostTintStrength")
    check_case(previous_result, "headless result 0.3.0 archive", archived_result, True)
    check_case(result_schema, "current result rejects 0.3.0 archive", archived_result, False)
    checked += 2

    color_value = Draft202012Validator(
        {"$ref": "urn:resina:schema:headless-result:0.4.0#/$defs/color"},
        registry=schema_registry(),
    )
    for vector in load_json(ROOT / "conformance/tokens/primitive-value-vectors.json"):
        if vector["type"] == "color":
            check_case(color_value, vector["name"], vector["value"], "error" not in vector)
            document = copy.deepcopy(expected_result)
            document["colors"]["focus"] = vector["value"]
            check_case(result_schema, f"headless result: {vector['name']}", document, "error" not in vector)
            checked += 2
    for vector in load_json(ROOT / "conformance/tokens/color-space-vectors.json"):
        check_case(color_value, vector["name"], vector["value"], "error" not in vector)
        checked += 1
    for filename, component in (
        ("xyz-conversion-vectors.json", "xyz"),
        ("rgb-conversion-vectors.json", "rgb"),
        ("lab-conversion-vectors.json", "components"),
    ):
        for vector in load_json(ROOT / "conformance/color" / filename):
            source = {"colorSpace": vector["space"], "components": vector[component]}
            check_case(color_value, f"{filename}: {vector['name']}", source, "error" not in vector)
            checked += 1

    fallback_schema = validator_for("schemas/srgb-fallback.schema.json")
    for vector in load_json(ROOT / "conformance/color/srgb-decoding-vectors.json"):
        if ("linearSrgb" in vector) == ("error" in vector):
            raise ValueError(f"sRGB decoding vector needs one outcome: {vector['name']}")
        source = {"colorSpace": "srgb", "components": vector["srgb"]}
        check_case(color_value, f"sRGB decoding input: {vector['name']}", source, "error" not in vector)
        checked += 1
        if "linearSrgb" in vector:
            result = {"colorSpace": "srgb-linear", "components": vector["linearSrgb"]}
            check_case(color_value, f"sRGB decoding output: {vector['name']}", result, True)
            checked += 1
    for vector in load_json(ROOT / "conformance/color/srgb-fallback-vectors.json"):
        check_case(
            color_value,
            f"fallback input: {vector['name']}",
            vector["value"],
            vector.get("error") != "InvalidValue",
        )
        checked += 1
        if "expected" in vector:
            check_case(fallback_schema, f"fallback output: {vector['name']}", vector["expected"], True)
            checked += 1
    valid_fallback = load_json(ROOT / "conformance/color/srgb-fallback-vectors.json")[0]["expected"]
    for name, field, value in (
        ("component", "components", ["none", 0.5, 0.875]),
        ("alpha", "alpha", 1.1),
        ("space", "colorSpace", "display-p3"),
    ):
        invalid = copy.deepcopy(valid_fallback)
        invalid[field] = value
        check_case(fallback_schema, f"fallback invalid {name}", invalid, False)
        checked += 1

    opaque_fallback_schema = validator_for("schemas/opaque-srgb-fallback.schema.json")
    opaque_vectors = load_json(ROOT / "conformance/color/opaque-fallback-vectors.json")
    for vector in opaque_vectors:
        if ("expected" in vector) == ("error" in vector):
            raise ValueError(f"opaque fallback vector needs one outcome: {vector['name']}")
        check_case(color_value, f"opaque fallback source: {vector['name']}", vector["source"], True)
        checked += 1
        if "authored" in vector:
            check_case(color_value, f"authored opaque fallback: {vector['name']}", vector["authored"], True)
            checked += 1
        if "expected" in vector:
            check_case(
                opaque_fallback_schema,
                f"opaque fallback output: {vector['name']}",
                vector["expected"],
                True,
            )
            checked += 1
    translucent_result = copy.deepcopy(opaque_vectors[0]["expected"])
    translucent_result["alpha"] = 0.5
    check_case(opaque_fallback_schema, "opaque fallback rejects alpha below one", translucent_result, False)
    checked += 1

    duplicate_spatial = ROOT / "conformance/spatial/invalid-duplicate-role.json"
    try:
        load_json(duplicate_spatial)
    except ValueError as error:
        if "duplicate JSON member" not in str(error):
            raise
    else:
        raise AssertionError(f"{duplicate_spatial}: duplicate role was accepted")
    checked += 1

    duplicate_typography = ROOT / "conformance/typography/invalid-duplicate-role.json"
    try:
        load_json(duplicate_typography)
    except ValueError as error:
        if "duplicate JSON member" not in str(error):
            raise
    else:
        raise AssertionError(f"{duplicate_typography}: duplicate role was accepted")
    checked += 1

    previous_materials = validator_for("schemas/versions/material-assignments-0.1.0.schema.json")
    current_materials = validator_for("schemas/material-assignments.schema.json")
    legacy_materials = copy.deepcopy(
        load_json(ROOT / "conformance/materials/role-assignment-vectors.json")[0]["document"]
    )
    legacy_materials["schemaVersion"] = "0.1.0"
    legacy_materials["control"]["primary"] = "gel"
    check_case(previous_materials, "material assignments 0.1.0 archive", legacy_materials, True)
    check_case(current_materials, "current material assignments reject 0.1.0", legacy_materials, False)
    checked += 2
    legacy_materials["schemaVersion"] = "0.2.0"
    check_case(current_materials, "current material assignments reject structural gel", legacy_materials, False)
    checked += 1

    environment = validator_for("schemas/environment.schema.json")
    for filename, valid in (
        ("valid-mixed-input.json", True),
        ("valid-minimal-capabilities.json", True),
        ("invalid-duplicate-input.json", False),
    ):
        path = ROOT / "conformance" / "environment" / filename
        check_case(environment, str(path.relative_to(ROOT)), load_json(path), valid)
        checked += 1

    oversized = ROOT / "conformance/environment/invalid-nonfinite-number.json"
    try:
        load_json(oversized)
    except ValueError as error:
        if "exceeds finite range" not in str(error):
            raise
    else:
        raise AssertionError(f"{oversized}: non-finite number was accepted")
    checked += 1

    density_base = load_json(ROOT / "conformance/environment/valid-mixed-input.json")
    for vector in load_json(ROOT / "conformance/environment/density-vectors.json"):
        document = copy.deepcopy(density_base)
        for field in ("densityPreference", "viewingProfile", "inputCapabilities", "textScale"):
            document[field] = vector[field]
        check_case(environment, f"density: {vector['name']}", document, True)
        checked += 1

    for vector in load_json(ROOT / "conformance/interaction/target-minimum-vectors.json"):
        document = copy.deepcopy(density_base)
        for field in (
            "inputCapabilities",
            "densityPreference",
            "viewingProfile",
            "textScale",
            "qualityPolicy",
        ):
            document[field] = vector[field]
        check_case(environment, f"target minimum: {vector['name']}", document, True)
        checked += 1

    for vector in load_json(ROOT / "conformance/typography/resolution-vectors.json"):
        document = copy.deepcopy(density_base)
        document["textScale"] = vector["textScale"]
        check_case(environment, f"typography: {vector['name']}", document, True)
        checked += 1

    previous = validator_for("schemas/versions/environment-0.1.0.schema.json")
    current = load_json(ROOT / "conformance/environment/valid-mixed-input.json")
    missing_capability = copy.deepcopy(current)
    del missing_capability["rendererCapabilities"]["translucentSurfaces"]
    check_case(environment, "current environment requires translucency", missing_capability, False)
    checked += 1

    migrated = copy.deepcopy(current)
    migrated["schemaVersion"] = "0.1.0"
    del migrated["rendererCapabilities"]["translucentSurfaces"]
    check_case(previous, "environment 0.1.0 archive", migrated, True)
    check_case(environment, "current environment rejects 0.1.0", migrated, False)
    checked += 2

    from check_surface_paint_backend import baseline, case_request, expected_result
    paint_cases = load_json(ROOT / "conformance/ir/surface-paint-cases.json")
    paint_request_schema = validator_for("schemas/surface-paint-request.schema.json")
    paint_result_schema = validator_for("schemas/surface-paint-ir.schema.json")
    paint_case_schema = validator_for("schemas/surface-paint-case.schema.json")
    paint_names = set()
    for case in paint_cases:
        check_case(paint_case_schema, case["name"], case, True)
        if case["name"] in paint_names:
            raise ValueError(f"duplicate surface paint case: {case['name']}")
        paint_names.add(case["name"])
        request = case_request(baseline(), case)
        check_case(paint_request_schema, case["name"], request, case["requestSchemaValid"])
        checked += 1
        if "expectedFocus" in case:
            result = expected_result(request)
            if ("focus" in result) != case["expectedFocus"]:
                raise ValueError(f"{case['name']}: expected focus disagrees with request")
            check_case(paint_result_schema, case["name"], result, True)
            checked += 1

    from check_hit_region_backend import cases, validate_membership_vectors
    checked += validate_membership_vectors()
    hit_names = set()
    for case in cases():
        name = case["name"]
        check_case(validator_for("schemas/hit-region-case.schema.json"), name,
                   {key: value for key, value in case.items() if key != "request"}, True)
        if name in hit_names:
            raise ValueError(f"duplicate hit region case: {name}")
        hit_names.add(name)
        check_case(validator_for("schemas/hit-region-request.schema.json"), name,
                   case["request"], case["requestSchemaValid"])
        checked += 1
        if "expected" in case:
            check_case(validator_for("schemas/hit-region-ir.schema.json"), name, case["expected"], True)
            checked += 1

    from check_focus_traversal_backend import cases as focus_traversal_cases
    traversal_names = set()
    for case in focus_traversal_cases():
        name = case["name"]
        check_case(validator_for("schemas/focus-traversal-case.schema.json"), name, case, True)
        if name in traversal_names:
            raise ValueError(f"duplicate focus traversal case: {name}")
        traversal_names.add(name)
        check_case(validator_for("schemas/focus-traversal-request.schema.json"), name,
                   case["request"], case["requestSchemaValid"])
        checked += 1
        if "expected" in case:
            check_case(validator_for("schemas/focus-traversal-result.schema.json"), name, case["expected"], True)
            checked += 1

    from check_activation_backend import cases as activation_cases
    activation_names = set()
    for case in activation_cases():
        name = case["name"]
        check_case(validator_for("schemas/activation-case.schema.json"), name, case, True)
        if name in activation_names:
            raise ValueError(f"duplicate activation case: {name}")
        activation_names.add(name)
        check_case(validator_for("schemas/activation-request.schema.json"), name,
                   case["request"], case["requestSchemaValid"])
        checked += 1
        if "expected" in case:
            check_case(validator_for("schemas/activation-result.schema.json"), name, case["expected"], True)
            checked += 1

    activation_result_validator = validator_for("schemas/activation-result.schema.json")
    activation_results = {
        case["name"]: case["expected"]
        for case in activation_cases()
        if "expected" in case
    }
    for name, base, changes in [
        ("disabled activation", "pointer arm", [
            {"path": "/state/enabled", "value": False},
            {"path": "/state/hold", "value": None},
            {"path": "/pressed", "value": False},
            {"path": "/capture", "value": None},
            {"path": "/activate", "value": True},
        ]),
        ("acquisition without hold", "pointer arm", [
            {"path": "/state/hold", "value": None},
            {"path": "/pressed", "value": False},
        ]),
        ("acquisition with keyboard hold", "pointer arm", [
            {"path": "/state/hold", "value": {"kind": "key", "key": "space"}},
        ]),
        ("acquisition outside target", "pointer arm", [
            {"path": "/state/hold/inside", "value": False},
            {"path": "/pressed", "value": False},
        ]),
        ("acquisition activates", "pointer arm", [
            {"path": "/activate", "value": True},
        ]),
        ("release retains pointer hold", "pointer arm", [
            {"path": "/capture/kind", "value": "release"},
        ]),
        ("release retains keyboard hold", "pointer arm", [
            {"path": "/state/hold", "value": {"kind": "key", "key": "space"}},
            {"path": "/capture/kind", "value": "release"},
        ]),
    ]:
        check_case(activation_result_validator, name,
                   apply_changes(activation_results[base], changes), False)
        checked += 1

    for case in load_json(ROOT / "conformance/interaction/slider-value-cases.json"):
        name = case["name"]
        check_case(validator_for("schemas/slider-value-case.schema.json"), name, case, True)
        check_case(validator_for("schemas/slider-value-request.schema.json"), name,
                   case["request"], case["requestSchemaValid"])
        checked += 2
        if "expected" in case:
            check_case(validator_for("schemas/slider-value-ir.schema.json"), name,
                       case["expected"], True)
            checked += 1

    slider_adjustments = load_json(ROOT / "conformance/interaction/slider-adjustment-cases.json")
    check_case(validator_for("schemas/slider-adjustment-cases.schema.json"),
               "slider adjustment matrix", slider_adjustments, True)
    names = [case["name"] for case in slider_adjustments["cases"]]
    if len(names) != len(set(names)):
        raise ValueError("duplicate slider adjustment case")
    checked += 1 + len(names)
    adjustment_result = slider_adjustments["cases"][0]["expected"]
    adjustment_validator = validator_for("schemas/slider-adjustment-ir.schema.json")
    for name, changes in [
        ("rejected adjustment changed", [{"path": "/accepted", "value": False}]),
        ("unsupported adjustment version", [{"path": "/schemaVersion", "value": "0.2.0"}]),
        ("unbounded adjustment progress", [{"path": "/value/progress", "value": 2}]),
    ]:
        check_case(adjustment_validator, name, apply_changes(adjustment_result, changes), False)
        checked += 1
    check_case(adjustment_validator, "unknown adjustment result member",
               dict(adjustment_result, backend="native"), False)
    checked += 1
    for field in adjustment_result:
        incomplete = copy.deepcopy(adjustment_result)
        incomplete.pop(field)
        check_case(adjustment_validator, f"missing adjustment result {field}", incomplete, False)
        checked += 1

    toggle_names = set()
    for case in load_json(ROOT / "conformance/interaction/toggle-activation-cases.json"):
        name = case["name"]
        check_case(validator_for("schemas/toggle-activation-case.schema.json"), name, case, True)
        if name in toggle_names:
            raise ValueError(f"duplicate toggle activation case: {name}")
        toggle_names.add(name)
        check_case(validator_for("schemas/toggle-activation-request.schema.json"), name,
                   case["request"], case["requestSchemaValid"])
        checked += 1
        if "expected" in case:
            check_case(validator_for("schemas/toggle-activation-result.schema.json"), name, case["expected"], True)
            checked += 1

    toggle_state_names = set()
    for case in load_json(ROOT / "conformance/interaction/toggle-states-cases.json"):
        name = case["name"]
        check_case(validator_for("schemas/toggle-states-case.schema.json"), name, case, True)
        if name in toggle_state_names:
            raise ValueError(f"duplicate toggle state case: {name}")
        toggle_state_names.add(name)
        check_case(validator_for("schemas/toggle-states-request.schema.json"), name,
                   case["request"], case["requestSchemaValid"])
        checked += 1
        if "expected" in case:
            check_case(validator_for("schemas/state-set.schema.json"), name, case["expected"], True)
            checked += 1

    command_state_names = set()
    for case in load_json(ROOT / "conformance/interaction/command-states-cases.json"):
        name = case["name"]
        if name in command_state_names:
            raise ValueError(f"duplicate command state case: {name}")
        command_state_names.add(name)
        check_case(validator_for("schemas/command-states-case.schema.json"), name, case, True)
        check_case(validator_for("schemas/command-states-request.schema.json"), name,
                   case["request"], case["requestSchemaValid"])
        checked += 1
        if "expected" in case:
            check_case(validator_for("schemas/state-set.schema.json"), name, case["expected"], True)
            checked += 1

    checked += 2 + 2 * len(load_json(ROOT / "conformance/ir/command-paint-cases.json"))
    trajectory_names = set()
    for case in load_json(ROOT / "conformance/motion/spring-trajectory-cases.json"):
        name = case["name"]
        if name in trajectory_names:
            raise ValueError("duplicate trajectory case: " + name)
        trajectory_names.add(name)
        check_case(validator_for("schemas/spring-trajectory-case.schema.json"), name, case, True)
        check_case(validator_for("schemas/spring-trajectory-request.schema.json"), name,
                   case["request"], case["requestSchemaValid"])
        checked += 2
        if "expected" in case:
            check_case(validator_for("schemas/spring-trajectory-result.schema.json"), name, case["expected"], True)
            checked += 1
    motion_base = load_json(ROOT / "conformance/ir/command-motion-request.json")
    motion_names = set()
    check_case(validator_for("schemas/command-motion-request.schema.json"), "motion baseline", motion_base, True)
    checked += 1
    for case in load_json(ROOT / "conformance/ir/command-motion-cases.json"):
        if case["name"] in motion_names:
            raise ValueError("duplicate command motion case: " + case["name"])
        motion_names.add(case["name"])
        check_case(validator_for("schemas/command-motion-case.schema.json"), case["name"], case, True)
        check_case(validator_for("schemas/command-motion-request.schema.json"), case["name"],
                   apply_changes(motion_base, case["requestChanges"]), case["requestSchemaValid"])
        checked += 2
    label = load_json(ROOT / "conformance/ir/command-label-ir.json")
    label_validator = validator_for("schemas/command-label-ir.schema.json")
    check_case(label_validator, "complete label IR", label, True)
    checked += 1
    for name, changes in [
        ("empty label", [{"path": "/text", "value": ""}]),
        ("zero label width", [{"path": "/labelBounds/width", "value": 0}]),
        ("negative label origin", [{"path": "/labelBounds/x", "value": -1}]),
        ("invalid direction", [{"path": "/layoutDirection", "value": "auto"}]),
        ("unsupported label version", [{"path": "/schemaVersion", "value": "0.2.0"}]),
    ]:
        check_case(label_validator, name, apply_changes(label, changes), False)
        checked += 1
    extra = dict(label, renderer="native")
    check_case(label_validator, "renderer leakage", extra, False)
    checked += 1
    expansion = load_json(ROOT / "conformance/content/command-label-expansion.json")
    checked += check_label_expansion(expansion)
    accessibility_validator = validator_for("schemas/command-accessibility-ir.schema.json")
    accessibility_names = set()
    for case in load_json(ROOT / "conformance/accessibility/command-cases.json"):
        name = case["name"]
        if name in accessibility_names:
            raise ValueError(f"duplicate command accessibility case: {name}")
        accessibility_names.add(name)
        check_case(validator_for("schemas/command-accessibility-case.schema.json"), name, case, True)
        checked += 1
        if "expected" in case:
            check_case(accessibility_validator, name, case["expected"], True)
            checked += 1
    accessibility = load_json(ROOT / "conformance/accessibility/command-cases.json")[0]["expected"]
    for name, changes in [
        ("wrong role", [{"path": "/role", "value": "link"}]),
        ("empty name", [{"path": "/name", "value": ""}]),
        ("empty description", [{"path": "/description", "value": ""}]),
        ("invented value", [{"path": "/value", "value": 1}]),
        ("toggle state", [{"path": "/state", "value": {"enabled": True, "focused": False, "pressed": True}}]),
        ("missing invocation", [{"path": "/actions", "value": []}]),
        ("unknown action", [{"path": "/actions/0/kind", "value": "click"}]),
        ("unavailable enabled action", [{"path": "/actions/0/available", "value": False}]),
        ("available disabled action", [{"path": "/state/enabled", "value": False}]),
        ("enabled not focusable", [{"path": "/focusable", "value": False}]),
        ("focused not focusable", [{"path": "/state/enabled", "value": False},
                                  {"path": "/actions/0/available", "value": False},
                                  {"path": "/state/focused", "value": True},
                                  {"path": "/focusable", "value": False}]),
        ("unsupported relationship", [{"path": "/relationships", "value": ["external"]}]),
        ("unsupported version", [{"path": "/schemaVersion", "value": "0.2.0"}]),
    ]:
        check_case(accessibility_validator, name, apply_changes(accessibility, changes), False)
        checked += 1
    for field in accessibility:
        document = copy.deepcopy(accessibility)
        document.pop(field)
        check_case(accessibility_validator, f"missing accessibility {field}", document, False)
        checked += 1
    for field, value in [("pressed", True), ("backend", "native")]:
        document = dict(accessibility, **{field: value})
        check_case(accessibility_validator, f"unsupported accessibility {field}", document, False)
        checked += 1
    slider_accessibility_names = set()
    for case in load_json(ROOT / "conformance/accessibility/slider-cases.json"):
        name = case["name"]
        if name in slider_accessibility_names:
            raise ValueError(f"duplicate slider accessibility case: {name}")
        slider_accessibility_names.add(name)
        check_case(validator_for("schemas/slider-accessibility-case.schema.json"), name, case, True)
        checked += 1
        if "expected" in case:
            check_case(validator_for("schemas/slider-accessibility-ir.schema.json"),
                       name, case["expected"], True)
            checked += 1
    slider_semantics = load_json(ROOT / "conformance/accessibility/slider-cases.json")[0]["expected"]
    slider_semantics_validator = validator_for("schemas/slider-accessibility-ir.schema.json")
    for name, changes in [
        ("slider wrong role", [{"path": "/role", "value": "spinbutton"}]),
        ("slider empty name", [{"path": "/name", "value": ""}]),
        ("slider missing numeric value", [{"path": "/value", "value": None}]),
        ("slider unsupported orientation", [{"path": "/orientation", "value": "auto"}]),
        ("slider readonly available action", [{"path": "/state/readOnly", "value": True}]),
        ("slider disabled available action", [{"path": "/state/enabled", "value": False}]),
        ("slider writable unavailable action", [{"path": "/actions/1/available", "value": False}]),
        ("slider unknown action", [{"path": "/actions/0/kind", "value": "invoke"}]),
        ("slider duplicate action", [{"path": "/actions/2/kind", "value": "increase"}]),
        ("slider missing action", [{"path": "/actions", "value": slider_semantics["actions"][:2]}]),
        ("slider enabled not focusable", [{"path": "/focusable", "value": False}]),
        ("slider focused not focusable", [{"path": "/state/enabled", "value": False},
                                           {"path": "/state/focused", "value": True},
                                           {"path": "/focusable", "value": False},
                                           {"path": "/actions/0/available", "value": False},
                                           {"path": "/actions/1/available", "value": False},
                                           {"path": "/actions/2/available", "value": False}]),
        ("slider unsupported relationships", [{"path": "/relationships", "value": ["external"]}]),
        ("slider unsupported version", [{"path": "/schemaVersion", "value": "0.2.0"}]),
    ]:
        check_case(slider_semantics_validator, name, apply_changes(slider_semantics, changes), False)
        checked += 1
    for field in slider_semantics:
        incomplete = copy.deepcopy(slider_semantics)
        incomplete.pop(field)
        check_case(slider_semantics_validator, f"missing slider semantics {field}", incomplete, False)
        checked += 1
    check_case(slider_semantics_validator, "slider paint state leakage",
               dict(slider_semantics, pressed=True), False)
    checked += 1
    toggle_accessibility_names = set()
    for case in load_json(ROOT / "conformance/accessibility/toggle-cases.json"):
        name = case["name"]
        if name in toggle_accessibility_names:
            raise ValueError(f"duplicate toggle accessibility case: {name}")
        toggle_accessibility_names.add(name)
        check_case(validator_for("schemas/toggle-accessibility-case.schema.json"), name, case, True)
        checked += 1
        if "expected" in case:
            check_case(validator_for("schemas/toggle-accessibility-ir.schema.json"), name, case["expected"], True)
            checked += 1
    toggle_layout_names = set()
    for case in load_json(ROOT / "conformance/geometry/toggle-layout-cases.json"):
        name = case["name"]
        if name in toggle_layout_names:
            raise ValueError(f"duplicate toggle layout case: {name}")
        toggle_layout_names.add(name)
        check_case(validator_for("schemas/toggle-layout-case.schema.json"), name, case, True)
        check_case(validator_for("schemas/toggle-layout-request.schema.json"), name, case["request"], case["requestSchemaValid"])
        checked += 1
        if "expected" in case:
            check_case(validator_for("schemas/toggle-layout-ir.schema.json"), name, case["expected"], True)
            checked += 1
    slider_layout = load_json(ROOT / "conformance/geometry/slider-layout-cases.json")
    check_case(validator_for("schemas/slider-layout-cases.schema.json"),
               "slider layout matrix", slider_layout, True)
    names = [case["name"] for case in slider_layout["cases"]]
    if len(names) != len(set(names)):
        raise ValueError("duplicate slider layout case")
    checked += 1 + len(names)
    slider_geometry = slider_layout["cases"][0]["expected"]
    slider_geometry_validator = validator_for("schemas/slider-layout-ir.schema.json")
    for name, changes in [
        ("slider layout wrong version", [{"path": "/schemaVersion", "value": "0.2.0"}]),
        ("slider layout invalid axis", [{"path": "/orientation", "value": "auto"}]),
        ("slider layout invalid minimum position", [{"path": "/minimumPosition", "value": "top"}]),
        ("slider layout displaced allocation", [{"path": "/allocationBounds/x", "value": 1}]),
        ("slider layout empty track", [{"path": "/trackBounds/width", "value": 0}]),
        ("slider layout missing thumb extent", [{"path": "/thumbBounds/height", "value": 0}]),
        ("slider layout invalid progress", [{"path": "/value/progress", "value": -1}]),
    ]:
        check_case(slider_geometry_validator, name, apply_changes(slider_geometry, changes), False)
        checked += 1
    for field in slider_geometry:
        incomplete = copy.deepcopy(slider_geometry)
        incomplete.pop(field)
        check_case(slider_geometry_validator, f"missing slider layout {field}", incomplete, False)
        checked += 1
    check_case(slider_geometry_validator, "slider layout backend leakage",
               dict(slider_geometry, renderer="native"), False)
    checked += 1
    slider_position = load_json(ROOT / "conformance/interaction/slider-position-cases.json")
    slider_position_validator = validator_for("schemas/slider-position-cases.schema.json")
    check_case(slider_position_validator, "slider position matrix", slider_position, True)
    names = [case["name"] for case in slider_position["cases"]]
    if len(names) != len(set(names)):
        raise ValueError("duplicate slider position case")
    checked += 1 + len(names)
    for field in slider_position["cases"][0]["input"]:
        incomplete = copy.deepcopy(slider_position)
        incomplete["cases"][0]["input"].pop(field)
        check_case(slider_position_validator, f"missing slider position {field}", incomplete, False)
        checked += 1
    leaked = copy.deepcopy(slider_position)
    leaked["cases"][0]["input"]["renderer"] = "native"
    check_case(slider_position_validator, "slider position backend leakage", leaked, False)
    checked += 1
    slider_edits = load_json(ROOT / "conformance/interaction/slider-edit-cases.json")
    slider_states = load_json(ROOT / "conformance/interaction/slider-states-cases.json")
    states_validator = validator_for("schemas/slider-states-cases.schema.json")
    check_case(states_validator, "slider state cases", slider_states, True)
    names = [case["name"] for case in slider_states["cases"]]
    if len(names) != len(set(names)):
        raise ValueError("duplicate slider state case")
    state_pointer_traces = {case["name"]: case for case in
                            load_json(ROOT / "conformance/interaction/slider-pointer-cases.json")["cases"]}
    for case in slider_states["cases"]:
        if case["pointerTrace"] is not None:
            step = state_pointer_traces[case["pointerTrace"]]["steps"][case["stepIndex"]]
            if step.get("expected", {}).get("state", {}).get("hold") is None:
                raise ValueError(f"slider state case needs an open pointer hold: {case['name']}")
    checked += 1 + len(slider_states["cases"])
    for field in slider_states["cases"][0]:
        incomplete = copy.deepcopy(slider_states)
        incomplete["cases"][0].pop(field)
        check_case(states_validator, f"missing slider state {field}", incomplete, False)
        checked += 1
    for name, changes in [
        ("state permission must be explicit boolean", [{"path": "/cases/0/enabled", "value": "true"}]),
        ("state trace requires step", [{"path": "/cases/0/pointerTrace", "value": "trace"}]),
        ("state cannot invent read-only signal", [{"path": "/cases/0/expected/states/0", "value": "readOnly"}]),
    ]:
        check_case(states_validator, name, apply_changes(slider_states, changes), False)
        checked += 1
    leaked = copy.deepcopy(slider_states)
    leaked["cases"][0]["renderer"] = "native"
    check_case(states_validator, "slider state backend leakage", leaked, False)
    checked += 1
    presentations = load_json(ROOT / "conformance/interaction/slider-presentation-cases.json")
    check_case(validator_for("schemas/slider-presentation-cases.schema.json"),
               "slider presentation cases", presentations, True)
    names = [case["name"] for case in presentations["cases"]]
    if len(names) != len(set(names)):
        raise ValueError("duplicate slider presentation case")
    traces = {case["name"]: case for case in slider_edits["cases"]}
    for case in presentations["cases"]:
        step = traces[case["editTrace"]]["steps"][case["stepIndex"]]
        session = step["expected"]["session"]
        expected = case["expected"]
        if session is None or not expected["editing"] or any([
            {key: expected["committed"][key] for key in step["current"]} != step["current"],
            expected["visible"] != session["preview"],
            expected["revision"] != session["revision"],
            expected["valuePolicy"] != session["valuePolicy"],
        ]):
            raise ValueError(f"incoherent slider presentation case: {case['name']}")
    checked += 1 + len(presentations["cases"])
    presentation = presentations["cases"][0]["expected"]
    presentation_validator = validator_for("schemas/slider-presentation.schema.json")
    for field in presentation:
        incomplete = copy.deepcopy(presentation)
        incomplete.pop(field)
        check_case(presentation_validator, f"missing presentation {field}", incomplete, False)
        checked += 1
    for name, changes in [
        ("empty presentation revision", [{"path": "/revision", "value": ""}]),
        ("invalid presentation editing", [{"path": "/editing", "value": "true"}]),
        ("bare visible value", [{"path": "/visible", "value": 10}]),
    ]:
        check_case(presentation_validator, name, apply_changes(presentation, changes), False)
        checked += 1
    leaked = copy.deepcopy(presentation)
    leaked["renderer"] = "native"
    check_case(presentation_validator, "presentation backend leakage", leaked, False)
    checked += 1
    slider_keys = load_json(ROOT / "conformance/interaction/slider-key-cases.json")
    key_validator = validator_for("schemas/slider-key-cases.schema.json")
    check_case(key_validator, "slider keyboard cases", slider_keys, True)
    names = [case["name"] for case in slider_keys["cases"]]
    if len(names) != len(set(names)):
        raise ValueError("duplicate slider keyboard case")
    checked += 1 + len(slider_keys["cases"])
    key_result = next(case["expected"] for case in slider_keys["cases"]
                      if case.get("expected", {}).get("outcome") == "adjusted")
    key_result_validator = validator_for("schemas/slider-key-ir.schema.json")
    for field in key_result:
        incomplete = copy.deepcopy(key_result)
        incomplete.pop(field)
        check_case(key_result_validator, f"missing key result {field}", incomplete, False)
        checked += 1
    for name, changes in [
        ("adjusted key needs commit", [{"path": "/commit", "value": None}]),
        ("unavailable key cannot commit", [{"path": "/outcome", "value": "unavailable"}]),
        ("unsupported key cannot commit", [{"path": "/outcome", "value": "unsupported"}]),
        ("unknown key outcome", [{"path": "/outcome", "value": "held"}]),
        ("key commit must be accepted", [{"path": "/commit/accepted", "value": False},
                                         {"path": "/commit/changed", "value": False}]),
    ]:
        check_case(key_result_validator, name, apply_changes(key_result, changes), False)
        checked += 1
    check_case(key_result_validator, "key result cannot retain native events",
               dict(key_result, nativeKey=1), False)
    checked += 1
    policy = dict(slider_keys["cases"][0]["keyPolicy"], schemaVersion="0.1.0")
    key_policy_validator = validator_for("schemas/slider-key-policy.schema.json")
    check_case(key_policy_validator, "complete checked key policy", policy, True)
    checked += 1
    for field in policy:
        incomplete = copy.deepcopy(policy)
        incomplete.pop(field)
        check_case(key_policy_validator, f"missing checked key policy {field}", incomplete, False)
        checked += 1
    for field in policy["steps"]:
        incomplete = copy.deepcopy(policy)
        incomplete["steps"].pop(field)
        check_case(key_policy_validator, f"missing checked key quantum {field}", incomplete, False)
        checked += 1
    stopped_keys = copy.deepcopy(policy)
    stopped_keys["steps"] = {"kind": "stops", "step": 1, "page": 3}
    for name, changes in [
        ("stopped arrows cannot skip allowed values", [{"path": "/steps/step", "value": 2}]),
        ("stopped page must exceed one index", [{"path": "/steps/page", "value": 1}]),
    ]:
        check_case(key_policy_validator, name, apply_changes(stopped_keys, changes), False)
        checked += 1
    check_case(key_policy_validator, "key policy cannot retain backend tags",
               dict(policy, renderer="native"), False)
    checked += 1
    for name, changes in [
        ("key step must be positive", [{"path": "/steps/step", "value": 0}]),
        ("unknown key quantum", [{"path": "/steps/kind", "value": "automatic"}]),
        ("unknown key", [{"path": "/cases/0/key", "value": "nativeCode42"}]),
    ]:
        is_case = name == "unknown key"
        check_case(key_validator if is_case else key_policy_validator, name,
                   apply_changes(slider_keys if is_case else policy, changes), False)
        checked += 1
    for path in ["keyPolicy", "valuePolicy", "focused"]:
        incomplete = copy.deepcopy(slider_keys)
        incomplete["cases"][0].pop(path)
        check_case(key_validator, f"key delivery needs {path}", incomplete, False)
        checked += 1
    check_case(validator_for("schemas/slider-edit-cases.schema.json"),
               "slider edit traces", slider_edits, True)
    names = [case["name"] for case in slider_edits["cases"]]
    if len(names) != len(set(names)):
        raise ValueError("duplicate slider edit trace")
    checked += 1 + sum(len(case["steps"]) for case in slider_edits["cases"])
    edit_result = slider_edits["cases"][0]["steps"][0]["expected"]
    edit_result_validator = validator_for("schemas/slider-edit-result.schema.json")
    policy_validator = validator_for("schemas/slider-value-policy.schema.json")
    stopped = next(case for case in slider_edits["cases"]
                   if case["begin"]["valuePolicy"]["kind"] == "stops")
    stopped_policy = stopped["steps"][0]["expected"]["session"]["valuePolicy"]
    check_case(policy_validator, "complete stopped value policy", stopped_policy, True)
    checked += 1
    for policy in [edit_result["session"]["valuePolicy"], stopped_policy]:
        for field in policy:
            incomplete = copy.deepcopy(policy)
            incomplete.pop(field)
            check_case(policy_validator, f"missing policy {field}", incomplete, False)
            checked += 1
        check_case(policy_validator, "policy cannot retain native routing",
                   dict(policy, nativeHandle=1), False)
        checked += 1
    for name, changes in [
        ("unknown policy", [{"path": "/kind", "value": "automatic"}]),
        ("unknown tie rule", [{"path": "/tieBreak", "value": "closestPixel"}]),
        ("stops need full numeric IR", [{"path": "/stops/values/0", "value": -10}]),
    ]:
        check_case(policy_validator, name, apply_changes(stopped_policy, changes), False)
        checked += 1
    for path in ["begin", "steps"]:
        incomplete = copy.deepcopy(slider_edits)
        obj = incomplete["cases"][0][path]
        if path == "steps":
            obj = obj[0]
        obj.pop("valuePolicy")
        check_case(validator_for("schemas/slider-edit-cases.schema.json"),
                   f"edit {path} needs explicit policy", incomplete, False)
        checked += 1
    incomplete = copy.deepcopy(edit_result)
    incomplete["session"].pop("valuePolicy")
    check_case(edit_result_validator, "session needs frozen policy", incomplete, False)
    checked += 1
    for field in edit_result:
        incomplete = copy.deepcopy(edit_result)
        incomplete.pop(field)
        check_case(edit_result_validator, f"missing slider edit result {field}", incomplete, False)
        checked += 1
    for name, changes in [
        ("preview must retain session", [{"path": "/session", "value": None}]),
        ("cancel must close session", [{"path": "/outcome", "value": "cancelled"}]),
        ("commit needs intent", [{"path": "/outcome", "value": "committed"},
                                 {"path": "/session", "value": None}]),
        ("edit revision must not be empty", [{"path": "/session/revision", "value": ""}]),
        ("unknown edit outcome", [{"path": "/outcome", "value": "rebased"}]),
    ]:
        check_case(edit_result_validator, name, apply_changes(edit_result, changes), False)
        checked += 1
    check_case(edit_result_validator, "slider edit backend leakage",
               dict(edit_result, renderer="native"), False)
    checked += 1
    completed_edit = slider_edits["cases"][0]["steps"][-1]["expected"]
    for name, changes in [
        ("completed edit cannot retain session", [{"path": "/session", "value": edit_result["session"]}]),
        ("completed edit requires accepted intent", [{"path": "/commit/accepted", "value": False},
                                                     {"path": "/commit/changed", "value": False}]),
    ]:
        check_case(edit_result_validator, name, apply_changes(completed_edit, changes), False)
        checked += 1
    check_case(edit_result_validator, "preview cannot emit commit",
               dict(edit_result, commit=completed_edit["commit"]), False)
    checked += 1
    slider_anchors = load_json(ROOT / "conformance/interaction/slider-anchor-cases.json")
    anchor_validator = validator_for("schemas/slider-anchor-cases.schema.json")
    check_case(anchor_validator, "slider pointer anchor cases", slider_anchors, True)
    names = [case["name"] for case in slider_anchors["cases"]]
    if len(names) != len(set(names)):
        raise ValueError("duplicate slider pointer anchor case")
    checked += 1 + len(slider_anchors["cases"])
    missing_grab = copy.deepcopy(slider_anchors)
    missing_grab["cases"][0].pop("grabPoint")
    check_case(anchor_validator, "grab requires point", missing_grab, False)
    checked += 1
    for name, changes in [
        ("grab point cannot be null", [{"path": "/cases/0/grabPoint", "value": None}]),
        ("center rejects grab point", [{"path": "/cases/0/mode", "value": "center"}]),
        ("unknown anchor mode", [{"path": "/cases/0/mode", "value": "auto"}]),
    ]:
        check_case(anchor_validator, name, apply_changes(slider_anchors, changes), False)
        checked += 1
    for name, path in [("unknown pointer coordinate", "point"),
                       ("anchor cannot hide backend state", None)]:
        extra = copy.deepcopy(slider_anchors)
        if path is None:
            extra["cases"][0]["renderer"] = "native"
        else:
            extra["cases"][0][path]["z"] = 0
        check_case(anchor_validator, name, extra, False)
        checked += 1
    slider_pointers = load_json(ROOT / "conformance/interaction/slider-pointer-cases.json")
    pointer_validator = validator_for("schemas/slider-pointer-cases.schema.json")
    check_case(pointer_validator, "slider pointer traces", slider_pointers, True)
    incomplete = copy.deepcopy(slider_pointers)
    incomplete["cases"][0]["steps"][0].pop("valuePolicy")
    check_case(pointer_validator, "pointer needs explicit value policy", incomplete, False)
    checked += 1
    names = [case["name"] for case in slider_pointers["cases"]]
    if len(names) != len(set(names)):
        raise ValueError("duplicate slider pointer trace")
    for case in slider_pointers["cases"]:
        for step in case["steps"]:
            if step["layout"] not in case["layouts"] or step["control"] not in case["regions"]:
                raise ValueError(f"unknown slider pointer fixture reference: {case['name']}")
            if step["event"]["kind"] == "down" and step["event"]["region"] not in case["regions"]:
                raise ValueError(f"unknown slider pointer target fixture: {case['name']}")
    checked += 1 + sum(len(case["steps"]) for case in slider_pointers["cases"])
    armed = slider_pointers["cases"][0]["steps"][0]["expected"]
    pointer_result_validator = validator_for("schemas/slider-pointer-result.schema.json")
    for field in armed:
        missing = copy.deepcopy(armed)
        missing.pop(field)
        check_case(pointer_result_validator, f"missing slider pointer result {field}", missing, False)
        checked += 1
    for name, changes in [
        ("armed pointer needs hold", [{"path": "/state/hold", "value": None}]),
        ("pending pointer needs acquisition request", [{"path": "/routing", "value": None}]),
        ("armed pointer cannot be acquired", [{"path": "/state/hold/phase", "value": "acquired"}]),
        ("armed pointer cannot release", [{"path": "/routing/kind", "value": "release"}]),
        ("held identity must not be empty", [{"path": "/state/hold/id", "value": ""}]),
    ]:
        check_case(pointer_result_validator, name, apply_changes(armed, changes), False)
        checked += 1
    completed = next(step["expected"] for step in slider_pointers["cases"][0]["steps"]
                     if step.get("expected", {}).get("outcome") == "committed")
    for name, changes in [
        ("committed pointer must close", [{"path": "/state/hold", "value": armed["state"]["hold"]}]),
        ("committed pointer needs intent", [{"path": "/commit", "value": None}]),
        ("committed pointer needs accepted intent", [{"path": "/commit/accepted", "value": False},
                                                   {"path": "/commit/changed", "value": False}]),
        ("committed pointer cannot acquire", [{"path": "/routing/kind", "value": "acquire"}]),
    ]:
        check_case(pointer_result_validator, name, apply_changes(completed, changes), False)
        checked += 1
    check_case(pointer_result_validator, "armed pointer cannot commit",
               dict(armed, commit=completed["commit"]), False)
    checked += 1
    previewed = next(step["expected"] for step in slider_pointers["cases"][0]["steps"]
                     if step.get("expected", {}).get("outcome") == "previewed")
    ignored = slider_pointers["cases"][0]["steps"][1]["expected"]
    fallback = next(step["expected"] for case in slider_pointers["cases"] for step in case["steps"]
                    if step.get("expected", {}).get("outcome") == "armed"
                    and step["expected"]["state"]["hold"]["phase"] == "terminationOnly")
    for name, document in [
        ("preview requires acquired routing", apply_changes(previewed, [{"path": "/state/hold/phase", "value": "pending"}])),
        ("ignored pointer cannot acquire", dict(ignored, routing=armed["routing"])),
        ("fallback pointer cannot acquire", dict(fallback, routing=armed["routing"])),
        ("cancelled pointer cannot commit", dict(completed, outcome="cancelled")),
        ("unknown pointer outcome", dict(armed, outcome="captured")),
    ]:
        check_case(pointer_result_validator, name, document, False)
        checked += 1
    for where in [None, "anchor"]:
        leaked = copy.deepcopy(armed)
        if where is None:
            leaked["renderer"] = "native"
        else:
            leaked["state"]["hold"][where]["renderer"] = "native"
        check_case(pointer_result_validator, "slider pointer backend leakage", leaked, False)
        checked += 1
    malformed_event = copy.deepcopy(slider_pointers)
    malformed_event["cases"][0]["steps"][0]["event"]["button"] = "primary"
    check_case(pointer_validator, "undeclared slider pointer event field", malformed_event, False)
    checked += 1
    for filename, schema_name in [
        ("slider-stops-cases.json", "slider-stops-cases"),
        ("slider-stop-adjustment-cases.json", "slider-stop-adjustment-cases"),
    ]:
        documents = load_json(ROOT / "conformance/interaction" / filename)
        check_case(validator_for(f"schemas/{schema_name}.schema.json"), schema_name, documents, True)
        names = [case["name"] for case in documents["cases"]]
        if len(names) != len(set(names)):
            raise ValueError(f"duplicate {schema_name} case")
        checked += 1 + len(documents["cases"])
    stops = load_json(ROOT / "conformance/interaction/slider-stops-cases.json")["cases"][0]["expected"]
    stops_validator = validator_for("schemas/slider-stops-ir.schema.json")
    for name, document in [
        ("missing slider stops version", {"values": stops["values"]}),
        ("missing slider stops values", {"schemaVersion": "0.1.0"}),
        ("empty slider stops", dict(stops, values=[])),
        ("single slider stop", dict(stops, values=stops["values"][:1])),
        ("duplicate slider stops", dict(stops, values=[stops["values"][0], stops["values"][0]])),
        ("slider stops backend leakage", dict(stops, renderer="native")),
    ]:
        check_case(stops_validator, name, document, False)
        checked += 1
    for field in stops["values"][1]:
        incomplete = copy.deepcopy(stops)
        incomplete["values"][1].pop(field)
        check_case(stops_validator, f"incomplete slider stop {field}", incomplete, False)
        checked += 1
    stop_adjustments = load_json(ROOT / "conformance/interaction/slider-stop-adjustment-cases.json")
    stop_cases_validator = validator_for("schemas/slider-stop-adjustment-cases.schema.json")
    nearest_case = next(case for case in stop_adjustments["cases"] if case["adjustment"]["kind"] == "nearest")
    for name, action in [
        ("missing nearest tie policy", {"kind": "nearest", "value": 0.5}),
        ("unsupported nearest tie policy", {"kind": "nearest", "value": 0.5, "tieBreak": "automatic"}),
        ("negative stop count", {"kind": "increase", "count": -1}),
        ("fractional stop count", {"kind": "decrease", "count": 0.5}),
        ("boolean stop count", {"kind": "increase", "count": True}),
        ("stop count exceeds reference integer", {"kind": "increase", "count": 18446744073709551616}),
        ("undeclared stop action field", {"kind": "minimum", "repeat": True}),
    ]:
        malformed = dict(nearest_case, adjustment=action)
        check_case(stop_cases_validator, name, {"schemaVersion": "0.1.0", "cases": [malformed]}, False)
        checked += 1
    toggle_travel = load_json(ROOT / "conformance/motion/toggle-travel-cases.json")
    check_case(validator_for("schemas/toggle-travel-cases.schema.json"), "toggle travel matrix", toggle_travel, True)
    names = [case["name"] for case in toggle_travel["cases"]]
    if len(names) != len(set(names)):
        raise ValueError("duplicate toggle travel case")
    checked += 1 + len(names)
    toggle_snapshots = load_json(ROOT / "conformance/interaction/toggle-snapshot-cases.json")
    check_case(validator_for("schemas/toggle-snapshot-cases.schema.json"), "toggle snapshot matrix", toggle_snapshots, True)
    names = [case["name"] for case in toggle_snapshots["cases"]]
    if len(names) != len(set(names)):
        raise ValueError("duplicate toggle snapshot case")
    checked += 1 + len(names)
    toggle_motion = load_json(ROOT / "conformance/ir/toggle-part-motion-request.json")
    check_case(validator_for("schemas/toggle-part-motion-request.schema.json"), "toggle motion baseline", toggle_motion, True)
    checked += 1
    names = set()
    for case in load_json(ROOT / "conformance/ir/toggle-part-motion-cases.json"):
        if case["name"] in names:
            raise ValueError("duplicate toggle part motion case: " + case["name"])
        names.add(case["name"])
        check_case(validator_for("schemas/toggle-part-motion-case.schema.json"), case["name"], case, True)
        check_case(validator_for("schemas/toggle-part-motion-request.schema.json"), case["name"], apply_changes(toggle_motion, case["requestChanges"]), case["requestSchemaValid"])
        checked += 2
    toggle_part_request = load_json(ROOT / "conformance/ir/toggle-part-paint-request.json")
    check_case(validator_for("schemas/toggle-part-paint-request.schema.json"), "toggle part baseline", toggle_part_request, True)
    checked += 1
    toggle_part_names = set()
    for case in load_json(ROOT / "conformance/ir/toggle-part-paint-cases.json"):
        name = case["name"]
        if name in toggle_part_names:
            raise ValueError(f"duplicate toggle part paint case: {name}")
        toggle_part_names.add(name)
        check_case(validator_for("schemas/toggle-part-paint-case.schema.json"), name, case, True)
        check_case(validator_for("schemas/toggle-part-paint-request.schema.json"), name, apply_changes(toggle_part_request, case["requestChanges"]), case["requestSchemaValid"])
        checked += 1
    print(f"Validated {len(schema_paths)} schemas and {checked} conformance cases")


if __name__ == "__main__":
    main()
