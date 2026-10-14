import copy
import math
import unittest
from decimal import Decimal

from check_material_relief_paint_backend import compare
from check_schemas import ROOT, load_json, replace_at_pointer, validator_for
from check_surface_relief_backend import relief_mismatch
from material_relief_arithmetic import rectangular_normal, rectangular_samples


class MaterialReliefProofTests(unittest.TestCase):
    def test_exact_decimal_source_rules_reject_erased_negative_channels(self):
        case = next(case for case in load_json(ROOT / "conformance/ir/material-relief-paint-vectors.json") if case["name"] == "cast-convex-opaque-face-active-light")
        validator = validator_for("schemas/material-relief-paint-request.schema.json")
        for pointer in ("/light/ambient", "/light/interaction/strength", "/pigment/profiles/profiles/gel/sideShade"):
            for literal, valid in (("-1e-400", False), ("-0e-400", True), ("1e-400", True)):
                request = copy.deepcopy(case["request"])
                replace_at_pointer(request, pointer, Decimal(literal))
                with self.subTest(pointer=pointer, literal=literal):
                    self.assertEqual(not list(validator.iter_errors(request)), valid)

    def test_half_shoulder_normal_retains_full_tilt_and_recess_inverts_it(self):
        cases = load_json(ROOT / "conformance/ir/material-relief-paint-vectors.json")
        request = copy.deepcopy(cases[1]["request"])
        slope = 0.5 / 4 * 15 / 8
        normal = rectangular_normal(request, {"x": 60, "y": 2})
        self.assertEqual(normal["x"], 0)
        self.assertAlmostEqual(normal["y"], -slope / math.sqrt(1 + slope * slope), places=15)
        self.assertGreater(normal["z"], 0.9)
        request["profile"]["kind"] = "recessed"
        recess = rectangular_normal(request, {"x": 60, "y": 2})
        self.assertEqual(recess["y"], -normal["y"])
        self.assertEqual(recess["z"], normal["z"])

    def test_authored_rectangular_lighting_and_guard_samples_are_independent(self):
        for case in load_json(ROOT / "conformance/ir/material-relief-paint-vectors.json"):
            if "expected" not in case or any(axis for corner in case["request"]["outline"]["radii"].values() for axis in corner.values()):
                continue
            with self.subTest(case=case["name"]):
                light, samples = rectangular_samples(case["request"])
                target = case["expected"]
                self.assertIsNone(relief_mismatch(samples, target["samples"]))
                self.assertIsNone(relief_mismatch({name: light[name] for name in ("keyDirection", "ambient", "interaction")}, target["paint"]["light"]))
                self.assertEqual(light["pigment"]["body"], case["request"]["pigment"]["body"])

    def test_mutation_cannot_change_retained_body_plateau_or_edge(self):
        expected = next(case["expected"] for case in load_json(ROOT / "conformance/ir/material-relief-paint-vectors.json") if case["name"] == "cast-convex-opaque-face-active-light")
        self.assertIsNone(compare(expected, expected))
        for path in ("body", "plateau", "edge", "range"):
            actual = copy.deepcopy(expected)
            if path == "body":
                actual["paint"]["pigment"]["body"]["components"][0] += 1e-14
            elif path == "plateau":
                actual["samples"][2]["color"]["components"][0] += 1e-14
            elif path == "edge":
                actual["samples"][3]["color"]["components"][0] += 1e-14
            else:
                actual["paint"]["colorRanges"][2]["upper"]["components"][0] += 1e-14
            with self.subTest(path=path):
                self.assertIsNotNone(compare(actual, expected))

    def test_absent_channels_preserve_authored_static_paint(self):
        originals = load_json(ROOT / "conformance/ir/relief-paint-vectors.json")
        cases = load_json(ROOT / "conformance/ir/material-relief-paint-vectors.json")
        for original, current in zip(originals, cases):
            if "expected" not in original:
                continue
            with self.subTest(case=current["name"]):
                self.assertEqual(current["expected"]["samples"], original["expected"]["samples"])
                paint = copy.deepcopy(current["expected"]["paint"])
                paint["direction"] = paint.pop("light")["keyDirection"]
                paint.pop("colorRanges")
                self.assertIsNone(relief_mismatch(paint, original["expected"]["paint"]))
                self.assertEqual(paint["pigment"]["body"], original["expected"]["paint"]["pigment"]["body"])


if __name__ == "__main__":
    unittest.main()
