import json
import unittest

from check_schemas import ROOT, apply_changes, load_json, validator_for
from check_theme_backend import cases as theme_cases


PREFIX = "color source shape "
ASSIGNMENTS = {
    "colorAssignments": ("conformance/color/role-assignment-vectors.json", "schemas/color-assignments.schema.json"),
    "opaqueColorAssignments": ("conformance/color/opaque-assignment-vectors.json", "schemas/opaque-color-assignments.schema.json"),
}


class ColorAssignmentBoundaryTests(unittest.TestCase):
    def test_model_vectors_require_objects_and_preserve_roles(self):
        for key, (path, schema) in ASSIGNMENTS.items():
            vectors = load_json(ROOT / path)
            validator = validator_for(schema)
            invalid = [v for v in vectors if v["name"].startswith(PREFIX)]
            self.assertEqual(len(invalid), 12)
            for vector in vectors:
                with self.subTest(key=key, name=vector["name"]):
                    self.assertEqual(validator.is_valid(vector["document"]), "expected" in vector)
                    if "expected" in vector:
                        self.assertEqual(vector["document"]["roles"], vector["expected"])
            root = [v for v in invalid if " root " in v["name"]]
            roles = [v for v in invalid if " roles " in v["name"]]
            self.assertEqual(len(root), 6)
            self.assertEqual(len(roles), 6)
            baseline = vectors[0]["document"] if key == "colorAssignments" else vectors[1]["document"]
            positional_root = next(v for v in root if v["name"].endswith("positional"))
            self.assertEqual(positional_root["document"], [baseline["schemaVersion"], baseline["roles"]])
            positional_roles = next(v for v in roles if v["name"].endswith("positional"))
            fields = list(load_json(ROOT / "schemas/color-assignments.schema.json")["properties"]["roles"]["required"])
            expected = [baseline["roles"][f] for f in fields] if key == "colorAssignments" \
                else [list(item) for item in baseline["roles"].items()]
            self.assertEqual(positional_roles["document"]["roles"], expected)
            positive = [v["document"]["roles"] for v in vectors if "expected" in v]
            if key == "opaqueColorAssignments":
                self.assertIn({}, positive)
                self.assertTrue(any(0 < len(v) < 19 for v in positive))
            self.assertTrue(any(len(v) == 19 for v in positive))

    def test_public_cases_change_only_assignment_shapes(self):
        vectors = {v["name"] for path, _ in ASSIGNMENTS.values()
                   for v in load_json(ROOT / path) if v["name"].startswith(PREFIX)}
        baseline = load_json(ROOT / "conformance/headless/valid-request.json")
        surface = load_json(ROOT / "conformance/surfaces/binding-vectors.json")[0]["document"]
        for path, schema, original, prefix in [
            ("conformance/headless/backend-cases.json", "schemas/headless-resolution.schema.json", baseline, ""),
            ("conformance/surfaces/scenario-cases.json", "schemas/surface-scenario.schema.json",
             {"schemaVersion": "0.4.0", "resolution": baseline, "surface": surface}, "/resolution"),
        ]:
            cases = [c for c in load_json(ROOT / path) if c["name"].startswith(PREFIX)]
            self.assertEqual(len(cases), 24)
            self.assertEqual({c["name"] for c in cases}, vectors)
            validator = validator_for(schema)
            self.assertTrue(validator.is_valid(original))
            for case in cases:
                with self.subTest(path=path, name=case["name"]):
                    self.assertEqual(case["outcome"], "invalid")
                    self.assertEqual(len(case["requestChanges"]), 1)
                    change = case["requestChanges"][0]
                    assignment = change["path"].removeprefix(prefix).split("/")[1]
                    self.assertIn(assignment, ASSIGNMENTS)
                    self.assertEqual(change["path"], prefix + self.assignment_pointer(case["name"]))
                    request = apply_changes(original, case["requestChanges"])
                    self.assertFalse(validator.is_valid(request))
                    restored = apply_changes(request, [{"path": prefix + "/" + assignment, "value": baseline[assignment]}])
                    self.assertEqual(restored, original)
                    self.check_shape(case["name"], change, baseline[assignment], assignment)
        public = {c["name"]: c for c in load_json(ROOT / "conformance/themes/resolution-cases.json")}
        themes = [(name, source, expected) for name, source, expected in theme_cases() if name.startswith(PREFIX)]
        self.assertEqual(len(themes), 30)
        variants = set()
        for name, source, expected in themes:
            case = public[name]
            self.assertIsNone(expected)
            self.assertEqual(case["outcome"], "invalid")
            self.assertTrue(case["requestSchemaValid"])
            self.assertEqual(len(case["sourceChanges"]), 1)
            self.assertNotIn("requestChanges", case)
            self.assertNotIn("sourceReplace", case)
            change = case["sourceChanges"][0]
            self.assertEqual(change["path"], self.assignment_pointer(name))
            assignment = change["path"].split("/")[1]
            self.assertIn(assignment, ASSIGNMENTS)
            request = json.loads(source)
            self.assertTrue(validator_for("schemas/theme-resolution-request.schema.json").is_valid(request))
            document = request["themeSource"]
            theme = json.loads(document)
            validator = validator_for("schemas/theme-source.schema.json")
            self.assertFalse(validator.is_valid(theme))
            restored = apply_changes(theme, [{"path": "/" + assignment, "value": baseline[assignment]}])
            self.assertTrue(validator.is_valid(restored))
            self.check_shape(name, change, baseline[assignment], assignment)
            variants.add(case.get("sourceVariant", "embedded"))
        self.assertEqual(variants, {"embedded", "resolverBackedInline", "foundationResolver"})

    def assignment_pointer(self, name):
        key = "colorAssignments" if "semantic" in name else "opaqueColorAssignments"
        return "/" + key + ("/roles" if " roles " in name else "")

    def check_shape(self, name, change, baseline, key):
        value = change["value"]
        if "positional" in name:
            if " root " in name:
                self.assertEqual(value, [baseline["schemaVersion"], baseline["roles"]])
            elif key == "colorAssignments":
                fields = load_json(ROOT / "schemas/color-assignments.schema.json")["properties"]["roles"]["required"]
                self.assertEqual(value, [baseline["roles"][field] for field in fields])
            else:
                self.assertEqual(value, [list(item) for item in baseline["roles"].items()])
        self.assertNotIsInstance(value, dict)


if __name__ == "__main__":
    unittest.main()
