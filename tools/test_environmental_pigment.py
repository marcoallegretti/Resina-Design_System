import copy
import json
import unittest

from check_environmental_pigment_backend import build_request, check_independent_result, profile
from check_schemas import ROOT, load_json
from check_theme_derivation_backend import build_request as build_derivation_request
from environmental_pigment import (
    ADAPTABLE_ROLES, FROST_ROLE, PigmentError, adapt, half_turn, oklch_from_srgb, template_roles,
)
from theme_derivation import THEMES, derive, gamut_map, normalized_hue

WARM = {"color": {"colorSpace": "srgb", "components": [0.86, 0.72, 0.48]}}
BRIGHT = {
    "color": {"colorSpace": "srgb", "components": [0.98, 0.97, 0.93]},
    "range": {"lower": {"colorSpace": "srgb", "components": [0.95, 0.94, 0.90]},
              "upper": {"colorSpace": "srgb", "components": [1, 1, 1]}},
}


def request_for(observation=None, **changes):
    case = dict(changes)
    if observation is not None:
        case["observation"] = observation
    return build_request(case)[1]


def observation_at(lightness, chroma, hue):
    return {"color": {"colorSpace": "srgb", "components": gamut_map([lightness, chroma, hue])}}


class SelectionTests(unittest.TestCase):
    def targets(self, observation, theme="light"):
        request = request_for(observation)
        return request, adapt(request)["themes"][theme]["roles"]

    def template(self, request, theme):
        source = json.loads(request["templates"][theme])
        colors, _, _ = template_roles(source, request["externalSources"])
        return {role: oklch_from_srgb(colors[role]) for role in ADAPTABLE_ROLES}

    def test_hue_inside_the_range_tracks_the_observation(self):
        observation = observation_at(0.7, 0.1, 120.0)
        request, roles = self.targets(observation)
        observed = oklch_from_srgb(observation["color"]["components"])
        template = self.template(request, "light")
        for role in ("surface.base", "surface.high"):
            offset = half_turn(observed[2] - template[role][2])
            ranges = profile()["themes"]["light"][role]["hue"]
            self.assertTrue(ranges["min"] <= offset <= ranges["max"], role)
            self.assertAlmostEqual(roles[role]["targetOklch"][2], normalized_hue(observed[2]), places=9)

    def test_hue_outside_the_range_stops_at_its_bound(self):
        observation = observation_at(0.7, 0.1, 280.0)
        request, roles = self.targets(observation)
        template = self.template(request, "light")
        for role in ADAPTABLE_ROLES[:3]:
            bound = profile()["themes"]["light"][role]["hue"]
            offset = half_turn(roles[role]["targetOklch"][2] - template[role][2])
            self.assertIn(round(offset, 9), (bound["min"], bound["max"]), role)

    def test_chroma_is_clamped_to_the_authored_range(self):
        _, saturated = self.targets(observation_at(0.7, 0.3, 30.0))
        for role in ADAPTABLE_ROLES[:3]:
            self.assertEqual(saturated[role]["targetOklch"][1],
                             profile()["themes"]["light"][role]["chroma"]["max"])
        _, neutral = self.targets({"color": {"colorSpace": "srgb", "components": [0.5, 0.5, 0.5]}})
        for role in ADAPTABLE_ROLES[:3]:
            self.assertLess(neutral[role]["targetOklch"][1], 1e-3)

    def test_lightness_is_never_selected(self):
        request, roles = self.targets(observation_at(0.2, 0.1, 60.0))
        template = self.template(request, "light")
        for role, adapted in roles.items():
            self.assertEqual(adapted["targetOklch"][0], template[role][0])

    def test_achromatic_observation_keeps_template_hue(self):
        request, roles = self.targets({"color": {"colorSpace": "srgb", "components": [0.4, 0.4, 0.4]}})
        template = self.template(request, "light")
        for role in ADAPTABLE_ROLES[:3]:
            self.assertAlmostEqual(roles[role]["targetOklch"][2], template[role][2], places=9)

    def test_every_hue_and_chroma_keeps_every_guard_pair(self):
        for hue in range(0, 360, 5):
            for chroma in (0.0, 0.015, 0.05, 0.4):
                request = request_for(observation_at(0.65, chroma, float(hue)))
                result = adapt(request)
                check_independent_result(result, request)


class AdaptationTests(unittest.TestCase):
    def test_no_observation_gives_the_authored_theme(self):
        request = request_for()
        result = adapt(request)
        self.assertEqual(result["adaptation"], {"applied": False, "reason": "noObservation"})
        for name in THEMES:
            self.assertEqual(result["themes"][name]["source"], json.loads(request["templates"][name]))
            self.assertEqual(result["themes"][name]["roles"], {})

    def test_accessibility_preferences_suppress_adaptation(self):
        for preference in ("highContrast", "reducedTransparency"):
            preferences = {"reducedMotion": False, "reducedTransparency": False, "highContrast": False}
            preferences[preference] = True
            request = request_for(BRIGHT, accessibilityPreferences=preferences)
            result = adapt(request)
            self.assertEqual(result["adaptation"], {"applied": False, "reason": preference})
            check_independent_result(result, request)

    def test_a_present_observation_is_deterministic(self):
        request = request_for(BRIGHT)
        self.assertEqual(adapt(request), adapt(copy.deepcopy(request)))

    def test_frost_adapts_only_when_certified(self):
        request = request_for(BRIGHT)
        result = adapt(request)
        for name in THEMES:
            theme = result["themes"][name]
            self.assertEqual(FROST_ROLE in theme["roles"],
                             all(entry["passed"] for entry in theme["frostCertification"]))
        self.assertIn(FROST_ROLE, result["themes"]["light"]["roles"])
        self.assertNotIn(FROST_ROLE, result["themes"]["dark"]["roles"])
        check_independent_result(result, request)

    def test_frost_is_not_adapted_without_a_range(self):
        result = adapt(request_for(WARM))
        for name in THEMES:
            self.assertNotIn("frostCertification", result["themes"][name])
            self.assertNotIn(FROST_ROLE, result["themes"][name]["roles"])

    def test_derived_themes_are_valid_templates(self):
        _, derivation = build_derivation_request({
            "name": "teal", "outcome": "valid", "requestSchemaValid": True,
            "seed": {"colorSpace": "srgb", "components": [0.1, 0.6, 0.5]}})
        derived = derive(derivation)
        request = request_for(BRIGHT)
        for name in THEMES:
            request["templates"][name] = json.dumps(derived["themes"][name]["source"])
        result = adapt(request)
        for name in THEMES:
            assignments = result["themes"][name]["source"]["colorAssignments"]["roles"]
            for role in ("accent.primary", "accent.secondary", "accent.tertiary", "selection", "focus"):
                self.assertEqual(assignments[role], f"derivedColor.{role}")


class RejectionTests(unittest.TestCase):
    def test_nonfinite_and_oversized_observations_are_rejected(self):
        for component in (float("nan"), float("inf"), 1.0000001, -0.0000001, 1e300):
            observation = {"color": {"colorSpace": "srgb", "components": [component, 0.5, 0.5]}}
            request = request_for(WARM)
            request["observation"] = observation
            with self.assertRaisesRegex(PigmentError, "invalid observation"):
                adapt(request)

    def test_inconsistent_ranges_are_rejected(self):
        inverted = copy.deepcopy(BRIGHT)
        inverted["range"]["lower"]["components"][0] = 1.0
        outside = copy.deepcopy(BRIGHT)
        outside["color"]["components"] = [0.5, 0.5, 0.5]
        for observation in (inverted, outside):
            with self.assertRaisesRegex(PigmentError, "invalid observation"):
                adapt(request_for(observation))

    def test_invalid_observation_is_rejected_even_when_adaptation_is_suppressed(self):
        outside = copy.deepcopy(BRIGHT)
        outside["color"]["components"] = [0.5, 0.5, 0.5]
        preferences = {"reducedMotion": False, "reducedTransparency": False, "highContrast": True}
        with self.assertRaisesRegex(PigmentError, "invalid observation"):
            adapt(request_for(outside, accessibilityPreferences=preferences))

    def test_failing_vectors_name_their_themes(self):
        for case in load_json(ROOT / "conformance/themes/environmental-pigment-vectors.json"):
            if "failingThemes" not in case:
                continue
            with self.subTest(name=case["name"]):
                with self.assertRaises(PigmentError) as raised:
                    adapt(build_request(case)[1])
                for name in THEMES:
                    self.assertEqual(f"{name} theme:" in str(raised.exception),
                                     name in case["failingThemes"])


class IndependentCheckTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.request = request_for(BRIGHT)
        cls.result = adapt(cls.request)

    def mutated(self):
        return copy.deepcopy(self.result)

    def test_complete_result_passes(self):
        check_independent_result(self.result, self.request)

    def test_rebinding_an_accent_is_rejected(self):
        mutated = self.mutated()
        mutated["themes"]["light"]["source"]["colorAssignments"]["roles"]["accent.primary"] = (
            "adaptedColor.surface.base")
        with self.assertRaisesRegex(AssertionError, "differs from its template"):
            check_independent_result(mutated, self.request)

    def test_changed_lightness_is_rejected(self):
        mutated = self.mutated()
        mutated["themes"]["light"]["roles"]["surface.low"]["targetOklch"][0] += 0.01
        with self.assertRaisesRegex(AssertionError, "changed lightness"):
            check_independent_result(mutated, self.request)

    def test_chroma_outside_the_authored_range_is_rejected(self):
        mutated = self.mutated()
        mutated["themes"]["dark"]["roles"]["surface.base"]["targetOklch"][1] = 0.2
        with self.assertRaisesRegex(AssertionError, "chroma left"):
            check_independent_result(mutated, self.request)

    def test_uncertified_frost_adaptation_is_rejected(self):
        mutated = self.mutated()
        mutated["themes"]["dark"]["roles"][FROST_ROLE] = mutated["themes"]["light"]["roles"][FROST_ROLE]
        with self.assertRaisesRegex(AssertionError, "exactly when it is certified"):
            check_independent_result(mutated, self.request)

    def test_dropped_guard_pair_is_rejected(self):
        mutated = self.mutated()
        mutated["themes"]["light"]["legibilityGuard"].pop(9)
        with self.assertRaisesRegex(AssertionError, "every legibility guard pair"):
            check_independent_result(mutated, self.request)

    def test_change_without_adaptation_is_rejected(self):
        request = request_for()
        mutated = adapt(request)
        mutated["themes"]["dark"]["roles"] = self.result["themes"]["dark"]["roles"]
        with self.assertRaisesRegex(AssertionError, "no adaptation was applied"):
            check_independent_result(mutated, request)


if __name__ == "__main__":
    unittest.main()
