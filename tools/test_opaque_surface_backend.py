import copy
import math
import unittest

from check_opaque_surface_backend import opaque_surface_mismatch
from check_schemas import ROOT, load_json


class OpaqueSurfaceComparisonTests(unittest.TestCase):
    def setUp(self):
        self.expected = load_json(ROOT / "conformance/ir/opaque-surface-expected.json")

    def test_navigation_state_cannot_be_dropped_inferred_or_replaced(self):
        for expected_states, actual_states in (
            (["rest", "focused"], ["rest"]),
            (["focused"], ["rest", "focused"]),
            (["focused"], ["checked"]),
        ):
            with self.subTest(expected=expected_states, actual=actual_states):
                self.expected["states"]["states"] = expected_states
                actual = copy.deepcopy(self.expected)
                actual["states"]["states"] = actual_states
                self.assertIsNotNone(opaque_surface_mismatch(actual, self.expected))

    def test_geometry_and_offset_keep_their_numeric_policies(self):
        self.expected["geometry"]["content"]["offset"]["x"] = 1e300
        actual = copy.deepcopy(self.expected)
        actual["geometry"]["content"]["offset"]["x"] = math.nextafter(1e300, math.inf)
        self.assertIsNone(opaque_surface_mismatch(actual, self.expected))
        actual["geometry"]["content"]["offset"]["x"] = 1.01e300
        self.assertEqual(opaque_surface_mismatch(actual, self.expected), "/geometry/content/offset/x")

    def test_color_and_structure_cannot_be_hidden_by_geometry_comparison(self):
        actual = copy.deepcopy(self.expected)
        actual["pigment"]["body"]["components"][0] = .9
        self.assertEqual(opaque_surface_mismatch(actual, self.expected), "/pigment/body/components/0")
        actual = copy.deepcopy(self.expected)
        actual["extra"] = "unexpected"
        self.assertEqual(opaque_surface_mismatch(actual, self.expected), "/")
        self.assertEqual(opaque_surface_mismatch([], self.expected), "/")
        actual = copy.deepcopy(self.expected)
        actual.pop("lighting")
        self.assertEqual(opaque_surface_mismatch(actual, self.expected), "/lighting")


if __name__ == "__main__":
    unittest.main()
