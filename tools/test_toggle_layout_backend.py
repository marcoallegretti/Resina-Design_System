import copy
import json
import unittest
from types import SimpleNamespace
from unittest.mock import patch
from check_color_guard_backend import check_success
from check_schemas import ROOT, load_json, validator_for


class ToggleLayoutProtocolTests(unittest.TestCase):
    def test_checker_rejects_wrong_endpoint_and_changed_track(self):
        case = load_json(ROOT / "conformance/geometry/toggle-layout-cases.json")[1]
        validator = validator_for("schemas/toggle-layout-ir.schema.json")
        mutations = []
        wrong = copy.deepcopy(case["expected"])
        wrong["thumbBounds"] = wrong["offThumbBounds"]
        mutations.append(wrong)
        wrong = copy.deepcopy(case["expected"])
        wrong["trackBounds"]["width"] -= 1
        mutations.append(wrong)
        wrong = copy.deepcopy(case["expected"])
        wrong["onThumbBounds"]["x"] = wrong["offThumbBounds"]["x"]
        mutations.append(wrong)
        for actual in mutations:
            self.assertTrue(validator.is_valid(actual))
            output = SimpleNamespace(returncode=0, stderr="", stdout=json.dumps(actual))
            with patch("check_color_guard_backend.run_backend", return_value=output):
                with self.assertRaisesRegex(AssertionError, "output differs"):
                    check_success(["test-backend"], json.dumps(case["request"]), case["expected"], validator, 1, case["name"])

    def test_request_requires_complete_authored_inputs(self):
        request = load_json(ROOT / "conformance/geometry/toggle-layout-cases.json")[0]["request"]
        validator = validator_for("schemas/toggle-layout-request.schema.json")
        self.assertTrue(validator.is_valid(request))
        for field in request:
            document = copy.deepcopy(request)
            del document[field]
            self.assertFalse(validator.is_valid(document))
        for field in request["insets"]:
            document = copy.deepcopy(request)
            del document["insets"][field]
            self.assertFalse(validator.is_valid(document))


if __name__ == "__main__":
    unittest.main()
