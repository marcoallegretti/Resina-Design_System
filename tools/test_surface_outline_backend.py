import unittest

from check_surface_outline_backend import outline_mismatch


class OutlineComparison(unittest.TestCase):
    def test_geometry_tolerance_includes_relative_error_at_large_scales(self):
        self.assertIsNone(outline_mismatch({"x": 1e200 + 5e187}, {"x": 1e200}))
        self.assertEqual(outline_mismatch({"x": 1e200 + 1e190}, {"x": 1e200}), "/x")

    def test_structure_curvature_and_invalid_numbers_remain_strict(self):
        self.assertIsNotNone(outline_mismatch({"x": 1, "extra": 0}, {"x": 1}))
        self.assertIsNotNone(outline_mismatch({"curvature": "circular"}, {"curvature": "quartic"}))
        self.assertIsNotNone(outline_mismatch({"x": float("nan")}, {"x": 1}))
        self.assertIsNotNone(outline_mismatch({"x": True}, {"x": 1}))


if __name__ == "__main__":
    unittest.main()
