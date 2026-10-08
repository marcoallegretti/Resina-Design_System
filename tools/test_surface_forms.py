import copy
import unittest

from check_schemas import ROOT, apply_changes, load_json, validator_for
from check_surface_paint_backend import baseline as paint_baseline


PREFIX = "surface form source "


def malformed(form):
    return {
        PREFIX + "positional": ("", [form[field] for field in ("schemaVersion", "shape", "elevation")]),
        PREFIX + "tagged elevation": ("/elevation", {form["elevation"]: None}),
    }


class SurfaceFormSourceTests(unittest.TestCase):
    def test_form_vectors_follow_existing_object_and_string_schema(self):
        vectors = load_json(ROOT / "conformance/geometry/surface-form-vectors.json")
        original = vectors[0]["document"]
        validator = validator_for("schemas/surface-form.schema.json")
        schema = load_json(ROOT / "schemas/surface-form.schema.json")
        for shape in schema["properties"]["shape"]["enum"]:
            for elevation in schema["properties"]["elevation"]["enum"]:
                self.assertTrue(validator.is_valid({**original, "shape": shape, "elevation": elevation}))
                self.assertFalse(validator.is_valid({**original, "elevation": {elevation: None}}))
        cases = [case for case in vectors if case["name"].startswith(PREFIX)]
        expected = malformed(original)
        self.assertEqual(len(cases), len(expected))
        self.assertEqual({case["name"] for case in cases}, expected.keys())
        for case in cases:
            pointer, value = expected[case["name"]]
            document = {**original, pointer[1:]: value} if pointer else value
            self.assertEqual(case["document"], document)
            self.assertFalse(validator.is_valid(document))
            self.assertNotIn("expected", case)
            self.assertTrue(case["error"])

    def test_binding_vectors_isolate_the_nested_form(self):
        vectors = load_json(ROOT / "conformance/surfaces/binding-vectors.json")
        original = vectors[0]["document"]
        expected = malformed(original["form"])
        cases = [case for case in vectors if case["name"].startswith(PREFIX)]
        self.assertEqual(len(cases), len(expected))
        self.assertEqual({case["name"] for case in cases}, expected.keys())
        validator = validator_for("schemas/surface-binding.schema.json")
        self.assertTrue(validator.is_valid(original))
        for case in cases:
            pointer, value = expected[case["name"]]
            document = copy.deepcopy(original)
            document["form"] = {**original["form"], pointer[1:]: value} if pointer else value
            self.assertEqual(case["document"], document)
            self.assertFalse(validator.is_valid(document))
            self.assertNotIn("expected", case)
            self.assertTrue(case["error"])

    def test_ir_vectors_isolate_the_nested_form(self):
        consumers = [
            ("focus-ir", load_json(ROOT / "conformance/ir/focus-ir-request.json"), "/surface/form"),
            ("opaque-surface", load_json(ROOT / "conformance/ir/opaque-surface-request.json"), "/surface/form"),
            ("surface-paint", paint_baseline(), "/body/surface/form"),
        ]
        for consumer, original, prefix in consumers:
            form = original["body"]["surface"]["form"] if consumer == "surface-paint" else original["surface"]["form"]
            expected = malformed(form)
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
                    self.assertEqual(apply_changes(changed, [{"path": prefix, "value": form}]), original)


if __name__ == "__main__":
    unittest.main()
