import copy
import unittest

from check_schemas import ROOT, load_json, validator_for


class CommandPaintSchemaTests(unittest.TestCase):
    def sample(self):
        body = load_json(ROOT / "conformance/ir/opaque-surface-expected.json")
        body["materialRole"] = "control.passive"
        return {"schemaVersion": "0.1.0", "phase": "rest", "response": {"bodyMix": 0, "depthScale": 1},
                "paint": {"schemaVersion": "0.1.0", "body": body}}

    def test_phase_must_follow_preserved_body_signals(self):
        validator = validator_for("schemas/command-paint-ir.schema.json")
        result = self.sample()
        self.assertFalse(list(validator.iter_errors(result)))
        for states, phase in ((["hover"], "hover"), (["hover", "pressed"], "pressed"),
                              (["hover", "pressed", "disabled"], "disabled")):
            result["paint"]["body"]["states"]["states"] = states
            result["phase"] = phase
            self.assertFalse(list(validator.iter_errors(result)))
            result["phase"] = "rest"
            self.assertTrue(list(validator.iter_errors(result)))

    def test_rest_requires_identity_and_state_domain_is_closed(self):
        validator = validator_for("schemas/command-paint-ir.schema.json")
        result = self.sample()
        for field in ("bodyMix", "depthScale"):
            changed = copy.deepcopy(result)
            changed["response"][field] = 0.5
            self.assertTrue(list(validator.iter_errors(changed)))
        for state in ("checked", "busy", "dragging", "error"):
            changed = copy.deepcopy(result)
            changed["paint"]["body"]["states"]["states"] = [state]
            self.assertTrue(list(validator.iter_errors(changed)))
        changed = copy.deepcopy(result)
        changed["paint"]["body"]["states"]["states"] = ["focused"]
        self.assertTrue(list(validator.iter_errors(changed)))

    def test_command_profiles_are_explicit_and_reject_unused_invalid_data(self):
        validator = validator_for("schemas/command-appearance.schema.json")
        profile = load_json(ROOT / "definitions/command-appearance-light.json")
        self.assertFalse(list(validator.iter_errors(profile)))
        for family in profile["profiles"]:
            changed = copy.deepcopy(profile)
            del changed["profiles"][family]["disabled"]
            self.assertTrue(list(validator.iter_errors(changed)))
            changed = copy.deepcopy(profile)
            changed["profiles"][family]["pressed"]["depthScale"] = 1.01
            self.assertTrue(list(validator.iter_errors(changed)))
