import unittest

from check_schemas import ROOT, apply_changes, load_json, validator_for
from check_surface_paint_backend import baseline as paint_baseline


PREFIX = "elevation source shape "
ROLES = ["embedded", "base", "raised", "floating", "overlay", "modal"]


def malformed(original, name):
    roles = [original["roles"][role] for role in ROLES]
    if name == PREFIX + "both positional":
        return [original["schemaVersion"], roles]
    group, shape = name.removeprefix(PREFIX).split(" ", 1)
    positional = [original["schemaVersion"], original["roles"]] if group == "root" else roles
    value = {"positional": positional, "null": None, "boolean": True,
             "number": 1, "string": "assignment", "empty array": []}[shape]
    return value if group == "root" else {**original, "roles": value}


class ElevationAssignmentBoundaryTests(unittest.TestCase):
    def test_model_vectors_require_objects(self):
        vectors = load_json(ROOT / "conformance/elevation/depth-assignment-vectors.json")
        validator = validator_for("schemas/elevation-depth-assignments.schema.json")
        invalid = [v for v in vectors if v["name"].startswith(PREFIX)]
        names = {PREFIX + group + " " + shape for group in ["root", "roles"]
                 for shape in ["positional", "null", "boolean", "number", "string", "empty array"]}
        names.add(PREFIX + "both positional")
        self.assertEqual(len(invalid), 13)
        self.assertEqual({v["name"] for v in invalid}, names)
        original = vectors[0]["document"]
        self.assertEqual(set(original["roles"]), set(ROLES))
        for vector in vectors:
            with self.subTest(name=vector["name"]):
                self.assertEqual(validator.is_valid(vector["document"]), "expected" in vector)
                if "expected" in vector:
                    self.assertEqual(vector["document"]["roles"], vector["expected"])
                elif vector["name"].startswith(PREFIX):
                    self.assertEqual(vector["document"], malformed(original, vector["name"]))

    def test_public_cases_change_only_assignment_shapes(self):
        names = {v["name"] for v in load_json(ROOT / "conformance/elevation/depth-assignment-vectors.json")
                 if v["name"].startswith(PREFIX)}
        elevation = load_json(ROOT / "conformance/elevation/backend-cases.json")
        consumers = [
            ("conformance/elevation/backend-cases.json", "schemas/elevation-depth-request.schema.json",
             elevation[0]["request"], "/assignments"),
            ("conformance/ir/focus-ir-cases.json", "schemas/focus-ir-request.schema.json",
             load_json(ROOT / "conformance/ir/focus-ir-request.json"), "/depthAssignments"),
            ("conformance/ir/opaque-surface-cases.json", "schemas/opaque-surface-request.schema.json",
             load_json(ROOT / "conformance/ir/opaque-surface-request.json"), "/appearance/depthAssignments"),
            ("conformance/ir/surface-paint-cases.json", "schemas/surface-paint-request.schema.json",
             paint_baseline(), "/body/appearance/depthAssignments"),
        ]
        for path, schema, original, pointer in consumers:
            cases = [c for c in load_json(ROOT / path) if c["name"].startswith(PREFIX)]
            self.assertEqual(len(cases), 13)
            self.assertEqual({c["name"] for c in cases}, names)
            validator = validator_for(schema)
            self.assertTrue(validator.is_valid(original))
            assignment = original
            for member in pointer[1:].split("/"):
                assignment = assignment[member]
            for case in cases:
                with self.subTest(path=path, name=case["name"]):
                    expected = malformed(assignment, case["name"])
                    if "request" in case:
                        request = case["request"]
                        self.assertEqual(request, apply_changes(original, [{"path": pointer, "value": expected}]))
                    else:
                        target = pointer + ("/roles" if case["name"].startswith(PREFIX + "roles ") else "")
                        value = expected["roles"] if target.endswith("/roles") else expected
                        self.assertEqual(case["requestChanges"], [{"path": target, "value": value}])
                        request = apply_changes(original, case["requestChanges"])
                    self.assertFalse(case["requestSchemaValid"])
                    self.assertTrue(case.get("failure", False) or case.get("errorContains") == "assignment object")
                    self.assertFalse(validator.is_valid(request))
                    self.assertEqual(apply_changes(request, [{"path": pointer, "value": assignment}]), original)


if __name__ == "__main__":
    unittest.main()
