import json
import unittest

from check_schemas import ROOT, apply_changes, load_json, validator_for
from check_theme_backend import cases as theme_cases


PREFIX = "material source shape "


class MaterialAssignmentBoundaryTests(unittest.TestCase):
    def test_model_vectors_cover_objects_and_string_families(self):
        vectors = load_json(ROOT / "conformance/materials/role-assignment-vectors.json")
        validator = validator_for("schemas/material-assignments.schema.json")
        invalid = [v for v in vectors if v["name"].startswith(PREFIX)]
        self.assertEqual(len(invalid), 20)
        baseline = vectors[0]["document"]
        self.assertEqual(baseline, load_json(ROOT / "conformance/headless/valid-request.json")
                         ["materialAssignments"])
        self.assertEqual(baseline, load_json(ROOT / "conformance/themes/valid-source.json")
                         ["materialAssignments"])
        for vector in vectors:
            with self.subTest(name=vector["name"]):
                self.assertEqual(validator.is_valid(vector["document"]), "expected" in vector)
                if "expected" in vector:
                    roles = {group + "." + role: family
                             for group in ("surface", "control", "feedback")
                             for role, family in vector["document"][group].items()}
                    self.assertEqual(len(roles), 11)
                    self.assertEqual(vector["expected"], roles)
        self.assertEqual(invalid[0]["document"],
                         [baseline[k] for k in ("schemaVersion", "surface", "control", "feedback")])
        for group, fields in {
            "surface": ["base", "content", "chrome", "raised", "transient"],
            "control": ["passive", "interactive", "primary"],
            "feedback": ["focus", "selection", "drag"],
        }.items():
            positional = next(v["document"] for v in invalid
                              if v["name"] == PREFIX + "positional " + group)
            self.assertEqual(positional[group], [baseline[group][k] for k in fields])
            for role in fields:
                tagged = next(v["document"] for v in invalid
                              if v["name"] == PREFIX + "tagged " + group + "." + role)
                self.assertEqual(tagged[group][role], {baseline[group][role]: None})

    def test_public_consumers_mutate_only_the_material_source_shape(self):
        resolution = load_json(ROOT / "conformance/headless/valid-request.json")
        material = resolution["materialAssignments"]
        vectors = {v["name"]: v["document"] for v in
                   load_json(ROOT / "conformance/materials/role-assignment-vectors.json")
                   if v["name"].startswith(PREFIX)}
        surface = load_json(ROOT / "conformance/surfaces/binding-vectors.json")[0]["document"]
        for path, schema, baseline, pointer in [
            ("conformance/headless/backend-cases.json", "schemas/headless-resolution.schema.json",
             resolution, "/materialAssignments"),
            ("conformance/surfaces/scenario-cases.json", "schemas/surface-scenario.schema.json",
             {"schemaVersion": "0.4.0", "resolution": resolution, "surface": surface},
             "/resolution/materialAssignments"),
        ]:
            cases = [c for c in load_json(ROOT / path) if c["name"].startswith(PREFIX)]
            self.assertEqual(len(cases), 20)
            self.assertEqual({c["name"] for c in cases}, set(vectors))
            validator = validator_for(schema)
            self.assertTrue(validator.is_valid(baseline))
            for case in cases:
                with self.subTest(path=path, name=case["name"]):
                    self.assertEqual(case["outcome"], "invalid")
                    self.assertEqual(len(case["requestChanges"]), 1)
                    self.assertTrue(case["requestChanges"][0]["path"].startswith(pointer))
                    request = apply_changes(baseline, case["requestChanges"])
                    self.assertFalse(validator.is_valid(request))
                    changed = request["materialAssignments"] if pointer == "/materialAssignments" \
                        else request["resolution"]["materialAssignments"]
                    self.assertEqual(changed, vectors[case["name"]])
                    restored = apply_changes(request, [{"path": pointer, "value": material}])
                    self.assertEqual(restored, baseline)

        public = {c["name"]: c for c in load_json(ROOT / "conformance/themes/resolution-cases.json")}
        cases = [(name, json.loads(source), expected) for name, source, expected in theme_cases()
                 if name.startswith(PREFIX)]
        self.assertEqual(len(cases), 24)
        self.assertEqual({public[name].get("sourceVariant") for name, _, _ in cases},
                         {None, "resolverBackedInline", "foundationResolver"})
        source_validator = validator_for("schemas/theme-source.schema.json")
        request_validator = validator_for("schemas/theme-resolution-request.schema.json")
        for name, request, expected in cases:
            with self.subTest(name=name):
                self.assertIsNone(expected)
                self.assertTrue(request_validator.is_valid(request))
                theme = json.loads(request["themeSource"])
                variant = public[name].get("sourceVariant")
                original_name = name.replace(PREFIX + variant + " ", PREFIX, 1) if variant else name
                self.assertEqual(theme["materialAssignments"], vectors[original_name])
                self.assertFalse(source_validator.is_valid(theme))
                theme["materialAssignments"] = material
                self.assertTrue(source_validator.is_valid(theme))


if __name__ == "__main__":
    unittest.main()
