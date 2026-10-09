import copy
import json
import subprocess
import unittest
from unittest.mock import patch

from check_schemas import ROOT, apply_changes, load_json, validator_for
from check_slider_part_paint_backend import check_success, expected_result, range_contrast


class SliderPartPaintProtocolTests(unittest.TestCase):
    def setUp(self):
        self.request = load_json(ROOT / "conformance/ir/slider-part-paint-request.json")
        self.validator = validator_for("schemas/slider-part-paint-ir.schema.json")

    def check_output(self, request, output):
        completed = subprocess.CompletedProcess([], 0, json.dumps(output), "")
        with patch("check_slider_part_paint_backend.run_backend", return_value=completed):
            check_success(["backend"], request, 1, "mutated output")

    def test_checker_rejects_schema_valid_wrong_channels_and_context(self):
        self.request["part"] = "thumb"
        self.request["surface"]["states"]["states"] = ["hover", "focused"]
        expected = expected_result(self.request)
        self.assertTrue(self.validator.is_valid(expected))
        self.check_output(self.request, expected)
        mutations = [
            ("/layoutDirection", "ltr"),
            ("/response/bodyMix", 0.125),
            ("/paint/body/colorRole", "selection"),
            ("/paint/body/contentContrastRatio", 21),
            ("/paint/body/pigment/body/components", [1, 1, 1]),
            ("/paint/body/pigment/side/components", [0, 0, 0]),
            ("/paint/body/pigment/highlight/components", [1, 1, 1]),
            ("/paint/body/edge/contrastRatio", 3),
            ("/paint/body/lighting/sideOffset/y", 2),
            ("/paint/body/geometry/front/bounds/width", 21),
            ("/paint/focus/indicator/binding/colorRole", "selection"),
            ("/paint/focus/indicator/binding/opaqueColorFallback/components", [1, 1, 1]),
            ("/paint/focus/geometry/outer/contour/bounds/width", 29),
        ]
        for pointer, value in mutations:
            with self.subTest(pointer=pointer):
                actual = apply_changes(expected, [{"path": pointer, "value": value}])
                self.assertTrue(self.validator.is_valid(actual))
                with self.assertRaisesRegex(AssertionError, "output differs"):
                    self.check_output(self.request, actual)

    def test_ir_schema_enforces_every_public_phase_case_and_focus_owner(self):
        cases = load_json(ROOT / "conformance/interaction/slider-phase-cases.json")
        for part in ("track", "thumb"):
            for case in cases:
                with self.subTest(part=part, case=case["name"]):
                    request = copy.deepcopy(self.request)
                    request.update(part=part, readOnly=case["readOnly"])
                    request["surface"]["states"] = case["states"]
                    output = expected_result(request)
                    output["paint"]["body"]["states"] = case["states"]
                    self.assertEqual(self.validator.is_valid(output), "expected" in case)
                    if "expected" in case:
                        self.assertEqual(output["phase"], case["expected"])
                        self.assertEqual("focus" in output["paint"], part == "thumb" and "focused" in case["states"]["states"])
                        output["phase"] = "disabled" if output["phase"] != "disabled" else "rest"
                        self.assertFalse(self.validator.is_valid(output))

    def test_focus_is_required_only_for_focused_thumb(self):
        request_validator = validator_for("schemas/slider-part-paint-request.schema.json")
        self.request.pop("surroundingRanges")
        self.request["surface"]["states"]["states"] = ["rest", "focused"]
        self.assertTrue(request_validator.is_valid(self.request))
        self.assertTrue(self.validator.is_valid(expected_result(self.request)))
        self.request["part"] = "thumb"
        self.assertFalse(request_validator.is_valid(self.request))
        self.request["surroundingRanges"] = load_json(ROOT / "conformance/ir/slider-part-paint-request.json")["surroundingRanges"]
        output = expected_result(self.request)
        self.assertTrue(self.validator.is_valid(output))
        output["paint"].pop("focus")
        self.assertFalse(self.validator.is_valid(output))

    def test_range_contrast_checks_interior_luminance_and_all_ranges(self):
        gray = dict(colorSpace="srgb", components=[0.5] * 3, alpha=1)
        black = dict(gray, components=[0] * 3)
        white = dict(gray, components=[1] * 3)
        self.assertEqual(range_contrast(gray, [dict(lower=black, upper=white)]), 1)
        self.assertEqual(range_contrast(black, [dict(lower=white, upper=white)]), 21)
        self.assertEqual(range_contrast(black, [dict(lower=white, upper=white), dict(lower=black, upper=black)]), 1)

    def test_frost_keeps_original_navigation_binding_and_final_body_fallback(self):
        for case in load_json(ROOT / "conformance/ir/slider-part-paint-cases.json"):
            if "legibility fallback" not in case["name"]:
                continue
            output = expected_result(apply_changes(self.request, case["requestChanges"]))
            self.assertTrue(self.validator.is_valid(output))
            body = output["paint"]["body"]
            binding = output["paint"]["focus"]["indicator"]["binding"]
            self.assertTrue(body["contentFallbackApplied"])
            self.assertEqual(body["frostRepresentation"], "opaqueDimensional")
            self.assertEqual(binding["frostRepresentation"], case["name"].split()[0])
            self.assertEqual(binding["frostPortableBody"]["alpha"], 0.55)
            output["paint"]["body"]["contentFallbackApplied"] = False
            self.assertTrue(self.validator.is_valid(output))
            with self.assertRaisesRegex(AssertionError, "output differs"):
                self.check_output(apply_changes(self.request, case["requestChanges"]), output)

    def test_rest_requires_identity_response(self):
        output = expected_result(self.request)
        output["response"]["bodyMix"] = 0.01
        self.assertFalse(self.validator.is_valid(output))
        output["response"] = dict(bodyMix=0, depthScale=0.9)
        self.assertFalse(self.validator.is_valid(output))

    def test_frost_response_precedes_compositing_with_actual_backdrop(self):
        cases = load_json(ROOT / "conformance/ir/slider-part-paint-cases.json")
        checked = 0
        for case in cases:
            if not any(phase in case["name"] for phase in (" hover ", " dragging ")):
                continue
            if not any(case["name"].startswith(mode) for mode in ("translucentPigmented", "regularBackdrop", "shapedBackdrop")):
                continue
            request = apply_changes(self.request, case["requestChanges"])
            checked += 1
            if case.get("failure"):
                with self.assertRaisesRegex(ValueError, "actual opaque Frost representation"):
                    expected_result(request)
            else:
                output = expected_result(request)
                self.assertTrue(output["paint"]["body"]["contentFallbackApplied"])
                self.assertTrue(self.validator.is_valid(output))
        self.assertEqual(checked, 12)


if __name__ == "__main__":
    unittest.main()
