import json
import math
import unittest

import spring_range_arithmetic as arithmetic
from check_schemas import ROOT


class RangeCases(unittest.TestCase):
    def test_the_committed_cases_are_what_the_seeded_oracle_derives(self):
        committed = json.loads((ROOT / "conformance/motion/spring-range-cases.json").read_text(encoding="utf-8"))
        self.assertEqual(committed, arithmetic.cases())

    def test_accepted_and_rejected_requests_are_both_represented(self):
        committed = arithmetic.cases()
        self.assertGreater(sum("expected" in case for case in committed), 100)
        self.assertGreater(sum("errorContains" in case for case in committed), 5)

    def test_every_seeded_hostile_request_is_accepted_or_rejected_never_an_exception(self):
        for request in arithmetic.hostile_requests(arithmetic.SEED + 1, 200):
            result, reason = arithmetic.verdict(request)
            self.assertTrue((result is None) != (reason is None))

    def test_a_decayed_state_is_accepted_although_its_initial_energy_is_not_representable(self):
        request = arithmetic.named_request(1e308, 0.0, 100.0, 1.0, 1600.0, 80.0)
        result, _ = arithmetic.verdict(request)
        self.assertTrue(result["settled"])
        undecayed = arithmetic.named_request(1e308, 0.0, 0.0, 1.0, 1600.0, 80.0)
        self.assertIsNone(arithmetic.verdict(undecayed)[0])

    def test_cancellation_dominated_results_are_not_expected_values(self):
        request = arithmetic.named_request(-9.4e45, 1.2e122, 3.1e99, 6.6e-294, 9.5e-296, 7e-65, -3.6e235)
        self.assertFalse(arithmetic.well_conditioned(request))

    def test_the_equation_matches_the_closed_form_in_each_damping_regime(self):
        root = math.sqrt(3)
        slow, fast = -2 + root, -2 - root
        weight = (0 - fast) / (slow - fast)
        for damping, expected in (
            (0.0, math.cos(1.0)),
            (2.0, 2 / math.e),
            (4.0, weight * math.exp(slow) + (1 - weight) * math.exp(fast)),
        ):
            request = arithmetic.named_request(1.0, 0.0, 1.0, 1.0, 1.0, damping)
            request["dynamics"]["positionThreshold"] = 1e-300
            position = float(arithmetic.trajectory(request["dynamics"], request["initial"], 0.0, 1.0)[0])
            self.assertAlmostEqual(position, expected, places=12)


if __name__ == "__main__":
    unittest.main()
