import unittest

from check_schemas import ROOT, apply_changes, load_json, validator_for
from check_surface_paint_backend import baseline as paint_baseline


PREFIX = "shape intent form "


class ShapeIntentBoundaryTests(unittest.TestCase):
    def test_model_cases_cover_canonical_string_representations(self):
        vectors = load_json(ROOT / "conformance/geometry/surface-form-vectors.json")
        schema = load_json(ROOT / "schemas/surface-form.schema.json")
        shapes = schema["properties"]["shape"]["enum"]
        expected = {PREFIX + "tagged " + shape: {shape: None} for shape in shapes}
        expected.update({PREFIX + name: value for name, value in [
            ("null", None), ("boolean", True), ("number", 1), ("empty array", []),
            ("empty object", {}), ("positional", ["structural"]), ("empty string", ""),
            ("uppercase", "Structural"), ("trailing whitespace", "structural "),
        ]})
        cases = [v for v in vectors if v["name"].startswith(PREFIX)]
        self.assertEqual(len(cases), 14)
        self.assertEqual({c["name"] for c in cases}, expected.keys())
        validator = validator_for("schemas/surface-form.schema.json")
        request_validator = validator_for("schemas/shape-fallback-request.schema.json")
        request = load_json(ROOT / "conformance/geometry/shape-fallback-source-cases.json")[0]["request"]
        self.assertTrue(request_validator.is_valid(request))
        for case in cases:
            with self.subTest(name=case["name"]):
                self.assertEqual(case["document"], {
                    "schemaVersion": "0.1.0", "shape": expected[case["name"]], "elevation": "raised",
                })
                self.assertFalse(validator.is_valid(case["document"]))
                self.assertNotIn("expected", case)
                self.assertTrue(case["error"])
                self.assertTrue(validator.is_valid({**case["document"], "shape": "structural"}))
                self.assertFalse(request_validator.is_valid({**request, "shape": expected[case["name"]]}))
        for shape in shapes:
            self.assertTrue(validator.is_valid({"schemaVersion": "0.1.0", "shape": shape, "elevation": "raised"}))
            self.assertTrue(request_validator.is_valid({**request, "shape": shape}))

    def test_ir_cases_change_only_the_shape_intent(self):
        vectors = {v["name"]: v for v in load_json(ROOT / "conformance/geometry/surface-form-vectors.json")
                   if v["name"].startswith(PREFIX)}
        consumers = [
            ("focus-ir", load_json(ROOT / "conformance/ir/focus-ir-request.json"), "/surface/form/shape"),
            ("opaque-surface", load_json(ROOT / "conformance/ir/opaque-surface-request.json"), "/surface/form/shape"),
            ("surface-paint", paint_baseline(), "/body/surface/form/shape"),
        ]
        for consumer, original, pointer in consumers:
            cases = [c for c in load_json(ROOT / f"conformance/ir/{consumer}-cases.json")
                     if c["name"].startswith(PREFIX)]
            self.assertEqual(len(cases), 14)
            self.assertEqual({c["name"] for c in cases}, vectors.keys())
            validator = validator_for(f"schemas/{consumer}-request.schema.json")
            case_validator = validator_for(f"schemas/{consumer}-case.schema.json")
            self.assertTrue(validator.is_valid(original))
            for case in cases:
                with self.subTest(consumer=consumer, name=case["name"]):
                    value = vectors[case["name"]]["document"]["shape"]
                    self.assertEqual(case["requestChanges"], [{"path": pointer, "value": value}])
                    self.assertFalse(case["requestSchemaValid"])
                    self.assertTrue(case.get("failure", False) or case.get("errorContains") == vectors[case["name"]]["error"])
                    self.assertTrue(case_validator.is_valid(case))
                    changed = apply_changes(original, case["requestChanges"])
                    self.assertFalse(validator.is_valid(changed))
                    self.assertEqual(apply_changes(changed, [{"path": pointer, "value": "structural"}]), original)


if __name__ == "__main__":
    unittest.main()
