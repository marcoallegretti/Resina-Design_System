import copy
import json
from pathlib import Path

from jsonschema import Draft202012Validator


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

    with path.open(encoding="utf-8") as source:
        return json.load(
            source,
            object_pairs_hook=unique_members,
            parse_constant=invalid_constant,
        )


def validator_for(path):
    schema = load_json(ROOT / path)
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema)


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
        "schemas/material-assignments.schema.json",
        "schemas/state-set.schema.json",
        "schemas/surface-form.schema.json",
        "schemas/treatment-stack.schema.json",
        "schemas/versions/environment-0.1.0.schema.json",
    }
    actual_paths = {path.relative_to(ROOT).as_posix() for path in schema_paths}
    if actual_paths != expected_paths:
        raise ValueError(f"schema coverage differs: {actual_paths ^ expected_paths}")
    for path in schema_paths:
        validator_for(path.relative_to(ROOT))

    checked = 0
    for schema, vectors in (
        ("schemas/color-assignments.schema.json", "conformance/color/role-assignment-vectors.json"),
        ("schemas/material-assignments.schema.json", "conformance/materials/role-assignment-vectors.json"),
        ("schemas/state-set.schema.json", "conformance/states/state-set-vectors.json"),
        ("schemas/surface-form.schema.json", "conformance/geometry/surface-form-vectors.json"),
        ("schemas/treatment-stack.schema.json", "conformance/materials/treatment-stack-vectors.json"),
    ):
        checked += check_vectors(schema, vectors)

    environment = validator_for("schemas/environment.schema.json")
    for filename, valid in (
        ("valid-mixed-input.json", True),
        ("valid-minimal-capabilities.json", True),
        ("invalid-duplicate-input.json", False),
    ):
        path = ROOT / "conformance" / "environment" / filename
        check_case(environment, str(path.relative_to(ROOT)), load_json(path), valid)
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
