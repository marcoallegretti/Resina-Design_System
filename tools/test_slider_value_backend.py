import copy
import math
import unittest

from check_schemas import ROOT, load_json, validator_for
from check_slider_value_backend import slider_value_mismatch


class SliderValueProtocolTests(unittest.TestCase):
    def setUp(self):
        self.cases = load_json(ROOT / "conformance/interaction/slider-value-cases.json")
        self.schema = validator_for("schemas/slider-value-ir.schema.json")

    def test_checker_rejects_changed_values_and_false_endpoints(self):
        expected = next(case["expected"] for case in self.cases if case["name"] == "large near upper endpoint")
        self.assertIsNone(slider_value_mismatch(expected, expected))
        for field, value in [("minimum", 0), ("maximum", 1), ("value", 0),
                             ("progress", 1), ("progress", 0), ("progress", True),
                             ("progress", expected["progress"] - 8 * math.ulp(expected["progress"]))]:
            actual = {**expected, field: value}
            with self.subTest(field=field, value=value):
                self.assertEqual(slider_value_mismatch(actual, expected), "/" + field)
        for case in self.cases:
            if "expected" in case and case["expected"]["progress"] in (0, 1):
                target = case["expected"]
                actual = {**target, "progress": math.nextafter(target["progress"], 0.5)}
                self.assertEqual(slider_value_mismatch(actual, target), "/progress")

    def test_schema_requires_complete_numeric_ir_without_backend_metadata(self):
        baseline = self.cases[0]["expected"]
        self.assertTrue(self.schema.is_valid(baseline))
        for field in baseline:
            document = copy.deepcopy(baseline)
            del document[field]
            self.assertFalse(self.schema.is_valid(document))
        for changes in [{"progress": -0.1}, {"progress": 1.1}, {"value": None},
                        {"value": True}, {"backend": "guido"}, {"schemaVersion": "0.2.0"}]:
            self.assertFalse(self.schema.is_valid({**baseline, **changes}))


if __name__ == "__main__":
    unittest.main()
