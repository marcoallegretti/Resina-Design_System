import copy
import json
import math
from functools import lru_cache
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[1]


def load_json(path):
    def unique_members(pairs):
        result = {}
        for name, value in pairs:
            if name in result:
                raise ValueError(f"duplicate JSON member {name!r} in {path}")
            result[name] = value
        return result

    def invalid_constant(value):
        raise ValueError(f"invalid JSON number {value} in {path}")

    def finite_float(value):
        number = float(value)
        if not math.isfinite(number):
            raise ValueError(f"JSON number exceeds finite range in {path}")
        return number

    def finite_int(value):
        number = int(value)
        try:
            finite = math.isfinite(float(number))
        except OverflowError:
            finite = False
        if not finite:
            raise ValueError(f"JSON number exceeds finite range in {path}")
        return number

    with path.open(encoding="utf-8") as source:
        return json.load(
            source,
            object_pairs_hook=unique_members,
            parse_constant=invalid_constant,
            parse_float=finite_float,
            parse_int=finite_int,
        )


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
        "schemas/color-assignments.schema.json",
        "schemas/environment.schema.json",
        "schemas/headless-resolution.schema.json",
        "schemas/headless-result.schema.json",
        "schemas/material-assignments.schema.json",
        "schemas/spatial-assignments.schema.json",
        "schemas/srgb-fallback.schema.json",
        "schemas/state-set.schema.json",
        "schemas/surface-form.schema.json",
        "schemas/surface-binding.schema.json",
        "schemas/surface-binding-result.schema.json",
        "schemas/surface-scenario.schema.json",
        "schemas/treatment-stack.schema.json",
        "schemas/typography-assignments.schema.json",
        "schemas/versions/material-assignments-0.1.0.schema.json",
        "schemas/versions/environment-0.1.0.schema.json",
        "schemas/versions/headless-result-0.1.0.schema.json",
    }
    actual_paths = {path.relative_to(ROOT).as_posix() for path in schema_paths}
    if actual_paths != expected_paths:
        raise ValueError(f"schema coverage differs: {actual_paths ^ expected_paths}")
    for path in schema_paths:
        validator_for(path.relative_to(ROOT))

    material_schema = load_json(ROOT / "schemas/material-assignments.schema.json")
    color_schema = load_json(ROOT / "schemas/color-assignments.schema.json")
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

    checked = 0
    for schema, vectors in (
        ("schemas/color-assignments.schema.json", "conformance/color/role-assignment-vectors.json"),
        ("schemas/material-assignments.schema.json", "conformance/materials/role-assignment-vectors.json"),
        ("schemas/spatial-assignments.schema.json", "conformance/spatial/assignment-vectors.json"),
        ("schemas/state-set.schema.json", "conformance/states/state-set-vectors.json"),
        ("schemas/surface-form.schema.json", "conformance/geometry/surface-form-vectors.json"),
        ("schemas/surface-binding.schema.json", "conformance/surfaces/binding-vectors.json"),
        ("schemas/treatment-stack.schema.json", "conformance/materials/treatment-stack-vectors.json"),
        ("schemas/typography-assignments.schema.json", "conformance/typography/assignment-vectors.json"),
    ):
        checked += check_vectors(schema, vectors)

    surface_results = validator_for("schemas/surface-binding-result.schema.json")
    surface_vectors = load_json(ROOT / "conformance/surfaces/binding-vectors.json")
    for vector in surface_vectors:
        if "expected" in vector:
            check_case(surface_results, f"surface result: {vector['name']}", vector["expected"], True)
            checked += 1
    for name, source_index, change in (
        ("Frost representation omitted", 0, lambda result: result.pop("frostRepresentation")),
        (
            "Frost representation on Elastomer",
            1,
            lambda result: result.update({"frostRepresentation": "opaqueDimensional"}),
        ),
        ("Gel as structural control", 1, lambda result: result.update({"materialFamily": "gel"})),
    ):
        result = copy.deepcopy(surface_vectors[source_index]["expected"])
        change(result)
        check_case(surface_results, f"surface result: {name}", result, False)
        checked += 1

    headless = validator_for("schemas/headless-resolution.schema.json")
    valid_headless = load_json(ROOT / "conformance/headless/valid-request.json")
    check_case(headless, "headless valid request", valid_headless, True)
    checked += 1
    for name, change in (
        ("unknown member", lambda document: document.update({"rendererName": "example"})),
        ("missing input", lambda document: document.pop("environment")),
        ("nested role", lambda document: document["colorAssignments"]["roles"].pop("focus")),
    ):
        document = copy.deepcopy(valid_headless)
        change(document)
        check_case(headless, f"headless {name}", document, False)
        checked += 1

    scenario_schema = validator_for("schemas/surface-scenario.schema.json")
    for vector in surface_vectors:
        scenario = {
            "schemaVersion": "0.1.0",
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
        "schemaVersion": "0.1.0",
        "resolution": valid_headless,
        "surface": surface_vectors[0]["document"],
    }
    for name, change in (
        ("unsupported version", lambda document: document.update({"schemaVersion": "0.2.0"})),
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
        (
            "unknown fallback role",
            ("colorFallbacks", "unknown"),
            expected_result["colorFallbacks"]["focus"],
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

    previous_result = validator_for("schemas/versions/headless-result-0.1.0.schema.json")
    archived_result = copy.deepcopy(expected_result)
    archived_result["schemaVersion"] = "0.1.0"
    archived_result.pop("colorFallbacks")
    check_case(previous_result, "headless result 0.1.0 archive", archived_result, True)
    check_case(result_schema, "current result rejects 0.1.0 archive", archived_result, False)
    checked += 2

    color_value = Draft202012Validator(
        {"$ref": "urn:resina:schema:headless-result:0.2.0#/$defs/color"},
        registry=schema_registry(),
    )
    for vector in load_json(ROOT / "conformance/tokens/primitive-value-vectors.json"):
        if vector["type"] == "color":
            check_case(color_value, vector["name"], vector["value"], "error" not in vector)
            checked += 1
    for vector in load_json(ROOT / "conformance/tokens/color-space-vectors.json"):
        check_case(color_value, vector["name"], vector["value"], "error" not in vector)
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
