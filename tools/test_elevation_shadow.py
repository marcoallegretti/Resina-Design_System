import copy
import json
import unittest

from check_elevation_shadow_backend import BASELINE, VECTORS, compare_vector, semantic_mismatch, source_cases
from check_schemas import ROOT, load_json
from elevation_shadow_arithmetic import expected_result, hull_violation, oklab, range_contrast, srgb


def vectors():
    return {case["name"]: case for case in load_json(ROOT / VECTORS)}


def mutated(case, mutation):
    """A complete result produced by a nonconformant law, keeping the reference hull.

    None means the nonconformant backend fails a request that must succeed.
    """
    actual = copy.deepcopy(case["expected"])
    changed = expected_result(case["request"], mutation)
    if changed is None:
        return None
    for name, value in changed.items():
        if name == "shadow" and value is not None:
            value = {**value, "hull": actual["shadow"]["hull"]}
        actual[name] = value
    return actual


def rejected(mutation):
    names = []
    for name, case in vectors().items():
        if "expected" in case:
            actual = mutated(case, mutation)
            if actual is None or compare_vector(actual, case["expected"], case["request"]):
                names.append(name)
    return names


class ElevationShadowProofTests(unittest.TestCase):
    def test_oklab_matches_public_conversion_vectors(self):
        for vector in load_json(ROOT / "conformance/color/oklab-conversion-vectors.json"):
            if "srgb" in vector and "oklab" in vector:
                for actual, expected in zip(oklab(vector["srgb"]), vector["oklab"]):
                    self.assertAlmostEqual(actual, expected, delta=1e-12)

    def test_authored_vectors_satisfy_independent_arithmetic(self):
        for name, case in vectors().items():
            if "expected" in case:
                with self.subTest(name=name):
                    self.assertIsNone(compare_vector(case["expected"], case["expected"], case["request"]))

    def test_light_mirrored_in_rtl_is_rejected(self):
        names = rejected("mirror-rtl")
        self.assertIn("rtl asymmetric shape keeps the physical light", names)
        self.assertIn("rtl symmetric shape equals ltr", names)
        case = vectors()["rtl asymmetric shape keeps the physical light"]
        ltr = vectors()["ltr asymmetric shape"]["expected"]["shadow"]["hull"]
        actual = copy.deepcopy(case["expected"])
        mirrored = copy.deepcopy(ltr)
        width = case["request"]["size"]["width"]
        for segment in mirrored["segments"]:
            for point in ("from", "to", "center"):
                if point in segment:
                    segment[point]["x"] = width - segment[point]["x"]
        actual["shadow"]["hull"] = mirrored
        self.assertIsNotNone(semantic_mismatch(actual, case["request"]))

    def test_centered_blur_inside_the_band_is_rejected(self):
        names = rejected("gaussian")
        self.assertIn("light raised rest with advancedShadow", names)
        self.assertIn("dark modal depthScale 2 with advancedShadow", names)
        self.assertNotIn("tier 0 without advancedShadow", names)

    def test_guard_that_ignores_the_band_is_rejected(self):
        names = rejected("skip-band-guard")
        self.assertIn("light raised rest on surface.low", names)
        self.assertNotIn("light raised depthScale 0 on surface.base", names)

    def test_guard_failures_depend_on_the_band(self):
        cases = vectors()
        for name in ("edge guard fails only beside the band", "focus guard fails only beside the band"):
            request = cases[name]["request"]
            self.assertIsNone(expected_result(request))
            self.assertIsNotNone(expected_result(request, "skip-band-guard"))
        control = cases["edge and focus minimum 5.5 hold without a shadow"]
        self.assertIsNone(control["expected"]["shadow"])
        self.assertEqual(control["request"]["edge"]["minimumContrast"], 5.5)
        self.assertEqual(control["request"]["focus"]["minimumContrast"], 5.5)

    def test_focus_without_the_outline_strong_fallback_is_rejected(self):
        names = rejected("no-focus-fallback")
        self.assertEqual(names, ["focus falls back to outline.strong beside the band",
                                 "edge and focus minimum 5.5 hold without a shadow"])
        _, successes = source_cases()
        fallbacks = [request for name, _, request in successes if name.endswith("focus falls back to outline.strong")]
        self.assertEqual(len(fallbacks), 2)
        for request in fallbacks:
            self.assertEqual(expected_result(request)["focus"]["colorRole"], "outline.strong")
            self.assertIsNone(expected_result(request, "no-focus-fallback"))

    def test_hull_outside_the_canonical_record_is_rejected(self):
        case = vectors()["ltr asymmetric shape"]
        actual = copy.deepcopy(case["expected"])
        segments = actual["shadow"]["hull"]["segments"]
        actual["shadow"]["hull"]["segments"] = segments[1:] + segments[:1]
        self.assertIsNone(hull_violation(actual["shadow"]["hull"], case["request"], actual["shadow"]["sweep"]))
        self.assertTrue(semantic_mismatch(actual, case["request"]).startswith("/shadow/hull/segments"))

    def test_geometry_members_use_relative_tolerance(self):
        case = vectors()["dark modal depthScale 2 with advancedShadow"]
        expected = case["expected"]
        geometry = copy.deepcopy(expected)
        geometry["finalDepth"] *= 1 + 9e-13
        geometry["shadow"]["paintBounds"]["width"] *= 1 + 9e-13
        self.assertIsNone(compare_vector(geometry, expected, case["request"]))
        ratio = copy.deepcopy(expected)
        ratio["edge"]["contrastRatio"] += 2e-12
        self.assertEqual(compare_vector(ratio, expected, case["request"]), "/edge/contrastRatio")

    def test_tier_zero_is_a_subset_of_the_enhanced_appearance(self):
        cases = vectors()
        tier0 = cases["tier 0 without advancedShadow"]["expected"]
        enhanced = cases["light raised rest with advancedShadow"]["expected"]
        self.assertEqual(tier0["edge"], enhanced["edge"])
        self.assertEqual(tier0["shadow"]["colorRange"], enhanced["shadow"]["colorRange"])
        for distance, low, high in zip(cases["tier 0 without advancedShadow"]["request"]["distanceSamples"],
                                       tier0["samples"], enhanced["samples"]):
            self.assertLessEqual(low["weight"], high["weight"])
            if distance <= 0:
                self.assertEqual(low, high)

    def test_zero_final_depth_casts_no_shadow_despite_contact_margin(self):
        cases = vectors()
        self.assertIsNone(cases["contact margin without final depth casts no shadow"]["expected"]["shadow"])
        band = cases["contact margin extends the band beyond the final depth"]["expected"]["shadow"]
        self.assertAlmostEqual(band["bandThickness"]["bottom"], 0.9403762257505053 * 3.5, places=12)

    def test_black_neighbor_reports_zero_difference_without_failure(self):
        result = vectors()["dark raised rest on black reports zero perceptual difference"]["expected"]
        self.assertEqual(result["shadow"]["deltaEOk"], 0)
        self.assertEqual(result["edge"]["colorRole"], "outline")

    def test_reference_profiles_cover_every_role(self):
        for theme, shade in (("light", 0.062), ("dark", 0.4)):
            profiles = load_json(ROOT / f"definitions/shadow-{theme}.json")
            self.assertEqual({profile["shadowShade"] for profile in profiles["roles"].values()}, {shade})
            self.assertEqual(set(profiles), {"schemaVersion", "roles"})

    def test_guard_range_bounds_every_penumbra_color(self):
        request = vectors()[BASELINE]["request"]
        adjacent = request["adjacentColor"]["components"]
        band = [c * (1 - 0.062) for c in adjacent]
        outline = request["edge"]["outline"]["components"]
        bound = range_contrast(outline, band, adjacent)
        for step in range(101):
            color = [c * (1 - 0.062 * step / 100) for c in adjacent]
            self.assertGreaterEqual(range_contrast(outline, color, color), bound - 1e-15)
        self.assertEqual(srgb(band)["alpha"], 1)

    def test_source_cases_cover_positional_and_overflow_inputs(self):
        failures, successes = source_cases()
        names = [name for name, _ in failures]
        for expected in ("positional request", "positional /profiles/roles/raised", "positional /edge",
                         "overflow /elevationDepth", "final depth beyond finite range", "modal shadowShade 1",
                         "edge guard fails only beside the band", "focus guard fails only beside the band"):
            self.assertIn(expected, names)
        self.assertTrue(all(json.loads(source) == request for _, source, request in successes))


if __name__ == "__main__":
    unittest.main()
