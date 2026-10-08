import copy
import unittest

from check_schemas import ROOT, apply_changes, load_json, validator_for
from check_surface_paint_backend import baseline as paint_baseline


PREFIX = "surface intent form "
FIELDS = ["schemaVersion", "materialRole", "colorRole", "form", "states", "treatmentStack"]


def malformed(surface):
    return {
        PREFIX + "positional": ("", [surface[field] for field in FIELDS]),
        PREFIX + "tagged material role": ("/materialRole", {surface["materialRole"]: None}),
        PREFIX + "tagged color role": ("/colorRole", {surface["colorRole"]: None}),
    }


class SurfaceIntentBoundaryTests(unittest.TestCase):
    def test_binding_cases_reject_only_the_intended_source_forms(self):
        vectors = load_json(ROOT / "conformance/surfaces/binding-vectors.json")
        baseline = vectors[0]["document"]
        validator = validator_for("schemas/surface-binding.schema.json")
        self.assertTrue(validator.is_valid(baseline))
        cases = [case for case in vectors if case["name"].startswith(PREFIX)]
        expected = malformed(baseline)
        self.assertEqual(len(cases), len(expected))
        self.assertEqual({case["name"] for case in cases}, expected.keys())
        for case in cases:
            with self.subTest(name=case["name"]):
                pointer, value = expected[case["name"]]
                document = copy.deepcopy(baseline)
                if pointer:
                    document[pointer[1:]] = value
                else:
                    document = value
                self.assertEqual(case["document"], document)
                self.assertFalse(validator.is_valid(document))
                self.assertNotIn("expected", case)
                self.assertTrue(case["error"])
        schema = load_json(ROOT / "schemas/surface-binding.schema.json")
        for field in ["materialRole", "colorRole"]:
            for role in schema["properties"][field]["enum"]:
                self.assertTrue(validator.is_valid({**baseline, field: role}))
                self.assertFalse(validator.is_valid({**baseline, field: {role: None}}))

    def test_ir_cases_change_only_the_surface_form_under_test(self):
        consumers = [
            ("focus-ir", load_json(ROOT / "conformance/ir/focus-ir-request.json"), "/surface"),
            ("opaque-surface", load_json(ROOT / "conformance/ir/opaque-surface-request.json"), "/surface"),
            ("surface-paint", paint_baseline(), "/body/surface"),
        ]
        for consumer, original, prefix in consumers:
            surface = original["body"]["surface"] if consumer == "surface-paint" else original["surface"]
            expected = malformed(surface)
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
                    self.assertEqual(apply_changes(changed, [{"path": prefix, "value": surface}]), original)


if __name__ == "__main__":
    unittest.main()
