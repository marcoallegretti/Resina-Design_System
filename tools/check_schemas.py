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


def main():
    schema_paths = sorted((ROOT / "schemas").rglob("*.schema.json"))
    expected_paths = {
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
        len(backend_cases)
        + len(surface_backend_cases)
        + len(theme_cases)
        + len(theme_resolution_cases)
        + len(resolver_cases)
        + len(frost_cases)
        + len(edge_cases)
        + len(readability_cases)
        + len(focus_cases)
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
            checked += 1
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

    print(f"Validated {len(schema_paths)} schemas and {checked} conformance cases")


if __name__ == "__main__":
    main()
