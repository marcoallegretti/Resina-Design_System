import copy
import json
import unittest
from types import SimpleNamespace
from unittest.mock import patch

from check_color_guard_backend import check_success
from check_schemas import ROOT, load_json, validator_for


class ToggleStateProtocolTests(unittest.TestCase):
    def setUp(self):
        self.cases = load_json(ROOT / "conformance/interaction/toggle-states-cases.json")
        self.case = next(case for case in self.cases if case["name"] == "disabled focused hover / on")
        self.result_validator = validator_for("schemas/state-set.schema.json")

    def assert_output_rejected(self, actual):
        self.assertTrue(self.result_validator.is_valid(actual))
        completed = SimpleNamespace(returncode=0, stderr="", stdout=json.dumps(actual))
        with patch("check_color_guard_backend.run_backend", return_value=completed):
            with self.assertRaisesRegex(AssertionError, "output differs at /states"):
                check_success(["test-backend"], json.dumps(self.case["request"]),
                              self.case["expected"], self.result_validator, 1, self.case["name"])

    def test_checker_rejects_loss_of_selection_focus_or_availability(self):
        for signal in ("checked", "focused", "disabled"):
            with self.subTest(signal=signal):
                actual = copy.deepcopy(self.case["expected"])
                actual["states"].remove(signal)
                self.assert_output_rejected(actual)

    def test_checker_rejects_selection_substitution_and_noncanonical_order(self):
        actual = copy.deepcopy(self.case["expected"])
        actual["states"][actual["states"].index("checked")] = "selected"
        self.assert_output_rejected(actual)
        actual = copy.deepcopy(self.case["expected"])
        actual["states"].reverse()
        self.assert_output_rejected(actual)

    def test_request_requires_independent_boolean_checked_and_hover(self):
        validator = validator_for("schemas/toggle-states-request.schema.json")
        for field in ("checked", "hovered"):
            for value in (None, 1, "true"):
                with self.subTest(field=field, value=value):
                    document = {**self.case["request"], field: value}
                    self.assertFalse(validator.is_valid(document))
            document = copy.deepcopy(self.case["request"])
            del document[field]
            self.assertFalse(validator.is_valid(document))


if __name__ == "__main__":
    unittest.main()
