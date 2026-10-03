import copy
import math
import unittest

from check_extruded_contour_backend import contour_mismatch


class ContourComparisonTests(unittest.TestCase):
    def setUp(self):
        self.expected = {
            "segments": [
                {"kind": "arc", "center": {"x": 1e300, "y": 0}, "start": {"x": 1, "y": 0}}
            ]
        }

    def test_coordinates_allow_large_magnitude_rounding(self):
        actual = copy.deepcopy(self.expected)
        actual["segments"][0]["center"]["x"] = math.nextafter(1e300, math.inf)
        self.assertIsNone(contour_mismatch(actual, self.expected))
        actual["segments"][0]["center"]["x"] = 1.01e300
        self.assertEqual(contour_mismatch(actual, self.expected), "/segments/0/center/x")

    def test_radials_keep_absolute_tolerance(self):
        actual = copy.deepcopy(self.expected)
        actual["segments"][0]["start"]["x"] += 1.5e-12
        self.assertEqual(contour_mismatch(actual, self.expected), "/segments/0/start/x")

    def test_structure_and_numbers_fail_explicitly(self):
        for value in [True, "0", math.inf, math.nan]:
            actual = copy.deepcopy(self.expected)
            actual["segments"][0]["center"]["y"] = value
            self.assertEqual(contour_mismatch(actual, self.expected), "/segments/0/center/y")
        actual = copy.deepcopy(self.expected)
        actual["segments"].append(actual["segments"][0])
        self.assertEqual(contour_mismatch(actual, self.expected), "/segments")
        self.assertEqual(contour_mismatch({**self.expected, "extra": 0}, self.expected), "/")


if __name__ == "__main__":
    unittest.main()
