import copy
import json
import unittest

from check_schemas import ROOT, load_json
from check_theme_derivation_backend import (
    build_request, check_independent_result, derivation_mismatch, foundation_text,
)
from theme_derivation import (
    DERIVED_ROLES, THEMES, DerivationError, clip, delta_eok, derive, gamut_map, in_gamut,
    legibility_guard, oklab_to_srgb, oklch_from_srgb, oklch_to_oklab, template_colors,
)

BLUE = {"name": "blue", "outcome": "valid", "requestSchemaValid": True,
        "seed": {"colorSpace": "srgb", "components": [0, 0, 1]}}


class GamutMappingTests(unittest.TestCase):
    def test_lightness_bounds_return_white_and_black(self):
        self.assertEqual(gamut_map([1, 0.3, 120]), [1.0, 1.0, 1.0])
        self.assertEqual(gamut_map([0, 0.3, 120]), [0.0, 0.0, 0.0])

    def test_in_gamut_target_converts_without_mapping(self):
        target = oklch_from_srgb([0.2, 0.4, 0.6])
        for actual, expected in zip(gamut_map(target), [0.2, 0.4, 0.6]):
            self.assertAlmostEqual(actual, expected, places=12)

    def test_distant_target_reduces_chroma_instead_of_clipping(self):
        target = [0.8888, 0.0791, 265.0]
        self.assertFalse(in_gamut(oklab_to_srgb(oklch_to_oklab(target))))
        self.assertGreaterEqual(delta_eok(clip(target), target), 0.02)
        mapped = gamut_map(target)
        self.assertTrue(in_gamut(mapped))
        self.assertGreater(max(abs(a - b) for a, b in zip(mapped, clip(target))), 1e-3)
        self.assertLess(oklch_from_srgb(mapped)[1], target[1])

    def test_search_exits_match_reference_values(self):
        for origin, expected in [
            ([0.878, 0.203, 101.9], [0.9521966026830196, 0.8519357687705986, 0.0]),
            ([0.5, 0.3, 30.0], [0.7659544146267703, 0.0, 0.0]),
            ([0.8888, 0.0791, 265.0], [0.7619422285267825, 0.8563475125754105, 1.0]),
        ]:
            for actual, value in zip(gamut_map(origin), expected):
                self.assertAlmostEqual(actual, value, delta=1e-9)

    def test_near_target_returns_the_clipped_color(self):
        target = [0.8888, 0.0791, 0.0]
        self.assertLess(delta_eok(clip(target), target), 0.02)
        self.assertEqual(gamut_map(target), clip(target))


class DerivationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source, cls.request = build_request(BLUE)
        cls.result = derive(cls.request)

    def test_authored_guard_vectors_are_reproduced(self):
        vectors = load_json(ROOT / "conformance/themes/legibility-guard-vectors.json")
        for vector, name in zip(vectors, THEMES):
            theme = json.loads((ROOT / vector["theme"]).read_text(encoding="utf-8"))
            _, opaque = template_colors(theme, {"foundation.json": foundation_text()})
            guards, failures = legibility_guard(opaque)
            self.assertEqual(failures, [])
            self.assertIsNone(derivation_mismatch(guards, vector["legibilityGuard"]), name)

    def test_complete_result_passes_the_independent_checks(self):
        check_independent_result(self.result, self.request)

    def test_dropped_guard_pair_is_rejected(self):
        mutated = copy.deepcopy(self.result)
        mutated["themes"]["light"]["legibilityGuard"].pop(9)
        with self.assertRaisesRegex(AssertionError, "every legibility guard pair"):
            check_independent_result(mutated, self.request)

    def test_component_above_one_is_rejected(self):
        mutated = copy.deepcopy(self.result)
        mutated["themes"]["dark"]["roles"]["focus"]["color"]["components"][0] = 1.0000001
        with self.assertRaisesRegex(AssertionError, "outside"):
            check_independent_result(mutated, self.request)

    def test_missing_focus_is_rejected(self):
        mutated = copy.deepcopy(self.result)
        del mutated["themes"]["light"]["roles"]["focus"]
        with self.assertRaisesRegex(AssertionError, "derive exactly"):
            check_independent_result(mutated, self.request)

    def test_clipped_focus_differs_from_the_mapped_vector(self):
        dark = self.result["themes"]["dark"]["roles"]["focus"]
        clipped = copy.deepcopy(self.result)
        clipped["themes"]["dark"]["roles"]["focus"]["color"]["components"] = clip(dark["targetOklch"])
        self.assertEqual(derivation_mismatch(clipped, self.result),
                         "/themes/dark/roles/focus/color/components/0")

    def test_derived_source_only_rebinds_derived_roles(self):
        for name in THEMES:
            template = json.loads(self.request["templates"][name])
            source = self.result["themes"][name]["source"]
            changed = {role for role, path in source["colorAssignments"]["roles"].items()
                       if path != template["colorAssignments"]["roles"][role]}
            self.assertEqual(changed, set(DERIVED_ROLES))
            self.assertEqual(source["tokenResolver"]["resolutionOrder"][:-1],
                             template["tokenResolver"]["resolutionOrder"])

    def test_failing_vectors_name_their_themes(self):
        for case in load_json(ROOT / "conformance/themes/derivation-vectors.json"):
            if "failingThemes" not in case:
                continue
            with self.subTest(name=case["name"]):
                _, request = build_request(case)
                with self.assertRaises(DerivationError) as raised:
                    derive(request)
                for name in THEMES:
                    self.assertEqual(f"{name} theme:" in str(raised.exception),
                                     name in case["failingThemes"])


if __name__ == "__main__":
    unittest.main()
