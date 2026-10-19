import copy
import json
import unittest
from types import SimpleNamespace
from unittest.mock import patch

from check_color_guard_backend import check_success
from check_schemas import ROOT, load_json, validator_for
from command_layout_arithmetic import resolve, resolve_extents

CASES = ROOT / "conformance/layout/command-layout-cases.json"


def named(name):
    return next(case for case in load_json(CASES) if case["name"] == name)


class CommandLayoutProtocolTests(unittest.TestCase):
    def test_checker_rejects_wrong_policies(self):
        case = named("compact text 2 coarse rtl")
        validator = validator_for("schemas/command-layout-ir.schema.json")
        mutations = []
        wrong = copy.deepcopy(case["expected"])
        wrong["targetMinimum"] = {"width": 24, "height": 24}
        mutations.append(wrong)
        wrong = copy.deepcopy(case["expected"])
        wrong["maximumSize"] = resolve_extents(case["request"]["profile"], case["request"]["tokens"])["compact"][1]
        mutations.append(wrong)
        wrong = copy.deepcopy(case["expected"])
        wrong["padding"]["start"] = case["request"]["spatialBinding"]["space.control.inline"]["value"]
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
        validator = validator_for("schemas/command-layout-request.schema.json")
        self.assertTrue(validator.is_valid(request))
        for field in request:
            document = copy.deepcopy(request)
            del document[field]
            self.assertFalse(validator.is_valid(document), field)

    def test_matrix_covers_every_density_text_input_and_direction(self):
        matrix = [case for case in load_json(CASES) if case["name"].endswith((" ltr", " rtl"))]
        self.assertEqual(len(matrix), 32)
        seen = set()
        for case in matrix:
            environment = case["request"]["environment"]
            coarse = "coarsePointer" in environment["inputCapabilities"]
            seen.add((environment["densityPreference"], environment["textScale"], coarse,
                      environment["layoutDirection"]))
            target = 48 if coarse else 24
            self.assertEqual(case["expected"]["targetMinimum"], {"width": target, "height": target})
            self.assertLessEqual(case["expected"]["minimumSize"]["height"],
                                 case["expected"]["maximumSize"]["height"])
        self.assertEqual(len(seen), 32)

    def test_oracle_keeps_minimum_and_scales_only_enlarged_text(self):
        request = copy.deepcopy(named("standard text 1 fine ltr")["request"])
        minimum, maximum = resolve_extents(request["profile"], request["tokens"])["standard"]
        for text_scale, factor in ((0.5, 1), (1, 1), (1.5, 1.5), (2, 2)):
            request["environment"]["textScale"] = text_scale
            result = resolve(request)
            self.assertEqual(result["minimumSize"], minimum)
            self.assertEqual(result["maximumSize"]["width"], maximum["width"] * factor)

    def test_oracle_reports_an_overflowing_text_scale(self):
        request = copy.deepcopy(named("standard text 1 fine ltr")["request"])
        request["environment"]["textScale"] = 1e308
        self.assertTrue(validator_for("schemas/command-layout-request.schema.json").is_valid(request))
        with self.assertRaisesRegex(ValueError, "text-scaled maximum size is not finite"):
            resolve(request)

    def test_direct_touch_alone_selects_the_coarse_target(self):
        request = copy.deepcopy(named("compact text 1 fine ltr")["request"])
        for inputs, target in ((["directTouch"], 48), (["finePointer"], 24), (["coarsePointer"], 48)):
            request["environment"]["inputCapabilities"] = inputs
            self.assertEqual(resolve(request)["targetMinimum"], {"width": target, "height": target})


if __name__ == "__main__":
    unittest.main()
