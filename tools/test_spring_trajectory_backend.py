import unittest
from copy import deepcopy

from check_spring_trajectory_backend import trajectory_mismatch


class TrajectoryComparisonTests(unittest.TestCase):
    def test_state_tolerance_preserves_target_and_metadata(self):
        expected = {"schemaVersion": "0.1.0", "representation": "spring", "target": 1.0,
                    "state": {"position": 0.5, "velocity": 0.2}, "settled": False}
        actual = deepcopy(expected)
        actual["state"]["position"] += 5e-10
        self.assertIsNone(trajectory_mismatch(actual, expected))
        for field, value in (("target", 1.0 + 1e-13), ("target", True),
                             ("settled", 0), ("representation", "immediate"), ("extra", 0)):
            self.assertIsNotNone(trajectory_mismatch({**expected, field: value}, expected))
        for field, value in (("position", float("nan")), ("velocity", float("inf")),
                             ("position", True), ("velocity", 0.2 + 2e-9)):
            actual = deepcopy(expected)
            actual["state"][field] = value
            self.assertIsNotNone(trajectory_mismatch(actual, expected))

    def test_settled_endpoint_is_exact_even_within_sample_tolerance(self):
        expected = {"schemaVersion": "0.1.0", "representation": "spring", "target": -2.0,
                    "state": {"position": -2.0, "velocity": 0.0}, "settled": True}
        self.assertIsNone(trajectory_mismatch(expected, expected))
        actual = deepcopy(expected)
        actual["state"]["position"] += 1e-10
        self.assertEqual(trajectory_mismatch(actual, expected), "/state/position")
