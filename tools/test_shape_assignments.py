import unittest

from check_schemas import ROOT, apply_changes, load_json, validator_for
from check_surface_paint_backend import baseline as paint_baseline


PREFIX = "shape source form "
PROFILES = ["structural", "soft", "rounded", "capsule", "organic"]
CORNERS = ["topStart", "topEnd", "bottomEnd", "bottomStart"]
FORMS = ["positional", "null", "boolean", "number", "string", "empty array"]


def malformed(original, name):
    label = name.removeprefix(PREFIX)
    group, form = next((label.removesuffix(" " + f), f) for f in FORMS
                       if label.endswith(" " + f))
    pointer = ""
    if group == "root":
        positional = [original["schemaVersion"], original["profiles"]]
    elif group == "profiles":
        pointer = "/profiles"
        positional = [original["profiles"][p] for p in PROFILES]
    elif group == "organic radii":
        pointer = "/profiles/organic/radii"
        positional = [original["profiles"]["organic"]["radii"][c] for c in CORNERS]
    else:
        profile = group.split()[0]
        pointer = "/profiles/" + profile
        value = original["profiles"][profile]
        if group.endswith("corners") or group.endswith("radii"):
            radii = {c: value["radius"] for c in CORNERS}
            positional = (["corners", radii] if group.endswith("corners") else
                          {"kind": "corners", "radii": list(radii.values())})
        else:
            positional = [value["kind"]]
            if value["kind"] != "capsule":
                positional.append(value["radius" if value["kind"] == "uniform" else "radii"])
    value = {"positional": positional, "null": None, "boolean": True,
             "number": 1, "string": "assignment", "empty array": []}[form]
    return pointer, value


class ShapeAssignmentBoundaryTests(unittest.TestCase):
    def test_model_vectors_match_assignment_schema(self):
        vectors = load_json(ROOT / "conformance/geometry/shape-fallback-assignment-vectors.json")
        validator = validator_for("schemas/shape-fallback-assignments.schema.json")
        invalid = [v for v in vectors if v["name"].startswith(PREFIX)]
        names = {PREFIX + group + " " + form
                 for group in ["root", "profiles", *PROFILES, "organic radii"] for form in FORMS}
        names.update(PREFIX + p + " " + form for p in PROFILES[:3]
                     for form in ["corners positional", "radii positional"])
        self.assertEqual(len(invalid), 54)
        self.assertEqual({v["name"] for v in invalid}, names)
        original = vectors[0]["document"]
        for vector in vectors:
            with self.subTest(name=vector["name"]):
                self.assertEqual(validator.is_valid(vector["document"]), "expected" in vector)
                if vector["name"].startswith(PREFIX):
                    pointer, value = malformed(original, vector["name"])
                    expected = (apply_changes(original, [{"path": pointer, "value": value}])
                                if pointer else value)
                    self.assertEqual(vector["document"], expected)

    def test_public_cases_change_only_shape_assignment_forms(self):
        names = {v["name"] for v in load_json(ROOT / "conformance/geometry/shape-fallback-assignment-vectors.json")
                 if v["name"].startswith(PREFIX)}
        consumers = [
            ("conformance/ir/focus-ir-cases.json", "schemas/focus-ir-request.schema.json",
             load_json(ROOT / "conformance/ir/focus-ir-request.json"), "/shapeAssignments"),
            ("conformance/ir/opaque-surface-cases.json", "schemas/opaque-surface-request.schema.json",
             load_json(ROOT / "conformance/ir/opaque-surface-request.json"), "/appearance/shapeAssignments"),
            ("conformance/ir/surface-paint-cases.json", "schemas/surface-paint-request.schema.json",
             paint_baseline(), "/body/appearance/shapeAssignments"),
        ]
        for path, schema, original, owner in consumers:
            cases = [c for c in load_json(ROOT / path) if c["name"].startswith(PREFIX)]
            self.assertEqual(len(cases), 54)
            self.assertEqual({c["name"] for c in cases}, names)
            validator = validator_for(schema)
            self.assertTrue(validator.is_valid(original))
            assignment = original
            for part in owner[1:].split("/"):
                assignment = assignment[part]
            for case in cases:
                with self.subTest(path=path, name=case["name"]):
                    pointer, value = malformed(assignment, case["name"])
                    self.assertEqual(case["requestChanges"], [{"path": owner + pointer, "value": value}])
                    request = apply_changes(original, case["requestChanges"])
                    self.assertFalse(case["requestSchemaValid"])
                    self.assertTrue(case.get("failure", False) or case.get("errorContains") == "assignment object")
                    self.assertFalse(validator.is_valid(request))
                    self.assertEqual(apply_changes(request, [{"path": owner, "value": assignment}]), original)

    def test_case_schema_requires_correct_validity_declarations(self):
        validator = validator_for("schemas/shape-fallback-assignment-case.schema.json")
        document = load_json(ROOT / "definitions/tier0-shapes.json")
        self.assertTrue(validator.is_valid({"name": "valid", "document": document, "expected": "valid"}))
        for invalid in [None, True, 1, "assignment", [], [document["schemaVersion"], document["profiles"]]]:
            self.assertTrue(validator.is_valid({"name": "invalid", "document": invalid, "error": "assignment object"}))
            self.assertFalse(validator.is_valid({"name": "invalid", "document": invalid, "expected": "valid"}))
            self.assertFalse(validator.is_valid({"name": "invalid", "document": invalid}))
        self.assertFalse(validator.is_valid({"name": "invalid", "document": document, "error": "invalid"}))


if __name__ == "__main__":
    unittest.main()
