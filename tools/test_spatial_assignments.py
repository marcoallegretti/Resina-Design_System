import json
import unittest

from check_schemas import ROOT, apply_changes, load_json, validator_for
from check_theme_backend import cases as theme_cases, theme_source


PREFIX = "spatial source shape "


class SpatialAssignmentBoundaryTests(unittest.TestCase):
    def test_model_vectors_require_objects_and_preserve_roles(self):
        vectors = load_json(ROOT / "conformance/spatial/assignment-vectors.json")
        validator = validator_for("schemas/spatial-assignments.schema.json")
        invalid = [v for v in vectors if v["name"].startswith(PREFIX)]
        self.assertEqual(len(invalid), 12)
        for vector in vectors:
            with self.subTest(name=vector["name"]):
                self.assertEqual(validator.is_valid(vector["document"]), "expected" in vector)
                if "expected" in vector:
                    self.assertEqual(vector["document"]["roles"], vector["expected"])
        original = vectors[0]["document"]
        for group in ["root", "roles"]:
            shapes = [v for v in invalid if f"{group} " in v["name"]]
            self.assertEqual(len(shapes), 6)
            positional = next(v for v in shapes if v["name"].endswith("positional"))
            expected = [original["schemaVersion"], original["roles"]] if group == "root" else \
                {**original, "roles": [list(item) for item in original["roles"].items()]}
            self.assertEqual(positional["document"], expected)

    def test_public_cases_change_only_spatial_shapes(self):
        vectors = {v["name"] for v in load_json(ROOT / "conformance/spatial/assignment-vectors.json")
                   if v["name"].startswith(PREFIX)}
        baseline = load_json(ROOT / "conformance/headless/valid-request.json")
        surface = load_json(ROOT / "conformance/surfaces/binding-vectors.json")[0]["document"]
        for path, schema, original, prefix in [
            ("conformance/headless/backend-cases.json", "schemas/headless-resolution.schema.json", baseline, ""),
            ("conformance/surfaces/scenario-cases.json", "schemas/surface-scenario.schema.json",
             {"schemaVersion": "0.4.0", "resolution": baseline, "surface": surface}, "/resolution"),
        ]:
            cases = [c for c in load_json(ROOT / path) if c["name"].startswith(PREFIX)]
            self.assertEqual(len(cases), 12)
            self.assertEqual({c["name"] for c in cases}, vectors)
            validator = validator_for(schema)
            self.assertTrue(validator.is_valid(original))
            for case in cases:
                with self.subTest(path=path, name=case["name"]):
                    self.assertEqual(case["outcome"], "invalid")
                    self.assertEqual(len(case["requestChanges"]), 1)
                    change = case["requestChanges"][0]
                    self.assertEqual(change["path"], prefix + self.assignment_pointer(case["name"]))
                    request = apply_changes(original, case["requestChanges"])
                    self.assertFalse(validator.is_valid(request))
                    restored = apply_changes(request, [{"path": prefix + "/spatialAssignments",
                                                        "value": baseline["spatialAssignments"]}])
                    self.assertEqual(restored, original)
                    self.check_shape(case["name"], change["value"], baseline["spatialAssignments"])
        public = {c["name"]: c for c in load_json(ROOT / "conformance/themes/resolution-cases.json")}
        themes = [(name, source, expected) for name, source, expected in theme_cases() if name.startswith(PREFIX)]
        self.assertEqual(len(themes), 14)
        self.assertEqual({name for name, _, _ in themes if name in vectors}, vectors)
        variants = set()
        base_text = (ROOT / "conformance/themes/valid-source.json").read_text(encoding="utf-8")
        foundation_text = (ROOT / "tokens/foundation.json").read_text(encoding="utf-8")
        for name, source, expected in themes:
            with self.subTest(name=name):
                case = public[name]
                self.assertIsNone(expected)
                self.assertEqual(case["outcome"], "invalid")
                self.assertTrue(case["requestSchemaValid"])
                self.assertEqual(len(case["sourceChanges"]), 1)
                self.assertNotIn("requestChanges", case)
                self.assertNotIn("sourceReplace", case)
                change = case["sourceChanges"][0]
                self.assertEqual(change["path"], self.assignment_pointer(name))
                request = json.loads(source)
                self.assertTrue(validator_for("schemas/theme-resolution-request.schema.json").is_valid(request))
                theme = json.loads(request["themeSource"])
                validator = validator_for("schemas/theme-source.schema.json")
                self.assertFalse(validator.is_valid(theme))
                unchanged, sources = theme_source({k: v for k, v in case.items() if k != "sourceChanges"},
                                                 base_text, foundation_text)
                original = json.loads(unchanged)
                restored = apply_changes(theme, [{"path": "/spatialAssignments",
                                                  "value": original["spatialAssignments"]}])
                self.assertEqual(restored, original)
                self.assertEqual(request["externalSources"], sources)
                self.assertTrue(validator.is_valid(restored))
                self.check_shape(name, change["value"], original["spatialAssignments"])
                variants.add(case.get("sourceVariant", "embedded"))
        self.assertEqual(variants, {"embedded", "resolverBackedInline", "foundationResolver"})

    def assignment_pointer(self, name):
        return "/spatialAssignments" + ("/roles" if "roles " in name else "")

    def check_shape(self, name, value, baseline):
        if "positional" in name:
            expected = [baseline["schemaVersion"], baseline["roles"]] if "root " in name else \
                [list(item) for item in baseline["roles"].items()]
            self.assertEqual(value, expected)
        self.assertNotIsInstance(value, dict)


if __name__ == "__main__":
    unittest.main()
