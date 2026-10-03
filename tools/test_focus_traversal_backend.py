import copy
import unittest

from check_focus_traversal_backend import cases, focus_traversal_mismatch
from check_schemas import validator_for


class FocusTraversalConformanceTests(unittest.TestCase):
    def test_no_target_is_explicit_and_schema_strict(self):
        validator = validator_for("schemas/focus-traversal-result.schema.json")
        expected = {"schemaVersion": "0.1.0", "targetId": None}
        self.assertFalse(list(validator.iter_errors(expected)))
        for actual in (
            {"schemaVersion": "0.1.0"},
            {**expected, "targetId": ""},
            {**expected, "targetId": False},
            {**expected, "selected": True},
        ):
            self.assertTrue(list(validator.iter_errors(actual)))
            self.assertIsNotNone(focus_traversal_mismatch(actual, expected))

    def test_changed_target_or_unicode_normalization_cannot_pass(self):
        case = next(case for case in cases() if case["name"] == "distinct unicode sequences")
        expected = case["expected"]
        self.assertIsNone(focus_traversal_mismatch(copy.deepcopy(expected), expected))
        self.assertIsNotNone(focus_traversal_mismatch({**expected, "targetId": "é"}, expected))
        self.assertIsNotNone(focus_traversal_mismatch({**expected, "targetId": None}, expected))


if __name__ == "__main__":
    unittest.main()
