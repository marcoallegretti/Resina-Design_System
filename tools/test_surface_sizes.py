import copy
import unittest

from check_schemas import ROOT, apply_changes, load_json, validator_for
from check_surface_paint_backend import baseline as paint_baseline


PREFIX = "surface size form "
LABELS = ["positional", "null", "boolean", "number", "string", "empty array",
          "missing width", "missing height", "unknown member"]
LABELS += [field + " " + form for field in ["width", "height"]
           for form in ["null", "boolean", "string", "array", "object"]]


def malformed(original, label):
    if label == "positional":
        return [original["width"], original["height"]]
    values = {"null": None, "boolean": True, "number": 1, "string": "size", "empty array": []}
    if label in values:
        return values[label]
    result = copy.deepcopy(original)
    if label.startswith("missing "):
        del result[label.split()[1]]
    elif label == "unknown member":
        result["depth"] = 1
    else:
        field, form = label.split()
        result[field] = {"null": None, "boolean": True, "string": "1", "array": [], "object": {}}[form]
    return result


class SurfaceSizeBoundaryTests(unittest.TestCase):
    def test_model_cases_match_the_normative_size_schema(self):
        cases = load_json(ROOT / "conformance/geometry/surface-size-vectors.json")
        validator = validator_for("schemas/surface-size.schema.json")
        metadata = validator_for("schemas/surface-size-case.schema.json")
        original = {"width": 200, "height": 80}
        invalid = [c for c in cases if "error" in c]
        self.assertEqual(len(cases), 25)
        self.assertEqual(len(invalid), 19)
        self.assertEqual({c["name"] for c in invalid}, {PREFIX + label for label in LABELS})
        for case in cases:
            with self.subTest(name=case["name"]):
                self.assertTrue(metadata.is_valid(case))
                self.assertEqual(validator.is_valid(case["document"]), "expected" in case)
                if "error" in case:
                    self.assertEqual(case["document"], malformed(original, case["name"].removeprefix(PREFIX)))
        for case in invalid:
            self.assertFalse(metadata.is_valid({**case, "expected": "valid"}))
        success = cases[0]
        self.assertFalse(metadata.is_valid({**success, "error": "invalid size"}))
        for invalid in [None, True, 1, "size", [], [200, 80]]:
            self.assertFalse(metadata.is_valid({**success, "document": invalid}))

    def test_public_cases_change_only_the_size_representation(self):
        vectors = {v["name"]: v for v in load_json(ROOT / "conformance/geometry/surface-size-vectors.json")
                   if "error" in v}
        shape = load_json(ROOT / "conformance/geometry/shape-fallback-source-cases.json")[0]["request"]
        validator = validator_for("schemas/shape-fallback-request.schema.json")
        self.assertTrue(validator.is_valid(shape))
        for case in vectors.values():
            self.assertFalse(validator.is_valid({**shape, "size": case["document"]}))
        for consumer, original, pointer, path in [
            ("focus-ir", load_json(ROOT / "conformance/ir/focus-ir-request.json"), "/size", "conformance/ir/focus-ir-cases.json"),
            ("opaque-surface", load_json(ROOT / "conformance/ir/opaque-surface-request.json"), "/size", "conformance/ir/opaque-surface-cases.json"),
            ("surface-paint", paint_baseline(), "/body/size", "conformance/ir/surface-paint-cases.json"),
            ("hit-region", load_json(ROOT / "conformance/interaction/hit-region-request.json"), "/componentMinimum", "conformance/interaction/hit-region-cases.json"),
            ("inset-contour", load_json(ROOT / "conformance/geometry/inset-contour-vectors.json")[0]["request"], "/size", "conformance/geometry/inset-contour-vectors.json"),
            ("extruded-contour", load_json(ROOT / "conformance/geometry/extruded-contour-vectors.json")[0]["request"], "/size", "conformance/geometry/extruded-contour-vectors.json"),
        ]:
            cases = [c for c in load_json(ROOT / path) if c["name"].startswith(PREFIX)]
            self.assertEqual(len(cases), 19)
            self.assertEqual({c["name"] for c in cases}, vectors.keys())
            validator = validator_for(f"schemas/{consumer}-request.schema.json")
            metadata = validator_for(f"schemas/{consumer}-case.schema.json")
            self.assertTrue(validator.is_valid(original))
            size = original
            for part in pointer[1:].split("/"):
                size = size[part]
            for case in cases:
                with self.subTest(consumer=consumer, name=case["name"]):
                    changed = malformed(size, case["name"].removeprefix(PREFIX))
                    expected = apply_changes(original, [{"path": pointer, "value": changed}])
                    if "request" in case:
                        self.assertEqual(case["request"], expected)
                    else:
                        self.assertEqual(case["requestChanges"], [{"path": pointer, "value": changed}])
                    self.assertFalse(case["requestSchemaValid"])
                    self.assertTrue(case.get("failure", False) or case.get("errorContains") == vectors[case["name"]]["error"])
                    self.assertTrue(metadata.is_valid(case))
                    self.assertFalse(validator.is_valid(expected))
                    self.assertEqual(apply_changes(expected, [{"path": pointer, "value": size}]), original)


if __name__ == "__main__":
    unittest.main()
