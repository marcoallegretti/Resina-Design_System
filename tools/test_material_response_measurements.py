import unittest

from check_schemas import ROOT, load_json
from material_response_measurements import measure


class ResponseMeasurementTests(unittest.TestCase):
    def test_measurement_distinguishes_monotone_and_controlled_peak(self):
        frost = measure(load_json(ROOT / "definitions/motion/frost-depth.json"))
        elastomer = measure(load_json(ROOT / "definitions/motion/elastomer-depth.json"))
        self.assertTrue(frost["monotoneUnitStep"])
        self.assertEqual(frost["peakPosition"], 1)
        self.assertFalse(elastomer["monotoneUnitStep"])
        self.assertGreater(elastomer["peakPosition"], 1.04)
        self.assertLess(elastomer["peakPosition"], 1.05)
        for report in (frost, elastomer):
            self.assertEqual(len(report["retargets"]), 6)
            self.assertTrue(all(record["retainedState"]["velocity"] != 0
                                for record in report["retargets"]))

    def test_unsettled_response_is_rejected_instead_of_reporting_a_false_bound(self):
        dynamics = load_json(ROOT / "definitions/motion/elastomer-depth.json")
        dynamics["damping"] = 0
        with self.assertRaisesRegex(ValueError, "did not respond and settle"):
            measure(dynamics)


if __name__ == "__main__":
    unittest.main()
