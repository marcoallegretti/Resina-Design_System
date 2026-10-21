import copy
import json
import subprocess
import unittest
from unittest.mock import patch

from check_schemas import ROOT, load_json, validator_for


class CommandPaintSchemaTests(unittest.TestCase):
    def sample(self):
        body = load_json(ROOT / "conformance/ir/lit-control-body-ir.json")
        body["materialRole"] = "control.passive"
        body["states"]["states"] = ["rest"]
        return {"schemaVersion": "0.2.0", "phase": "rest", "response": {"bodyMix": 0, "depthScale": 1},
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


class LitCommandCheckerTests(unittest.TestCase):
    def published(self):
        request = load_json(ROOT / "conformance/ir/command-paint-request.json")
        return request, load_json(ROOT / "conformance/ir/command-paint-ir.json")

    def test_lit_face_rejects_changed_light_pigment_and_content_bounds(self):
        from check_command_paint_backend import lit_face_mismatch
        request, result = self.published()
        body = result["paint"]["body"]
        self.assertIsNone(lit_face_mismatch(body, request, result["response"], "rest"))
        for name, path, change in (
                ("ambient", "/paint/body/face/light", lambda face: face["light"].update(ambient=0.25)),
                ("interaction", "/paint/body/face/light", lambda face: face["light"].update(
                    interaction={"direction": {"x": 1, "y": 0}, "strength": 0.5})),
                ("side pigment", "/paint/body/face/pigment",
                 lambda face: face["pigment"]["side"]["components"].__setitem__(0, 0.5)),
                ("content width", "/paint/body/face/contentBounds",
                 lambda face: face["contentBounds"].update(width=face["contentBounds"]["width"] + 2 ** -30))):
            changed = copy.deepcopy(body)
            change(changed["face"])
            with self.subTest(name):
                self.assertEqual(lit_face_mismatch(changed, request, result["response"], "rest"), path)

    def test_lit_face_derives_admitted_light_from_the_request_phase(self):
        from check_command_paint_backend import admitting, lit_face_mismatch
        request, result = self.published()
        self.assertEqual(lit_face_mismatch(result["paint"]["body"], admitting(request), result["response"], "rest"),
                         "/paint/body/face/light")

    def test_light_differential_rejects_light_outside_the_face(self):
        import check_command_paint_backend as checker
        request, result = self.published()
        requests = []

        def resolve(command, sent, timeout, name):
            requests.append(sent)
            output = copy.deepcopy(result)
            if sent["light"]["ambient"] is not None:
                output["paint"]["body"]["face"]["light"]["ambient"] = sent["light"]["ambient"]
                output["paint"]["body"]["depth"] += shift
            return output

        for shift in (0, 2 ** -40):
            requests.clear()
            with self.subTest(shift=shift), patch.object(checker, "check_success", side_effect=resolve):
                if shift:
                    with self.assertRaisesRegex(AssertionError, "more than face.light"):
                        checker.check_light_differential(["backend"], request, 1, "case")
                else:
                    checker.check_light_differential(["backend"], request, 1, "case")
                lit, unlit = requests
                self.assertEqual(lit["light"], request["light"])
                self.assertEqual(unlit["light"], {"ambient": None, "interaction": None})
                for sent in requests:
                    environment = sent["surface"]["body"]["theme"]["environment"]
                    self.assertTrue(environment["rendererCapabilities"]["dynamicLighting"])
                    self.assertFalse(environment["accessibilityPreferences"]["highContrast"])
                    self.assertFalse(environment["accessibilityPreferences"]["reducedMotion"])
                    self.assertEqual(environment["qualityPolicy"], "balanced")

    def test_suppressed_negative_zero_sources_must_be_rejected(self):
        import check_command_paint_backend as checker
        request, _ = self.published()
        accepted = []

        def backend(accept_negative):
            def run(command, source, timeout):
                if "-1e-400" in source and not accept_negative:
                    return subprocess.CompletedProcess(command, 1, "", "negative zero source\n")
                return subprocess.CompletedProcess(command, 0, "{}", "")
            return run

        def succeed(command, sent, timeout, name, source=None):
            accepted.append((name, sent, source))

        with patch("check_color_guard_backend.run_backend", side_effect=backend(False)), \
                patch.object(checker, "check_success", side_effect=succeed):
            self.assertEqual(checker.coefficient_sources(["backend"], request, 1), 24)
        self.assertEqual(len(accepted), 18)
        suppressed = set()
        for name, sent, source in accepted:
            *label, pointer, literal = name.split(" ")
            self.assertNotIn("-1e-400", source)
            self.assertEqual(json.loads(source), sent)
            value = sent
            for member in pointer.strip("/").split("/"):
                value = value[member]
            self.assertEqual(value, 0)
            states = sent["surface"]["body"]["surface"]["states"]["states"]
            contrast = sent["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]["highContrast"]
            suppressed.add((" ".join(label), tuple(states), contrast))
        self.assertEqual(suppressed, {("rest", ("rest",), False), ("disabled", ("disabled",), False),
                                      ("high contrast", ("hover",), True)})
        with patch("check_color_guard_backend.run_backend", side_effect=backend(True)), \
                patch.object(checker, "check_success", side_effect=succeed), \
                self.assertRaisesRegex(AssertionError, "rest /light/ambient negative zero source"):
            checker.coefficient_sources(["backend"], request, 1)
