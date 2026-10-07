import copy
import unittest

from check_activation_backend import activation_mismatch, cases
from check_schemas import validator_for


class ActivationProtocolTests(unittest.TestCase):
    def test_alternate_record_and_key_shapes_fail_the_request_schema(self):
        invalid = [case for case in cases() if case["name"].startswith("invalid JSON shape")]
        self.assertEqual(len(invalid), 11)
        case_validator = validator_for("schemas/activation-case.schema.json")
        request_validator = validator_for("schemas/activation-request.schema.json")
        for case in invalid:
            with self.subTest(name=case["name"]):
                self.assertTrue(case_validator.is_valid(case))
                self.assertFalse(request_validator.is_valid(case["request"]))
                self.assertFalse(case["requestSchemaValid"])
                self.assertTrue(case["failure"])

    def test_missing_null_hold_and_invalid_state_invariants_fail(self):
        validator = validator_for("schemas/activation-state.schema.json")
        state = copy.deepcopy(cases()[0]["request"]["state"])
        self.assertTrue(validator.is_valid(state))
        del state["hold"]
        self.assertFalse(validator.is_valid(state))
        state["hold"] = {"kind": "key", "key": "space"}
        state["focused"] = False
        self.assertFalse(validator.is_valid(state))

    def test_pressed_signal_must_match_retained_hold(self):
        validator = validator_for("schemas/activation-result.schema.json")
        for case in cases():
            if "expected" not in case:
                continue
            result = copy.deepcopy(case["expected"])
            self.assertTrue(validator.is_valid(result), case["name"])
            result["pressed"] = not result["pressed"]
            self.assertFalse(validator.is_valid(result), case["name"])

    def test_mismatch_detects_false_activation_and_lost_capture(self):
        expected = cases()[0]["expected"]
        for field, value in [("activate", True), ("capture", None)]:
            actual = {**expected, field: value}
            self.assertEqual(activation_mismatch(actual, expected), "/")


if __name__ == "__main__":
    unittest.main()
