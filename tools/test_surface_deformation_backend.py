import copy
import unittest
from fractions import Fraction

import surface_deformation_arithmetic as arithmetic
from check_schemas import ROOT, apply_changes, load_json
from check_surface_outline_backend import outline_mismatch
from check_surface_deformation_backend import (
    KINDS,
    baseline,
    case_request,
    cases,
    expected_result,
    motion_mismatch,
)


def accepted_cases(kind):
    return [case for case in cases(kind) if "errorContains" not in case]


def request_of(kind, name):
    case = next(case for case in cases(kind) if case["name"] == name)
    return case_request(baseline(kind), case)


class OracleStatesTheCases(unittest.TestCase):
    def test_cases_state_the_policy_the_oracle_derives(self):
        for kind in KINDS:
            for case in accepted_cases(kind):
                with self.subTest(kind=kind, name=case["name"]):
                    result = expected_result(kind, case_request(baseline(kind), case))
                    self.assertEqual(result["policy"], case["expectedPolicy"])

    def test_semantic_rejections_name_their_reason(self):
        for kind in KINDS:
            rejected = [c for c in cases(kind) if "errorContains" in c and c["requestSchemaValid"]]
            self.assertGreaterEqual(len(rejected), 7)
            for case in rejected:
                with self.subTest(kind=kind, name=case["name"]):
                    with self.assertRaisesRegex(arithmetic.Reject, f"(?i){case['errorContains']}"):
                        expected_result(kind, case_request(baseline(kind), case))

    def test_every_policy_and_projection_marker_is_covered(self):
        markers = {}
        for kind, channels in KINDS.items():
            results = [expected_result(kind, case_request(baseline(kind), c)) for c in accepted_cases(kind)]
            self.assertEqual({r["policy"] for r in results}, {"spring", "castImmediate", "reducedMotion"})
            markers[kind] = {name: {r["projection"][name] for r in results} for name in channels}
        every = {"none", "lowerBound", "upperBound"}
        self.assertEqual(set.union(*markers["deformation"].values()), every)
        self.assertIn("upperBound", markers["emergence"]["progress"])
        self.assertIn("lowerBound", markers["emergence"]["opacity"])

    def test_both_content_outcomes_are_covered_by_emergence_cases(self):
        results = [expected_result("emergence", case_request(baseline("emergence"), c))
                   for c in accepted_cases("emergence")]
        self.assertTrue(any(r["fromContent"] is None for r in results))
        self.assertTrue(any(r["toContent"] is None for r in results))
        self.assertTrue(any(r["fromContent"] and r["toContent"] for r in results))


class OutlineArithmeticMatchesThePublicVectors(unittest.TestCase):
    def test_every_accepted_surface_outline_vector_is_reproduced(self):
        vectors = [v for v in load_json(ROOT / "conformance/geometry/surface-outline-vectors.json") if "expected" in v]
        self.assertGreaterEqual(len(vectors), 6)
        for vector in vectors:
            request = vector["request"]
            with self.subTest(name=vector["name"]):
                size = request["size"]
                actual = arithmetic.published(arithmetic.rest_outline(
                    request, Fraction(size["width"]), Fraction(size["height"])))
                self.assertIsNone(outline_mismatch(actual, vector["expected"]))


class DeformationDomain(unittest.TestCase):
    def grid(self, request, steps=8):
        width, height, domains = arithmetic.deformation_domain(request)
        lower_w, upper_w = domains["widthScale"]
        lower_h, upper_h = domains["heightScale"]
        for i in range(steps + 1):
            for j in range(steps + 1):
                yield (
                    width, height,
                    lower_w + (upper_w - lower_w) * i / steps,
                    lower_h + (upper_h - lower_h) * j / steps,
                )

    def test_corner_check_agrees_with_a_dense_grid_for_content_containment(self):
        base = baseline("deformation")
        outlines = [
            base["outline"],
            request_of("deformation", "right to left quartic asymmetric corners")["outline"],
        ]
        disagreements = 0
        for outline in outlines:
            for x in [Fraction(70 + step, 10) for step in range(0, 21, 2)]:
                for y in (Fraction(6), Fraction(8), Fraction(10)):
                    for width in (Fraction(30), Fraction(40), Fraction(50)):
                        request = copy.deepcopy(base)
                        request["outline"] = outline
                        box = (x, y, width, Fraction(20))
                        corners = [
                            arithmetic.contained(box, *reversed(arithmetic.place(request, w, h, a, b)))
                            for w, h, a, b in self.grid(request, 1)
                        ]
                        dense = [
                            arithmetic.contained(box, *reversed(arithmetic.place(request, w, h, a, b)))
                            for w, h, a, b in self.grid(request)
                        ]
                        disagreements += all(corners) != all(dense)
        self.assertEqual(disagreements, 0)

    def test_the_worst_corner_is_not_always_the_smallest_silhouette(self):
        request = baseline("deformation")
        width, height, domains = arithmetic.deformation_domain(request)
        box = arithmetic.rectangle({"x": 7.6, "y": 10, "width": 36, "height": 20})
        outcome = {}
        for scale_x in domains["widthScale"]:
            for scale_y in domains["heightScale"]:
                origin, resolved = arithmetic.place(request, width, height, scale_x, scale_y)
                outcome[(float(scale_x), float(scale_y))] = arithmetic.contained(box, resolved, origin)
        self.assertTrue(outcome[(0.94, 0.94)])
        self.assertFalse(outcome[(0.94, 1.06)])

    def test_reservation_covers_every_sampled_silhouette_and_is_attained(self):
        request = baseline("deformation")
        sweep = arithmetic.sweep_of(request)
        result = expected_result("deformation", request)
        reserved = result["reservation"]
        left, top = Fraction(reserved["x"]), Fraction(reserved["y"])
        right, bottom = left + Fraction(reserved["width"]), top + Fraction(reserved["height"])
        extremes = []
        for width, height, scale_x, scale_y in self.grid(request, 12):
            origin, resolved = arithmetic.place(request, width, height, scale_x, scale_y)
            size = resolved["size"]
            for dx, dy in ((0, 0), sweep):
                extremes.append((origin[0] + dx, origin[1] + dy, origin[0] + dx + size["width"], origin[1] + dy + size["height"]))
        tolerance = Fraction(1, 10 ** 12)
        self.assertTrue(all(box[0] >= left - tolerance and box[1] >= top - tolerance
                            and box[2] <= right + tolerance and box[3] <= bottom + tolerance for box in extremes))
        self.assertAlmostEqual(float(min(box[0] for box in extremes)), float(left), places=9)
        self.assertAlmostEqual(float(max(box[3] for box in extremes)), float(bottom), places=9)

    def test_centre_is_fixed_for_every_scale(self):
        request = baseline("deformation")
        for width, height, scale_x, scale_y in self.grid(request, 6):
            origin, resolved = arithmetic.place(request, width, height, scale_x, scale_y)
            self.assertEqual(origin[0] + resolved["size"]["width"] / 2, width / 2)
            self.assertEqual(origin[1] + resolved["size"]["height"] / 2, height / 2)

    def test_family_limits_follow_the_recorded_decision(self):
        self.assertEqual(arithmetic.LIMITS, {"cast": 0.0, "frost": 0.02, "elastomer": 0.06})
        with self.assertRaisesRegex(arithmetic.Reject, "Gel"):
            arithmetic.policy_of({"family": "gel", "reducedMotion": True})


class EmergenceEndpoints(unittest.TestCase):
    def test_endpoint_outlines_are_exact(self):
        request = baseline("emergence")
        for end, time, emerged in (("from", 0, True), ("to", 5, True), ("from", 5, False)):
            changed = apply_changes(request, [{"path": "/time", "value": time}, {"path": "/emerged", "value": emerged}])
            result = expected_result("emergence", changed)
            size = changed[end]["outline"]["size"]
            expected = arithmetic.published(arithmetic.rest_outline(
                changed[end]["outline"], Fraction(size["width"]), Fraction(size["height"])))
            self.assertEqual(result["outline"], expected)
            self.assertEqual(result["origin"], {"x": float(changed[end]["origin"]["x"]), "y": float(changed[end]["origin"]["y"])})


class ComparisonDetectsChanges(unittest.TestCase):
    def mismatch(self, kind, edit):
        request = case_request(baseline(kind), cases(kind)[0])
        actual = copy.deepcopy(expected_result(kind, request))
        self.assertIsNone(motion_mismatch(kind, request, actual))
        edit(actual)
        return motion_mismatch(kind, request, actual)

    def test_deformation_members_are_compared(self):
        for name, edit in (
            ("reservation", lambda a: a["reservation"].update(width=a["reservation"]["width"] + 1e-6)),
            ("origin", lambda a: a["origin"].update(x=a["origin"]["x"] + 1e-6)),
            ("outline", lambda a: a["outline"]["contentBounds"].update(x=a["outline"]["contentBounds"]["x"] + 1e-6)),
            ("projection", lambda a: a["projection"].update(widthScale="lowerBound")),
            ("policy", lambda a: a.update(policy="castImmediate")),
            ("extra member", lambda a: a.update(unexpected=1)),
        ):
            with self.subTest(name=name):
                self.assertIsNotNone(self.mismatch("deformation", edit))

    def test_emergence_members_are_compared(self):
        for name, edit in (
            ("source content", lambda a: a["fromContent"].update(opacity=a["fromContent"]["opacity"] + 1e-6)),
            ("destination content", lambda a: a.update(toContent=None)),
            ("identity", lambda a: a.update(identity="badge")),
            ("sample", lambda a: a["progress"]["state"].update(position=a["progress"]["state"]["position"] + 1e-6)),
            ("reservation", lambda a: a["reservation"].update(x=a["reservation"]["x"] + 1e-6)),
        ):
            with self.subTest(name=name):
                self.assertIsNotNone(self.mismatch("emergence", edit))

    def test_numeric_noise_inside_the_geometry_tolerance_is_accepted(self):
        request = baseline("deformation")
        actual = copy.deepcopy(expected_result("deformation", request))
        actual["origin"]["x"] += 1e-13
        self.assertIsNone(motion_mismatch("deformation", request, actual))


if __name__ == "__main__":
    unittest.main()
