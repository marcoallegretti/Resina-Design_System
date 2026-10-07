import copy
import math
import unittest

from check_schemas import ROOT, load_json, validator_for
from check_slider_adjustment_backend import adjustment_mismatch
from check_slider_position_backend import position_expected


class SliderPositionProtocolTests(unittest.TestCase):
    def setUp(self):
        self.cases = load_json(ROOT / "conformance/interaction/slider-position-protocol-cases.json")

    def test_successful_vectors_follow_rational_mapping_and_permission(self):
        for case in self.cases:
            if "expected" in case:
                with self.subTest(name=case["name"]):
                    self.assertEqual(case["expected"], position_expected(case["request"]))

    def test_schema_requires_current_authored_layout_and_rejects_resolved_state(self):
        validator = validator_for("schemas/slider-position-request.schema.json")
        baseline = self.cases[0]["request"]
        self.assertTrue(validator.is_valid(baseline))
        for field in baseline:
            document = copy.deepcopy(baseline)
            del document[field]
            self.assertFalse(validator.is_valid(document))
        for field, value in [("progress", 0.25), ("backend", "guido")]:
            document = copy.deepcopy(baseline)
            document["layout"]["value"][field] = value
            self.assertFalse(validator.is_valid(document))

    def test_oracle_preserves_numeric_intent_under_mirroring_and_scaling(self):
        for case in self.cases:
            if "expected" not in case or not case["name"].startswith(("horizontal ", "vertical ")):
                continue
            request = copy.deepcopy(case["request"])
            layout = request["layout"]
            layout["layoutDirection"] = "rtl" if layout["layoutDirection"] == "ltr" else "ltr"
            if layout["orientation"] == "horizontal":
                request["desiredOrigin"] = (layout["allocationSize"]["width"]
                                            - layout["thumbSize"]["width"] - request["desiredOrigin"])
            for field in ("allocationSize", "thumbSize", "insets"):
                layout[field] = {key: value * 2 for key, value in layout[field].items()}
            layout["trackThickness"] *= 2
            request["desiredOrigin"] *= 2
            self.assertEqual(position_expected(request), case["expected"])

    def test_checker_rejects_numeric_drift_and_incorrect_acceptance_or_change(self):
        for case in self.cases:
            if "expected" not in case:
                continue
            expected = case["expected"]
            for field in ("accepted", "changed"):
                actual = copy.deepcopy(expected)
                actual[field] = not actual[field]
                self.assertEqual(adjustment_mismatch(actual, expected), "/" + field)
            actual = copy.deepcopy(expected)
            actual["value"]["value"] = math.nextafter(actual["value"]["value"], math.inf)
            self.assertEqual(adjustment_mismatch(actual, expected), "/value/value")


if __name__ == "__main__":
    unittest.main()
