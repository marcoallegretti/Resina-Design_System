import json
import unittest

from check_schemas import ROOT, apply_changes, load_json, validator_for
from control_paint_source import positional_control_request


class ControlPaintSourceTests(unittest.TestCase):
    def test_full_positional_records_are_schema_invalid_without_other_changes(self):
        consumers = (
            ("command-paint", ("schemaVersion", "surface", "commandAppearance")),
            ("command-motion", ("schemaVersion", "surface", "commandAppearance", "channels", "time")),
            ("toggle-part-paint", ("schemaVersion", "part", "surface", "checkedColorRole", "interactionAppearance")),
            ("toggle-part-motion", ("schemaVersion", "surface", "interactionAppearance", "part", "checkedColorRole", "channels", "time")),
        )
        for consumer, members in consumers:
            with self.subTest(consumer=consumer):
                original = load_json(ROOT / f"conformance/ir/{consumer}-request.json")
                saved = json.dumps(original)
                validator = validator_for(f"schemas/{consumer}-request.schema.json")
                self.assertTrue(validator.is_valid(original))
                positional = json.loads(positional_control_request(original, members))
                self.assertEqual(len(positional), len(original))
                self.assertEqual(dict(zip(members, positional, strict=True)), original)
                self.assertFalse(validator.is_valid(positional))
                self.assertEqual(json.dumps(original), saved)

    def test_tagged_toggle_names_are_independently_rejected(self):
        for consumer in ("toggle-part-paint", "toggle-part-motion"):
            original = load_json(ROOT / f"conformance/ir/{consumer}-request.json")
            validator = validator_for(f"schemas/{consumer}-request.schema.json")
            cases = load_json(ROOT / f"conformance/ir/{consumer}-cases.json")
            for member, label in (("part", "part"), ("checkedColorRole", "checked color role")):
                with self.subTest(consumer=consumer, member=member):
                    selected = [case for case in cases if case["name"] == f"toggle request source tagged {label}"]
                    self.assertEqual(len(selected), 1)
                    case = selected[0]
                    self.assertTrue(validator_for(f"schemas/{consumer}-case.schema.json").is_valid(case))
                    self.assertFalse(case["requestSchemaValid"])
                    invalid = apply_changes(original, case["requestChanges"])
                    self.assertEqual(invalid, {**original, member: {original[member]: None}})
                    self.assertFalse(validator.is_valid(invalid))


if __name__ == "__main__":
    unittest.main()
