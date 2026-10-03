import copy
import unittest

from check_hit_region_backend import cases, hit_region_mismatch


class HitRegionConformanceTests(unittest.TestCase):
    def test_subminimum_or_shifted_region_cannot_pass_with_tolerance(self):
        expected = cases()[0]["expected"]
        for field in ("x", "y", "width", "height"):
            actual = copy.deepcopy(expected)
            actual["bounds"][field] -= 1e-10
            self.assertIsNotNone(hit_region_mismatch(actual, expected))
        self.assertIsNone(hit_region_mismatch(copy.deepcopy(expected), expected))

    def test_case_requests_do_not_mutate_the_baseline(self):
        first = cases()
        first[0]["request"]["visualBounds"]["width"] = 100
        self.assertEqual(cases()[0]["request"]["visualBounds"]["width"], 20)


if __name__ == "__main__":
    unittest.main()
