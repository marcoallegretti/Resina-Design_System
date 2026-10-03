import copy
import math
import unittest

from check_key_light_backend import key_light_mismatch


class KeyLightComparisonTests(unittest.TestCase):
    def setUp(self):
        self.expected = {
            "sideOffset": {"x": 1e300, "y": 0},
            "direction": {"x": -1, "y": 0},
            "normalHighlightWeights": [1, 0],
        }

    def test_offset_allows_rounding_at_large_magnitudes(self):
        actual = copy.deepcopy(self.expected)
        actual["sideOffset"]["x"] = math.nextafter(1e300, math.inf)
        self.assertIsNone(key_light_mismatch(actual, self.expected))
        actual["sideOffset"]["x"] = 1.01e300
        self.assertEqual(key_light_mismatch(actual, self.expected), "/sideOffset/x")

    def test_normalized_values_keep_absolute_tolerance(self):
        actual = copy.deepcopy(self.expected)
        actual["normalHighlightWeights"][0] += 1e-6
        self.assertEqual(key_light_mismatch(actual, self.expected), "/normalHighlightWeights/0")
        actual = copy.deepcopy(self.expected)
        actual["sideOffset"]["y"] = 1e-6
        self.assertEqual(key_light_mismatch(actual, self.expected), "/sideOffset/y")

    def test_structure_and_finite_number_requirements_are_exact(self):
        for value in [True, "0", math.inf, math.nan]:
            actual = copy.deepcopy(self.expected)
            actual["sideOffset"]["y"] = value
            self.assertEqual(key_light_mismatch(actual, self.expected), "/sideOffset/y")
        actual = copy.deepcopy(self.expected)
        actual["sideOffset"]["z"] = 0
        self.assertEqual(key_light_mismatch(actual, self.expected), "/sideOffset")
        actual = copy.deepcopy(self.expected)
        actual["extra"] = 0
        self.assertEqual(key_light_mismatch(actual, self.expected), "/")


if __name__ == "__main__":
    unittest.main()
