import copy
import json
import unittest
from decimal import Decimal

from check_material_light_backend import baseline, compare, source_cases
from material_light_arithmetic import expected_result
from check_schemas import validator_for


class MaterialLightProofTests(unittest.TestCase):
    def test_authored_signed_weights_and_exact_plateau(self):
        request = baseline()
        result = expected_result(request)
        self.assertEqual([sample["weight"] for sample in result["samples"]], [0.6, -0.6, 0])
        self.assertEqual(result["samples"][2]["color"], request["pigment"]["body"])
        request["light"]["ambient"] = 0.5
        request["light"]["interaction"] = {"direction": {"x": -1, "y": 0}, "strength": 0.25}
        result = expected_result(request)
        self.assertAlmostEqual(result["samples"][0]["weight"], 0.15, places=15)
        for actual, expected in zip(result["samples"][0]["color"]["components"], (0.4072, 0.506, 0.6048)):
            self.assertAlmostEqual(actual, expected, places=15)
        self.assertEqual(result["samples"][2]["color"], request["pigment"]["body"])

    def test_policy_and_physical_direction_are_independent(self):
        request = baseline()
        request["light"]["interaction"] = {"direction": {"x": -1, "y": 0}, "strength": 0.25}
        result = expected_result(request)
        request["environment"]["layoutDirection"] = "rtl"
        self.assertEqual(expected_result(request), result)
        request["interactionActive"] = False
        self.assertIsNone(expected_result(request)["interaction"])
        self.assertEqual(expected_result(request)["samples"][0]["weight"], 0.6)

    def test_checker_detects_planar_renormalization_and_inward_ranges(self):
        expected = expected_result(baseline())
        invalid = copy.deepcopy(expected)
        invalid["samples"][0]["weight"] = 1
        self.assertIsNotNone(compare(invalid, expected))
        invalid = copy.deepcopy(expected)
        invalid["samples"][2]["color"]["components"][0] += 1e-14
        self.assertIsNotNone(compare(invalid, expected))
        invalid = copy.deepcopy(expected)
        invalid["colorRanges"][0]["lower"]["components"][0] = expected["pigment"]["body"]["components"][0]
        self.assertIsNotNone(compare(invalid, expected))

    def test_source_cases_include_invalid_suppressed_and_valid_zero(self):
        failures, successes = source_cases()
        self.assertTrue(any("suppressed True" in name and "negative" in name for name, _ in failures))
        self.assertTrue(any("-0e-400" in name for name, _, _ in successes))

    def test_checker_rejects_coherent_retained_body_drift(self):
        _, successes = source_cases()
        _, source, expected = successes[0]
        cases = (("authored", baseline(), expected_result(baseline())),
                 ("generated", json.loads(source), expected))
        for name, request, expected in cases:
            with self.subTest(name=name):
                changed = copy.deepcopy(request)
                changed["pigment"]["body"]["components"][0] += 1e-14
                self.assertIsNotNone(compare(expected_result(changed), expected))

    def test_negative_sources_violate_exact_schema_minimum(self):
        validator = validator_for("schemas/material-light-request.schema.json")
        failures, _ = source_cases()
        sources = [source for name, source in failures if "negative" in name and "/light/" in name]
        self.assertEqual(len(sources), 8)
        for source in sources:
            request = json.loads(source, parse_float=Decimal)
            errors = list(validator.iter_errors(request))
            self.assertEqual(len(errors), 1)
            self.assertIn(errors[0].validator, ("minimum", "oneOf"))


if __name__ == "__main__":
    unittest.main()
