import unittest

from check_spring_backend import spring_mismatch


class SpringComparisonTests(unittest.TestCase):
    def test_numeric_tolerance_does_not_relax_structure_or_flags(self):
        expected = {"position": 1.0, "velocity": 1e6, "settled": False}
        self.assertIsNone(spring_mismatch({**expected, "position": 1.0 + 5e-10}, expected))
        self.assertIsNone(spring_mismatch({**expected, "velocity": 1e6 + 5e-4}, expected))
        for field, value in (
            ("position", True), ("position", float("nan")), ("velocity", float("inf")),
            ("position", 1.0 + 2e-9), ("velocity", 1e6 + 0.01), ("settled", 0),
        ):
            self.assertIsNotNone(spring_mismatch({**expected, field: value}, expected))
        self.assertIsNotNone(spring_mismatch({**expected, "extra": 0}, expected))
        self.assertIsNotNone(spring_mismatch({"position": 1}, expected))
