import copy
import unittest

from check_schemas import ROOT, apply_changes, load_json, validator_for
from check_surface_paint_backend import baseline as paint_baseline


PREFIX = "treatment stack source "


def malformed(stack):
    tagged = copy.deepcopy(stack["treatments"])
    tagged[0] = {tagged[0]: None}
    return {
        PREFIX + "positional": ("", [stack["schemaVersion"], stack["treatments"]]),
        PREFIX + "tagged item": ("/treatments", tagged),
    }


class TreatmentStackSourceTests(unittest.TestCase):
    def test_treatment_vectors_match_the_existing_schema(self):
        names = load_json(ROOT / "schemas/treatment-stack.schema.json")["properties"]["treatments"]["items"]["enum"]
        expected = {PREFIX + "positional": ["0.1.0", ["none"]]}
        expected.update({PREFIX + "tagged " + name: {"schemaVersion": "0.1.0", "treatments": [{name: None}]}
                         for name in names})
        cases = [case for case in load_json(ROOT / "conformance/materials/treatment-stack-vectors.json")
                 if case["name"].startswith(PREFIX)]
        self.assertEqual(len(cases), len(expected))
        self.assertEqual({case["name"] for case in cases}, expected.keys())
        validator = validator_for("schemas/treatment-stack.schema.json")
        for name in names:
            self.assertTrue(validator.is_valid({"schemaVersion": "0.1.0", "treatments": [name]}))
        for case in cases:
            self.assertEqual(case["document"], expected[case["name"]])
            self.assertFalse(validator.is_valid(case["document"]))
            self.assertTrue(case["error"])
            self.assertNotIn("expected", case)

    def test_binding_vectors_isolate_the_treatment_stack_encoding(self):
        vectors = load_json(ROOT / "conformance/surfaces/binding-vectors.json")
        original = vectors[0]["document"]
        expected = malformed(original["treatmentStack"])
        cases = [case for case in vectors if case["name"].startswith(PREFIX)]
        self.assertEqual(len(cases), len(expected))
        self.assertEqual({case["name"] for case in cases}, expected.keys())
        validator = validator_for("schemas/surface-binding.schema.json")
        self.assertTrue(validator.is_valid(original))
        for case in cases:
            pointer, value = expected[case["name"]]
            document = copy.deepcopy(original)
            document["treatmentStack"] = {**original["treatmentStack"], pointer[1:]: value} if pointer else value
            self.assertEqual(case["document"], document)
            self.assertFalse(validator.is_valid(document))
            self.assertTrue(case["error"])
            self.assertNotIn("expected", case)

    def test_ir_vectors_change_only_the_intended_treatment_encoding(self):
        consumers = [
            ("focus-ir", load_json(ROOT / "conformance/ir/focus-ir-request.json"), "/surface/treatmentStack"),
            ("opaque-surface", load_json(ROOT / "conformance/ir/opaque-surface-request.json"), "/surface/treatmentStack"),
            ("surface-paint", paint_baseline(), "/body/surface/treatmentStack"),
        ]
        for consumer, original, prefix in consumers:
            stack = original["body"]["surface"]["treatmentStack"] if consumer == "surface-paint" else original["surface"]["treatmentStack"]
            expected = malformed(stack)
            cases = [case for case in load_json(ROOT / f"conformance/ir/{consumer}-cases.json")
                     if case["name"].startswith(PREFIX)]
            self.assertEqual(len(cases), len(expected))
            self.assertEqual({case["name"] for case in cases}, expected.keys())
            validator = validator_for(f"schemas/{consumer}-request.schema.json")
            case_validator = validator_for(f"schemas/{consumer}-case.schema.json")
            self.assertTrue(validator.is_valid(original))
            for case in cases:
                with self.subTest(consumer=consumer, name=case["name"]):
                    pointer, value = expected[case["name"]]
                    self.assertEqual(case["requestChanges"], [{"path": prefix + pointer, "value": value}])
                    self.assertFalse(case["requestSchemaValid"])
                    self.assertTrue(case.get("failure", False) or case.get("errorContains"))
                    self.assertTrue(case_validator.is_valid(case))
                    changed = apply_changes(original, case["requestChanges"])
                    self.assertFalse(validator.is_valid(changed))
                    self.assertEqual(apply_changes(changed, [{"path": prefix, "value": stack}]), original)


if __name__ == "__main__":
    unittest.main()
