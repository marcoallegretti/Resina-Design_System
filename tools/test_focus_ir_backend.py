import copy
import math
import unittest

from check_focus_ir_backend import focus_ir_mismatch
from check_schemas import ROOT, load_json


class FocusIrComparisonTests(unittest.TestCase):
    def setUp(self):
        self.expected = load_json(ROOT / "conformance/ir/focus-ir-expected.json")

    def test_geometry_allows_relative_tolerance_but_unit_radials_do_not(self):
        self.expected["geometry"]["outer"]["contour"]["bounds"]["width"] = 1e300
        actual = copy.deepcopy(self.expected)
        actual["geometry"]["outer"]["contour"]["bounds"]["width"] = math.nextafter(1e300, math.inf)
        self.assertIsNone(focus_ir_mismatch(actual, self.expected))
        actual["geometry"]["outer"]["contour"]["segments"][0]["start"]["x"] -= 2e-12
        self.assertEqual(
            focus_ir_mismatch(actual, self.expected),
            "/geometry/outer/contour/segments/0/start/x",
        )

    def test_color_and_state_evidence_is_not_hidden_by_geometry(self):
        actual = copy.deepcopy(self.expected)
        actual["indicator"]["color"]["components"][0] = 0.9
        self.assertEqual(focus_ir_mismatch(actual, self.expected), "/indicator/color/components/0")
        actual = copy.deepcopy(self.expected)
        actual["indicator"]["binding"]["states"]["states"] = ["focused"]
        self.assertEqual(focus_ir_mismatch(actual, self.expected), "/indicator/binding/states/states")
        self.assertEqual(focus_ir_mismatch([], self.expected), "/")
        actual = copy.deepcopy(self.expected)
        actual.pop("geometry")
        self.assertEqual(focus_ir_mismatch(actual, self.expected), "/geometry")


if __name__ == "__main__":
    unittest.main()
