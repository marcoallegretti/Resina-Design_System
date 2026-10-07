import copy
import math
import unittest

from check_schemas import ROOT, load_json, validator_for
from check_slider_adjustment_backend import adjustment_expected, adjustment_mismatch


class SliderAdjustmentProtocolTests(unittest.TestCase):
    def setUp(self):
        self.cases = load_json(ROOT / "conformance/interaction/slider-adjustment-protocol-cases.json")
        self.request = self.cases[0]["request"]

    def test_successful_vectors_follow_exact_rational_intents_and_live_permission(self):
        for case in self.cases:
            if "expected" in case:
                with self.subTest(name=case["name"]):
                    self.assertEqual(case["expected"], adjustment_expected(case["request"]))

    def test_checker_rejects_false_commit_permission_and_numeric_results(self):
        expected = self.cases[0]["expected"]
        self.assertIsNone(adjustment_mismatch(expected, expected))
        for field, value in [("accepted", False), ("changed", False), ("schemaVersion", "0.2.0")]:
            self.assertEqual(adjustment_mismatch({**expected, field: value}, expected), "/" + field)
        for field, value in [("minimum", 0), ("maximum", 1), ("value", 0),
                             ("progress", 0), ("progress", 1),
                             ("progress", expected["value"]["progress"] + 8 * math.ulp(expected["value"]["progress"]))]:
            actual = copy.deepcopy(expected)
            actual["value"][field] = value
            self.assertEqual(adjustment_mismatch(actual, expected), "/value/" + field)

    def test_request_schema_requires_complete_input_and_exact_intent_shape(self):
        schema = validator_for("schemas/slider-adjustment-request.schema.json")
        self.assertTrue(schema.is_valid(self.request))
        for field in self.request:
            document = copy.deepcopy(self.request)
            del document[field]
            self.assertFalse(schema.is_valid(document))
        for adjustment in [{"kind": "minimum", "amount": 1}, {"kind": "maximum", "value": 1},
                           {"kind": "increase"}, {"kind": "decrease", "amount": True},
                           {"kind": "setValue", "value": 10, "amount": 1}]:
            self.assertFalse(schema.is_valid({**self.request, "adjustment": adjustment}))
        self.assertFalse(schema.is_valid({**self.request, "backend": "guido"}))


if __name__ == "__main__":
    unittest.main()
