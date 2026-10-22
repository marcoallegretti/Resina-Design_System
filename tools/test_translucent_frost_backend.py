import copy
import unittest

from check_schemas import ROOT, load_json, validator_for
from check_translucent_frost_backend import (
    baseline,
    case_request,
    cases,
    expected_result,
    translucent_mismatch,
)
from translucent_frost_arithmetic import (
    OPAQUE_SURFACE_EXPECTED,
    bound,
    luminance,
    over,
    ratio,
    select_representation,
)


def named(name):
    case = next(case for case in cases() if case["name"] == name)
    return expected_result(case_request(baseline(), case))


class TranslucentFrostOracleTests(unittest.TestCase):
    def test_cases_state_what_the_oracle_derives(self):
        for case in cases():
            if "errorContains" in case:
                continue
            with self.subTest(name=case["name"]):
                result = expected_result(case_request(baseline(), case))
                self.assertEqual(result["appearance"], case["expectedAppearance"])
                self.assertEqual(result["frostRepresentation"], case["expectedRepresentation"])
                self.assertEqual(result.get("fallbackReason"), case.get("expectedFallbackReason"))
                self.assertEqual(
                    "focus" in result or "focus" in result.get("paint", {}),
                    case["expectedFocus"],
                )

    def test_semantic_rejections_name_their_reason(self):
        rejected = [case for case in cases()
                    if "errorContains" in case and case["requestSchemaValid"]]
        self.assertGreaterEqual(len(rejected), 8)
        for case in rejected:
            with self.subTest(name=case["name"]):
                with self.assertRaisesRegex(ValueError, case["errorContains"]):
                    expected_result(case_request(baseline(), case))

    def test_selection_reproduces_the_public_fallback_vectors(self):
        vectors = load_json(ROOT / "conformance/materials/frost-fallback-vectors.json")
        for vector in vectors:
            with self.subTest(name=vector["name"]):
                environment = {
                    "accessibilityPreferences": vector["preferences"],
                    "rendererCapabilities": vector["capabilities"],
                    "qualityPolicy": vector["quality"],
                }
                self.assertEqual(select_representation(environment), vector["expected"])

    def test_every_fallback_branch_has_a_public_case(self):
        outcomes = {
            (result["frostRepresentation"], result.get("fallbackReason"))
            for result in (
                expected_result(case_request(baseline(), case))
                for case in cases() if "errorContains" not in case
            )
        }
        self.assertEqual(outcomes, {
            ("shapedBackdrop", None),
            ("regularBackdrop", None),
            ("translucentPigmented", None),
            ("opaqueDimensional", "selection"),
            ("opaqueDimensional", "legibility"),
        })

    def test_certified_bound_never_exceeds_any_grid_backdrop_inside_the_box(self):
        for name in (
            "a bright busy interval keeps translucency",
            "a light foreground is certified by the upper backdrop end",
            "a foreground inside the composited interval certifies at ratio one",
        ):
            with self.subTest(name=name):
                body = named(name)["body"]
                tint = body["pigment"]["body"]
                foreground = body["foreground"]["components"]
                lower = body["backdrop"]["range"]["lower"]["components"]
                upper = body["backdrop"]["range"]["upper"]["components"]
                steps = 6
                axes = [
                    [low + (high - low) * step / steps for step in range(steps + 1)]
                    for low, high in zip(lower, upper)
                ]
                least = min(
                    ratio(foreground, over(tint["components"], tint["alpha"], [r, g, b]))
                    for r in axes[0] for g in axes[1] for b in axes[2]
                )
                self.assertGreaterEqual(least + 1e-12, body["contentContrastRatio"])

    def test_either_backdrop_end_can_bind_the_certificate(self):
        dark = named("a dark interval forces the opaque body through legibility")
        self.assertEqual(dark["fallbackReason"], "legibility")
        light = named("a light foreground is certified by the upper backdrop end")["body"]
        foreground = light["foreground"]["components"]
        composited = light["backdrop"]["compositedBody"]
        at_lower = ratio(foreground, composited["lower"]["components"])
        at_upper = ratio(foreground, composited["upper"]["components"])
        self.assertLess(at_upper, at_lower)
        self.assertAlmostEqual(light["contentContrastRatio"], at_upper, places=12)
        crossing = named("a foreground inside the composited interval certifies at ratio one")
        self.assertEqual(crossing["body"]["contentContrastRatio"], 1.0)
        case = next(case for case in cases()
                    if case["name"] == "a threshold between the two ends rejects a light foreground")
        threshold = next(change["value"] for change in case["requestChanges"]
                         if change["path"] == "/minimumContentContrast")
        self.assertLess(at_upper, threshold)
        self.assertLess(threshold, at_lower)

    def test_a_ring_retains_the_representation_the_body_resolved_to(self):
        for name in (
            "focused translucent surface keeps its navigation ring",
            "focused opaque selection keeps its navigation ring",
            "focused legibility fallback keeps its navigation ring",
        ):
            result = named(name)
            focus = result["paint"]["focus"] if "paint" in result else result["focus"]
            binding = focus["indicator"]["binding"]
            self.assertEqual(binding["frostRepresentation"], result["frostRepresentation"], name)
            if "paint" in result:
                self.assertEqual(binding["frostPortableBody"], binding["opaqueColorFallback"])

    def test_schema_rejects_a_ring_that_contradicts_its_body(self):
        validator = validator_for("schemas/translucent-frost-ir.schema.json")
        opaque = named("focused legibility fallback keeps its navigation ring")
        self.assertEqual(list(validator.iter_errors(opaque)), [])
        stale = copy.deepcopy(opaque)
        stale["paint"]["focus"]["indicator"]["binding"]["frostRepresentation"] = "shapedBackdrop"
        self.assertNotEqual(list(validator.iter_errors(stale)), [])
        flagged = copy.deepcopy(opaque)
        flagged["paint"]["body"]["contentFallbackApplied"] = False
        self.assertNotEqual(list(validator.iter_errors(flagged)), [])
        translucent = named("focused translucent surface keeps its navigation ring")
        self.assertEqual(list(validator.iter_errors(translucent)), [])
        wrong = copy.deepcopy(translucent)
        wrong["focus"]["indicator"]["binding"]["frostRepresentation"] = "regularBackdrop"
        self.assertNotEqual(list(validator.iter_errors(wrong)), [])

    def test_interior_luminance_crossing_is_certified_conservatively(self):
        foreground = [0.5, 0.5, 0.5]
        self.assertEqual(bound(foreground, [0.2] * 3, [0.9] * 3), 1.0)
        self.assertLess(bound(foreground, [0.7] * 3, [0.9] * 3), ratio(foreground, [1.0] * 3))
        self.assertAlmostEqual(
            bound([0.0] * 3, [0.4] * 3, [0.9] * 3), ratio([0.0] * 3, [0.4] * 3)
        )
        self.assertGreater(luminance([0.9] * 3), luminance([0.4] * 3))

    def test_geometry_and_light_are_the_committed_opaque_surface(self):
        shape = load_json(ROOT / OPAQUE_SURFACE_EXPECTED)
        for name in (
            "shaped backdrop over a bright interval keeps its tinted body",
            "a dark interval forces the opaque body through legibility",
        ):
            result = named(name)
            body = result["body"] if "body" in result else result["paint"]["body"]
            self.assertEqual(body["geometry"], shape["geometry"])
            self.assertEqual(body["lighting"], shape["lighting"])

    def test_threshold_cases_straddle_the_certified_bound(self):
        def threshold(name):
            case = next(case for case in cases() if case["name"] == name)
            return next(change["value"] for change in case["requestChanges"]
                        if change["path"] == "/minimumContentContrast")

        retained = named("a threshold just below the certified bound keeps translucency")
        fallen = named("a threshold just above the certified bound falls back")
        certified = retained["body"]["contentContrastRatio"]
        below = threshold("a threshold just below the certified bound keeps translucency")
        above = threshold("a threshold just above the certified bound falls back")
        self.assertLess(below, certified)
        self.assertGreater(certified - below, 1e-13)
        self.assertGreater(above, certified)
        self.assertGreater(above - certified, 1e-13)
        self.assertEqual(fallen["fallbackReason"], "legibility")

    def test_results_differ_when_a_decision_member_changes(self):
        expected = named("focus falls back to the strong outline over a mid-tone surround")
        self.assertIsNone(translucent_mismatch(copy.deepcopy(expected), expected))
        for path in (
            ("frostRepresentation",),
            ("body", "pigment", "body", "alpha"),
            ("body", "pigment", "side", "components", 0),
            ("body", "backdrop", "compositedBody", "lower", "components", 0),
            ("body", "contentContrastRatio",),
            ("body", "edge", "colorRole"),
            ("body", "diffusion", "radius"),
            ("focus", "indicator", "colorRole"),
            ("focus", "indicator", "contrastRatio"),
        ):
            with self.subTest(path=path):
                actual = copy.deepcopy(expected)
                parent = actual
                for member in path[:-1]:
                    parent = parent[member]
                value = parent[path[-1]]
                parent[path[-1]] = (
                    "regularBackdrop" if value == "shapedBackdrop"
                    else "outline.strong" if value == "outline"
                    else "focus" if value == "outline.strong"
                    else value + 0.001 if isinstance(value, float) else value + 1
                )
                self.assertIsNotNone(translucent_mismatch(actual, expected))

    def test_opaque_results_differ_when_the_reason_or_body_changes(self):
        expected = named("a dark interval forces the opaque body through legibility")
        for edit in (
            lambda r: r.update(fallbackReason="selection"),
            lambda r: r["paint"]["body"].update(contentFallbackApplied=False),
            lambda r: r["paint"]["body"]["pigment"]["body"]["components"].__setitem__(0, 0.5),
        ):
            actual = copy.deepcopy(expected)
            edit(actual)
            self.assertIsNotNone(translucent_mismatch(actual, expected))


if __name__ == "__main__":
    unittest.main()
