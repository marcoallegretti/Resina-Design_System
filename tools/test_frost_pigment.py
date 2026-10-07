import json
import unittest

from check_schemas import ROOT, apply_changes, load_json, validator_for
from check_theme_backend import cases as theme_cases


class FrostPigmentBoundaryTests(unittest.TestCase):
    def test_non_object_vectors_match_the_pigment_schema(self):
        vectors = load_json(ROOT / "conformance/materials/frost-pigment-vectors.json")
        invalid = [vector for vector in vectors
                   if vector["name"].startswith("non-object Frost pigment ")]
        self.assertEqual(len(invalid), 6)
        validator = validator_for("schemas/frost-pigment.schema.json")
        for vector in invalid:
            with self.subTest(name=vector["name"]):
                self.assertFalse(validator.is_valid(vector["document"]))
                self.assertIn("error", vector)
        positional = next(vector["document"] for vector in invalid
                          if vector["name"].endswith("positional record"))
        pigment = load_json(ROOT / "conformance/headless/valid-request.json")["frostPigment"]
        self.assertEqual(positional, [pigment["schemaVersion"], pigment["tintStrength"]])

    def test_public_consumers_reject_only_the_changed_pigment(self):
        resolution = load_json(ROOT / "conformance/headless/valid-request.json")
        surface = load_json(ROOT / "conformance/surfaces/binding-vectors.json")[0]["document"]
        for path, schema, base, pointer in [
            ("conformance/headless/backend-cases.json", "schemas/headless-resolution.schema.json",
             resolution, "/frostPigment"),
            ("conformance/surfaces/scenario-cases.json", "schemas/surface-scenario.schema.json",
             {"schemaVersion": "0.4.0", "resolution": resolution, "surface": surface},
             "/resolution/frostPigment"),
        ]:
            public = load_json(ROOT / path)
            invalid = [case for case in public if case["name"].startswith("non-object Frost pigment ")]
            self.assertEqual(len(invalid), 6)
            validator = validator_for(schema)
            self.assertTrue(validator.is_valid(base))
            for case in invalid:
                with self.subTest(path=path, name=case["name"]):
                    self.assertEqual(case["outcome"], "invalid")
                    self.assertEqual(len(case["requestChanges"]), 1)
                    self.assertEqual(case["requestChanges"][0]["path"], pointer)
                    self.assertFalse(validator.is_valid(apply_changes(base, case["requestChanges"])))

        public = {case["name"]: case for case in
                  load_json(ROOT / "conformance/themes/resolution-cases.json")}
        invalid = [(name, json.loads(source), expected) for name, source, expected in theme_cases()
                   if name.startswith("non-object Frost pigment ")]
        self.assertEqual(len(invalid), 8)
        self.assertEqual({public[name].get("sourceVariant") for name, _, _ in invalid},
                         {None, "resolverBackedInline", "foundationResolver"})
        source_validator = validator_for("schemas/theme-source.schema.json")
        request_validator = validator_for("schemas/theme-resolution-request.schema.json")
        for name, request, expected in invalid:
            with self.subTest(name=name):
                self.assertIsNone(expected)
                self.assertTrue(request_validator.is_valid(request))
                theme = json.loads(request["themeSource"])
                self.assertFalse(source_validator.is_valid(theme))
                theme["frostPigment"] = resolution["frostPigment"]
                self.assertTrue(source_validator.is_valid(theme))


if __name__ == "__main__":
    unittest.main()
