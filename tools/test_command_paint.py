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
            changed["profiles"][family]["pressed"]["depthScale"] = 2.01
            self.assertTrue(list(validator.iter_errors(changed)))


class PerceivableResponseTests(unittest.TestCase):
    def test_checker_oklab_matches_public_conversion_vectors(self):
        from check_command_paint_backend import oklab
        vectors = [v for v in load_json(ROOT / "conformance/color/oklab-conversion-vectors.json") if "srgb" in v and "oklab" in v]
        self.assertGreaterEqual(len(vectors), 7)
        for vector in vectors:
            for actual, expected in zip(oklab(vector["srgb"]), vector["oklab"]):
                self.assertAlmostEqual(actual, expected, delta=1e-6, msg=vector["name"])

    def test_every_phase_of_the_profile_is_measured(self):
        from check_command_paint_backend import imperceptible_phase
        profile = copy.deepcopy(load_json(ROOT / "conformance/ir/command-paint-request.json")["commandAppearance"]["profiles"]["elastomer"])
        self.assertIsNone(imperceptible_phase((0.8, 0.7, 0.6), 2, profile))
        for phase, response in (("hover", {"bodyMix": 0.06, "depthScale": 1}),
                                ("pressed", {"bodyMix": 0, "depthScale": 0.51}),
                                ("disabled", {"bodyMix": -0.02, "depthScale": 0.7})):
            changed = copy.deepcopy(profile)
            changed[phase] = response
            self.assertEqual(imperceptible_phase((0.8, 0.7, 0.6), 2, changed), phase)
        changed = copy.deepcopy(profile)
        changed["pressed"] = {"bodyMix": 0, "depthScale": 0.5}
        self.assertIsNone(imperceptible_phase((0.8, 0.7, 0.6), 2, changed))

    def test_disabled_is_measured_against_pressed(self):
        from check_command_paint_backend import imperceptible_phase
        profile = copy.deepcopy(load_json(ROOT / "conformance/ir/command-paint-request.json")["commandAppearance"]["profiles"]["elastomer"])
        profile["disabled"] = {"bodyMix": profile["pressed"]["bodyMix"], "depthScale": 0.3}
        self.assertEqual(imperceptible_phase((0.8, 0.7, 0.6), 2, profile), "disabled from pressed")
        profile["disabled"]["depthScale"] = 0
        self.assertIsNone(imperceptible_phase((0.8, 0.7, 0.6), 2, profile))
