import copy
import json
import math
import subprocess
import unittest
from unittest.mock import patch

from check_schemas import ROOT, load_json, validator_for
from check_color_guard_backend import check_success
from check_slider_layout_backend import layout_expected, layout_mismatch


class SliderLayoutProtocolTests(unittest.TestCase):
    def setUp(self):
        self.cases = load_json(ROOT / "conformance/geometry/slider-layout-protocol-cases.json")

    def test_successful_vectors_follow_rational_geometry_for_both_axes_and_directions(self):
        for case in self.cases:
            if "expected" in case:
                with self.subTest(name=case["name"]):
                    self.assertEqual(case["expected"], layout_expected(case["request"]))
                    self.assertIsNone(layout_mismatch(case["expected"], case["expected"]))

    def test_checker_rejects_reversed_axis_lost_insets_and_shifted_endpoints(self):
        expected = next(case["expected"] for case in self.cases
                        if case.get("expected", {}).get("value", {}).get("progress") == 0.5)
        for field, coordinate, value in [("thumbBounds", "x", expected["minimumThumbBounds"]["x"]),
                                          ("thumbBounds", "width", 1),
                                          ("trackBounds", "x", 0),
                                          ("minimumThumbBounds", "x", 0),
                                          ("maximumThumbBounds", "x", 0)]:
            actual = copy.deepcopy(expected)
            actual[field][coordinate] = value
            self.assertIsNotNone(layout_mismatch(actual, expected))
        for field, value in [("orientation", "vertical"), ("layoutDirection", "rtl"),
                             ("minimumPosition", "end")]:
            self.assertEqual(layout_mismatch({**expected, field: value}, expected), "/" + field)
        endpoint = self.cases[0]["expected"]
        actual = copy.deepcopy(endpoint)
        actual["thumbBounds"]["x"] += 1e-13
        self.assertEqual(layout_mismatch(actual, endpoint), "/thumbBounds")

    def test_schema_requires_authored_geometry_and_rejects_backend_metadata(self):
        schema = validator_for("schemas/slider-layout-request.schema.json")
        baseline = self.cases[0]["request"]
        self.assertTrue(schema.is_valid(baseline))
        for field in baseline:
            document = copy.deepcopy(baseline)
            del document[field]
            self.assertFalse(schema.is_valid(document))
        for field in ("allocationSize", "thumbSize", "insets", "value"):
            document = copy.deepcopy(baseline)
            document[field]["backend"] = "guido"
            self.assertFalse(schema.is_valid(document))

    def test_large_geometry_accepts_relative_rounding_and_rejects_material_error(self):
        expected = next(case["expected"] for case in self.cases if case["name"] == "large binary64 geometry")
        actual = copy.deepcopy(expected)
        actual["thumbBounds"]["x"] = 2.4227500000000003e17
        self.assertIsNone(layout_mismatch(actual, expected))
        actual["thumbBounds"]["x"] *= 1.000001
        self.assertEqual(layout_mismatch(actual, expected), "/thumbBounds/x")

    def test_tiny_geometry_preserves_endpoint_order_and_distinction(self):
        expected = next(case["expected"] for case in self.cases if case["name"] == "tiny distinct travel")
        self.assertIsNone(layout_mismatch(expected, expected))
        for reversed_axis in [True, False]:
            actual = copy.deepcopy(expected)
            if reversed_axis:
                actual["minimumThumbBounds"], actual["maximumThumbBounds"] = (
                    actual["maximumThumbBounds"], actual["minimumThumbBounds"])
                actual["thumbBounds"] = actual["minimumThumbBounds"]
            else:
                actual["maximumThumbBounds"] = actual["minimumThumbBounds"]
            self.assertEqual(layout_mismatch(actual, expected), "/maximumThumbBounds/x")

    def test_large_thumb_preserves_distinct_endpoint_centers(self):
        expected = next(case["expected"] for case in self.cases
                        if case["name"] == "large thumb with narrow travel")
        actual = copy.deepcopy(expected)
        actual["maximumThumbBounds"]["x"] = math.nextafter(
            actual["minimumThumbBounds"]["x"], math.inf)
        self.assertLess(actual["minimumThumbBounds"]["x"], actual["maximumThumbBounds"]["x"])
        self.assertEqual(layout_mismatch(actual, expected), "/maximumThumbBounds/x")

    def test_tiny_geometry_rejects_lost_thickness_and_containment(self):
        case = next(case for case in self.cases if case["name"] == "tiny distinct travel")
        for field, coordinate, value in [("trackBounds", "height", 1e-13),
                                         ("trackBounds", "y", 8e-13)]:
            actual = copy.deepcopy(case["expected"])
            actual[field][coordinate] = value
            self.assertIsNotNone(layout_mismatch(actual, case["expected"], case["request"]))
        actual = copy.deepcopy(case["expected"])
        for field in ("trackBounds", "minimumThumbBounds", "maximumThumbBounds", "thumbBounds"):
            actual[field]["y"] = 8e-13
        self.assertEqual(layout_mismatch(actual, case["expected"], case["request"]), "/trackBounds")

    def test_checker_rejects_nondeterministic_results_within_tolerance(self):
        case = next(case for case in self.cases if case["name"] == "large binary64 geometry")
        changed = copy.deepcopy(case["expected"])
        changed["thumbBounds"]["x"] = math.nextafter(changed["thumbBounds"]["x"], math.inf)
        outputs = [subprocess.CompletedProcess([], 0, json.dumps(result), "")
                   for result in (case["expected"], changed)]
        with patch("check_color_guard_backend.run_backend", side_effect=outputs):
            with self.assertRaisesRegex(AssertionError, "different results"):
                check_success([], json.dumps(case["request"]), case["expected"],
                              validator_for("schemas/slider-layout-ir.schema.json"), 30,
                              case["name"], layout_mismatch)


if __name__ == "__main__":
    unittest.main()
