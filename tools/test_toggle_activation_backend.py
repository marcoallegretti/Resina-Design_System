import copy
import json
import unittest
from types import SimpleNamespace
from unittest.mock import patch

from check_color_guard_backend import check_success
from check_schemas import ROOT, load_json, validator_for


class ToggleActivationProtocolTests(unittest.TestCase):
    def setUp(self):
        self.cases = load_json(ROOT / "conformance/interaction/toggle-activation-cases.json")
        self.result_validator = validator_for("schemas/toggle-activation-result.schema.json")

    def test_result_requires_binary_checked_and_complete_activation(self):
        baseline = self.cases[0]["expected"]
        self.assertTrue(self.result_validator.is_valid(baseline))
        for value in (None, "mixed", 1):
            with self.subTest(checked=value):
                document = {**baseline, "checked": value}
                self.assertFalse(self.result_validator.is_valid(document))
        for field in baseline:
            with self.subTest(missing=field):
                document = copy.deepcopy(baseline)
                del document[field]
                self.assertFalse(self.result_validator.is_valid(document))
        document = copy.deepcopy(baseline)
        del document["activation"]["state"]["hold"]
        self.assertFalse(self.result_validator.is_valid(document))

    def test_nested_activation_retains_availability_and_capture_guards(self):
        document = copy.deepcopy(self.cases[0]["expected"])
        document["activation"]["state"]["hold"] = None
        document["activation"]["pressed"] = False
        self.assertFalse(self.result_validator.is_valid(document))
        document["activation"]["capture"] = None
        self.assertTrue(self.result_validator.is_valid(document))
        document["activation"]["state"]["enabled"] = False
        document["activation"]["activate"] = True
        self.assertFalse(self.result_validator.is_valid(document))

    def test_checker_rejects_unchanged_value_on_accepted_toggle(self):
        case = next(case for case in self.cases if case["name"] == "inside release activates / off")
        actual = copy.deepcopy(case["expected"])
        actual["checked"] = case["request"]["checked"]
        completed = SimpleNamespace(returncode=0, stderr="", stdout=json.dumps(actual))
        with patch("check_color_guard_backend.run_backend", return_value=completed):
            with self.assertRaisesRegex(AssertionError, "output differs at /checked"):
                check_success(["test-backend"], json.dumps(case["request"]), case["expected"],
                              self.result_validator, 1, case["name"])


if __name__ == "__main__":
    unittest.main()
