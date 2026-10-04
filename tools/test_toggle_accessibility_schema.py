import copy
import unittest
from check_schemas import ROOT, load_json, validator_for


class ToggleAccessibilitySchemaTests(unittest.TestCase):
    def test_all_dimensions_and_binary_checked_are_required(self):
        validator = validator_for("schemas/toggle-accessibility-ir.schema.json")
        baseline = load_json(ROOT / "conformance/accessibility/toggle-cases.json")[0]["expected"]
        self.assertTrue(validator.is_valid(baseline))
        for field in baseline:
            document = copy.deepcopy(baseline)
            del document[field]
            with self.subTest(field=field):
                self.assertFalse(validator.is_valid(document))
        for field in baseline["state"]:
            document = copy.deepcopy(baseline)
            del document["state"][field]
            self.assertFalse(validator.is_valid(document))
        for checked in (None, 1, "mixed", "true"):
            document = copy.deepcopy(baseline)
            document["state"]["checked"] = checked
            self.assertFalse(validator.is_valid(document))
        for role in ("button", "checkbox", "radio"):
            document = copy.deepcopy(baseline)
            document["role"] = role
            self.assertFalse(validator.is_valid(document))

    def test_checked_does_not_override_availability_or_focus_constraints(self):
        validator = validator_for("schemas/toggle-accessibility-ir.schema.json")
        baseline = load_json(ROOT / "conformance/accessibility/toggle-cases.json")[0]["expected"]
        for checked in (False, True):
            for enabled in (False, True):
                for focused in (False, True):
                    for focusable in (False, True):
                        document = copy.deepcopy(baseline)
                        document["state"] = dict(checked=checked, enabled=enabled, focused=focused)
                        document["focusable"] = focusable
                        document["actions"][0]["available"] = enabled
                        with self.subTest(state=document["state"], focusable=focusable):
                            self.assertEqual(validator.is_valid(document), focusable or not (enabled or focused))
                        document["actions"][0]["available"] = not enabled
                        self.assertFalse(validator.is_valid(document))


if __name__ == "__main__":
    unittest.main()
