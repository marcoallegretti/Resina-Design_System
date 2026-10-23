import copy
import json
import unittest
from types import SimpleNamespace
from unittest.mock import patch

from check_color_guard_backend import check_success
from check_schemas import ROOT, load_json, validator_for
from toggle_profile_arithmetic import allocation, resolve, resolve_parts

CASES = ROOT / "conformance/layout/toggle-profile-resolution-cases.json"


def named(name):
    return next(case for case in load_json(CASES) if case["name"] == name)


class ToggleProfileProtocolTests(unittest.TestCase):
    def test_checker_rejects_wrong_results(self):
        case = named("compact text 2 coarse rtl")
        validator = validator_for("schemas/toggle-profile-ir.schema.json")
        mutations = []
        wrong = copy.deepcopy(case["expected"])
        wrong["trackSize"] = named("standard text 1 fine ltr")["expected"]["trackSize"]
        mutations.append(wrong)
        wrong = copy.deepcopy(case["expected"])
        wrong["thumbSize"]["width"] *= 2
        mutations.append(wrong)
        wrong = copy.deepcopy(case["expected"])
        wrong["insets"]["start"] += 1
        mutations.append(wrong)
        wrong = copy.deepcopy(case["expected"])
        wrong["layoutDirection"] = "ltr"
        mutations.append(wrong)
        for actual in mutations:
            self.assertTrue(validator.is_valid(actual))
            output = SimpleNamespace(returncode=0, stderr="", stdout=json.dumps(actual))
            with patch("check_color_guard_backend.run_backend", return_value=output):
                with self.assertRaisesRegex(AssertionError, "output differs"):
                    check_success(["test-backend"], json.dumps(case["request"]), case["expected"],
                                  validator, 1, case["name"])

    def test_request_requires_every_member(self):
        request = named("standard text 1 fine ltr")["request"]
        validator = validator_for("schemas/toggle-profile-request.schema.json")
        self.assertTrue(validator.is_valid(request))
        for field in request:
            document = copy.deepcopy(request)
            del document[field]
            self.assertFalse(validator.is_valid(document), field)

    def test_request_accepts_only_a_toggle_profile(self):
        request = named("command profile is rejected")["request"]
        self.assertFalse(validator_for("schemas/toggle-profile-request.schema.json").is_valid(request))
        self.assertTrue(validator_for("schemas/component-layout-profile.schema.json").is_valid(request["profile"]))

    def test_matrix_covers_every_density_text_input_and_direction(self):
        matrix = [case for case in load_json(CASES) if case["name"].endswith((" ltr", " rtl"))]
        self.assertEqual(len(matrix), 32)
        seen = set()
        for case in matrix:
            environment = case["request"]["environment"]
            seen.add((environment["densityPreference"], environment["textScale"],
                      "coarsePointer" in environment["inputCapabilities"], environment["layoutDirection"]))
        self.assertEqual(len(seen), 32)

    def test_oracle_never_scales_parts_with_text_or_changes_them_with_input(self):
        request = copy.deepcopy(named("standard text 1 fine ltr")["request"])
        baseline = resolve(request)
        for scale in (0.5, 2, 1e308):
            for inputs in (["finePointer"], ["coarsePointer"], ["directTouch"]):
                request["environment"]["textScale"] = scale
                request["environment"]["inputCapabilities"] = inputs
                self.assertEqual(resolve(request), baseline)

    def test_oracle_reports_the_first_failing_mode(self):
        request = copy.deepcopy(named("first failing mode is reported")["request"])
        with self.assertRaisesRegex(ValueError, "^Comfortable track and thumb cannot be allocated"):
            resolve(request)

    def test_oracle_rounds_the_allocation_like_binary64(self):
        track, thumb = {"width": 1e18, "height": 40}, {"width": 16, "height": 16}
        with self.assertRaisesRegex(ValueError, "representable arithmetic"):
            allocation(track, thumb, 12)
        allocation({"width": 1e15, "height": 40}, thumb, 12)
        parts = resolve_parts(named("standard text 1 fine ltr")["request"]["profile"],
                              named("standard text 1 fine ltr")["request"]["tokens"])
        self.assertEqual(parts["standard"][2], 12)


if __name__ == "__main__":
    unittest.main()
