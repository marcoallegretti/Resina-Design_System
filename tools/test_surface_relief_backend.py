import unittest

from check_surface_relief_backend import relief_mismatch


class ReliefComparisonTests(unittest.TestCase):
    def test_relative_tolerance_reaches_ordered_samples_without_weakening_classification(self):
        expected = {"samples": [{"point": {"x": 1e200}, "sample": None}]}
        self.assertIsNone(relief_mismatch(
            {"samples": [{"point": {"x": 1e200 + 1e188 / 2}, "sample": None}]}, expected))
        self.assertIsNotNone(relief_mismatch(
            {"samples": [{"point": {"x": 1e200}, "sample": {"height": 0}}]}, expected))
        self.assertIsNotNone(relief_mismatch({"samples": []}, expected))
        self.assertIsNotNone(relief_mismatch({"samples": [{"point": {"x": True}, "sample": None}]}, expected))
