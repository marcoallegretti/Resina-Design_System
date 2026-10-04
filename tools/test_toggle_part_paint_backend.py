import copy
import json
import unittest
from check_schemas import ROOT, load_json, validator_for
from check_toggle_part_paint_backend import toggle_part_mismatch


class TogglePartPaintProtocolTests(unittest.TestCase):
    def setUp(self):
        self.request = load_json(ROOT / "conformance/ir/toggle-part-paint-request.json")
        self.request["part"] = "thumb"
        theme = json.loads(self.request["surface"]["body"]["theme"]["themeSource"])
        theme["materialAssignments"]["control"]["interactive"] = "cast"
        self.request["surface"]["body"]["theme"]["themeSource"] = json.dumps(theme)
        body = load_json(ROOT / "conformance/ir/opaque-surface-expected.json")
        body["materialRole"] = "control.interactive"
        color = [0.8, 0.7, 0.6]
        body["pigment"]["body"]["components"] = color
        body["pigment"]["side"]["components"] = [c * 0.9 for c in color]
        body["pigment"]["highlight"]["components"] = [0.08 + c * 0.92 for c in color]
        linear = [c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4 for c in color]
        body["contentContrastRatio"] = (sum(c * w for c, w in zip(linear, (0.2126, 0.7152, 0.0722))) + 0.05) / 0.05
        self.output = dict(schemaVersion="0.1.0", part="thumb", checked=False, phase="rest",
                           response=dict(bodyMix=0, depthScale=1), paint=dict(schemaVersion="0.1.0", body=body))
        self.validator = validator_for("schemas/toggle-part-paint-ir.schema.json")

    def test_checker_rejects_schema_valid_wrong_selection_role_and_pigment(self):
        self.assertTrue(self.validator.is_valid(self.output))
        self.assertIsNone(toggle_part_mismatch(self.output, self.request))
        for field, value in (("colorRole", "selection"), ("contentContrastRatio", 21)):
            actual = copy.deepcopy(self.output)
            actual["paint"]["body"][field] = value
            self.assertTrue(self.validator.is_valid(actual))
            self.assertIsNotNone(toggle_part_mismatch(actual, self.request))
        actual = copy.deepcopy(self.output)
        actual["paint"]["body"]["pigment"]["body"]["components"] = [1, 1, 1]
        self.assertTrue(self.validator.is_valid(actual))
        self.assertEqual(toggle_part_mismatch(actual, self.request), "/paint/body/pigment/body")

    def test_checker_rejects_schema_valid_changed_body_bounds(self):
        actual = copy.deepcopy(self.output)
        actual["paint"]["body"]["geometry"]["front"]["bounds"]["width"] += 1
        self.assertTrue(self.validator.is_valid(actual))
        self.assertEqual(toggle_part_mismatch(actual, self.request), "/paint/body/geometry/front/bounds")

    def test_checked_and_thumb_focus_are_structural_invariants(self):
        actual = copy.deepcopy(self.output)
        actual["checked"] = True
        self.assertFalse(self.validator.is_valid(actual))
        actual["paint"]["body"]["states"]["states"].append("checked")
        self.assertTrue(self.validator.is_valid(actual))
        actual["paint"]["focus"] = load_json(ROOT / "conformance/ir/focus-ir-expected.json")
        self.assertFalse(self.validator.is_valid(actual))


if __name__ == "__main__":
    unittest.main()
