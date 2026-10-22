import copy
import json
import subprocess
import unittest
from unittest.mock import patch

from check_schemas import ROOT, load_json, validator_for
import check_toggle_part_paint_backend as checker
from check_command_paint_backend import admitting, lit_face_mismatch
from check_toggle_part_paint_backend import toggle_part_mismatch


def published_requests():
    """The public requests whose reference results are the published Toggle IR, as in the Rust tests."""
    base = load_json(ROOT / "conformance/ir/toggle-part-paint-request.json")
    track = admitting(copy.deepcopy(base))
    track["surface"]["body"]["surface"]["states"]["states"] = ["hover", "focused", "checked"]
    thumb = admitting(copy.deepcopy(base))
    thumb["part"] = "thumb"
    thumb["surface"]["body"]["size"] = {"width": 16, "height": 16}
    thumb["surface"]["body"]["surface"]["states"]["states"] = ["pressed", "checked"]
    return {"track": track, "thumb": thumb}


class TogglePartPaintProtocolTests(unittest.TestCase):
    def setUp(self):
        self.requests = published_requests()
        self.published = load_json(ROOT / "conformance/ir/toggle-part-paint-ir.json")
        self.request, self.output = self.requests["thumb"], self.published["thumb"]
        self.validator = validator_for("schemas/toggle-part-paint-ir.schema.json")

    def test_published_results_satisfy_the_independent_arithmetic(self):
        for part, request in self.requests.items():
            with self.subTest(part=part):
                self.assertTrue(self.validator.is_valid(self.published[part]))
                self.assertIsNone(toggle_part_mismatch(self.published[part], request))

    def test_checker_rejects_schema_valid_wrong_selection_role_and_pigment(self):
        self.assertTrue(self.validator.is_valid(self.output))
        for field, value in (("colorRole", "surface.base"), ("materialRole", "control.passive")):
            actual = copy.deepcopy(self.output)
            actual["paint"]["body"][field] = value
            self.assertTrue(self.validator.is_valid(actual))
            self.assertIsNotNone(toggle_part_mismatch(actual, self.request))
        actual = copy.deepcopy(self.output)
        actual["paint"]["body"]["face"]["contentContrastRatio"] = 21
        self.assertTrue(self.validator.is_valid(actual))
        self.assertEqual(toggle_part_mismatch(actual, self.request), "/paint/body/face/contentContrastRatio")
        actual = copy.deepcopy(self.output)
        actual["paint"]["body"]["face"]["pigment"]["body"]["components"] = [1, 1, 1]
        self.assertTrue(self.validator.is_valid(actual))
        self.assertEqual(toggle_part_mismatch(actual, self.request), "/paint/body/face/pigment/body")

    def test_lit_face_rejects_changed_light_pigment_profile_and_content_bounds(self):
        for part, request in self.requests.items():
            body = self.published[part]["paint"]["body"]
            for name, path, change in (
                    ("ambient", "/paint/body/face/light", lambda face: face["light"].update(ambient=0.5)),
                    ("interaction", "/paint/body/face/light", lambda face: face["light"].update(interaction=None)),
                    ("side pigment", "/paint/body/face/pigment",
                     lambda face: face["pigment"]["side"]["components"].__setitem__(0, 0.5)),
                    ("another part's profile", "/paint/body/face/surface/profile",
                     lambda face: face["surface"]["profile"].update(
                         shoulderWidth=face["surface"]["profile"]["shoulderWidth"] + 0.25)),
                    ("content width", "/paint/body/face/contentBounds",
                     lambda face: face["contentBounds"].update(width=face["contentBounds"]["width"] + 2 ** -30))):
                changed = copy.deepcopy(self.published[part])
                change(changed["paint"]["body"]["face"])
                with self.subTest(part=part, change=name):
                    self.assertEqual(toggle_part_mismatch(changed, request), path)
            self.assertIsNotNone(body["face"]["light"]["interaction"])

    def test_the_thumb_profile_is_not_the_track_profile(self):
        request = copy.deepcopy(self.request)
        request["relief"]["toggleThumb"], request["relief"]["toggleTrack"] = (
            request["relief"]["toggleTrack"], request["relief"]["toggleThumb"])
        self.assertEqual(toggle_part_mismatch(self.output, request), "/paint/body/face/surface/profile")

    def test_light_is_derived_from_the_request_environment_and_phase(self):
        body, response = self.output["paint"]["body"], self.output["response"]
        self.assertIsNone(lit_face_mismatch(body, self.request, response, "pressed", "toggleThumb", "selection"))
        for name, phase, change in (
                ("high contrast", "pressed", lambda environment: environment["accessibilityPreferences"].update(
                    highContrast=True)),
                ("rest phase", "rest", lambda environment: None),
                ("economy", "pressed", lambda environment: environment.update(qualityPolicy="economy"))):
            request = copy.deepcopy(self.request)
            change(request["surface"]["body"]["theme"]["environment"])
            with self.subTest(name):
                self.assertEqual(lit_face_mismatch(body, request, response, phase, "toggleThumb", "selection"),
                                 "/paint/body/face/light")

    def test_checked_and_thumb_focus_are_structural_invariants(self):
        actual = copy.deepcopy(self.output)
        actual["checked"] = False
        self.assertFalse(self.validator.is_valid(actual))
        actual["checked"] = True
        actual["paint"]["body"]["states"]["states"].remove("checked")
        self.assertFalse(self.validator.is_valid(actual))
        actual = copy.deepcopy(self.output)
        actual["paint"]["focus"] = load_json(ROOT / "conformance/ir/focus-ir-expected.json")
        self.assertFalse(self.validator.is_valid(actual))

    def test_light_differential_rejects_light_outside_the_face(self):
        requests = []

        def resolve(command, sent, timeout, name):
            requests.append(sent)
            output = copy.deepcopy(self.output)
            if sent["light"]["ambient"] is not None:
                output["paint"]["body"]["face"]["light"]["ambient"] = sent["light"]["ambient"]
                output["paint"]["body"]["depth"] += shift
            return output

        for shift in (0, 2 ** -40):
            requests.clear()
            with self.subTest(shift=shift), patch.object(checker, "check_success", side_effect=resolve):
                if shift:
                    with self.assertRaisesRegex(AssertionError, "more than face.light"):
                        checker.check_light_differential(["backend"], self.request, 1, "case")
                else:
                    checker.check_light_differential(["backend"], self.request, 1, "case")
                lit, unlit = requests
                self.assertEqual(lit["light"], self.request["light"])
                self.assertEqual(unlit["light"], {"ambient": None, "interaction": None})
                for sent in requests:
                    environment = sent["surface"]["body"]["theme"]["environment"]
                    self.assertTrue(environment["rendererCapabilities"]["dynamicLighting"])
                    self.assertFalse(environment["accessibilityPreferences"]["highContrast"])
                    self.assertFalse(environment["accessibilityPreferences"]["reducedMotion"])
                    self.assertEqual(environment["qualityPolicy"], "balanced")

    def test_suppressed_negative_zero_sources_must_be_rejected(self):
        base = load_json(ROOT / "conformance/ir/toggle-part-paint-request.json")
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
            self.assertEqual(checker.coefficient_sources(["backend"], base, 1), 24)
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
            checker.coefficient_sources(["backend"], base, 1)


if __name__ == "__main__":
    unittest.main()
