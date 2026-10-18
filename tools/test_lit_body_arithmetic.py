import copy
import math
import unittest

from check_schemas import ROOT, apply_changes, load_json
from lit_body_arithmetic import (
    LOGICAL, PHYSICAL, Body, body_invariants, contour_difference, physical_radii, pixels, samples, swept_contour,
)
from material_relief_arithmetic import rectangular_samples

SOURCE = load_json(ROOT / "conformance/ir/lit-control-body-source.json")
CASES = load_json(ROOT / "conformance/ir/lit-control-body-vectors.json")


def case_source(name):
    case = next(case for case in CASES if case["name"] == name)
    return apply_changes(SOURCE, case["sourceChanges"])


class LitBodyVectorTests(unittest.TestCase):
    def test_vectors_reproduce_from_independent_arithmetic(self):
        for case in CASES:
            source = apply_changes(SOURCE, case["sourceChanges"])
            with self.subTest(case=case["name"]):
                if "errorContains" in case:
                    with self.assertRaises(ValueError) as raised:
                        samples(source)
                    self.assertIn(case["errorContains"], str(raised.exception))
                    continue
                body, actual = samples(source)
                expected = case["expected"]
                self.assertEqual([(s["rule"], s["color"]) for s in actual],
                                 [(s["rule"], s["color"]) for s in expected["samples"]])
                self.assertEqual({name: body.light[name] for name in ("keyDirection", "ambient", "interaction")}, expected["light"])
                self.assertEqual({"x": body.offset[0], "y": body.offset[1]}, expected["sideOffset"])
                self.assertEqual(body.content_bounds, expected["contentBounds"])
                self.assertEqual(body.color_ranges(), expected["colorRanges"])

    def test_every_region_rule_and_admission_outcome_is_covered(self):
        rules = {s["rule"] for case in CASES if "expected" in case for s in case["expected"]["samples"]}
        self.assertEqual(rules, set(range(1, 8)))
        admitted = {case["name"] for case in CASES if "expected" in case and case["expected"]["light"]["interaction"]}
        self.assertEqual(admitted, {
            "hover-admits-interaction", "pressed-admits-interaction", "checked-hover-admits-interaction",
            "reduced-transparency-keeps-light", "full-quality-keeps-authored-strength", "steep-key-direction",
            "frost-opaque-dimensional-body", "elastomer-body", "strong-edge-candidate", "recessed-toggle-track",
            "convex-toggle-thumb", "asymmetric-corners-ltr", "asymmetric-corners-rtl", "flat-relief-keeps-projection",
            "square-corners", "hover-depth-zero-is-the-face", "circular-arc-boundaries",
            "rich-capabilities-change-no-light",
        })

    def test_capabilities_beyond_dynamic_lighting_change_no_paint(self):
        expected = {case["name"]: case["expected"] for case in CASES if "expected" in case}
        self.assertEqual(expected["rich-capabilities-change-no-light"], expected["hover-admits-interaction"])

    def test_content_plateau_and_flat_normals_return_the_exact_body(self):
        for case in CASES:
            if "expected" not in case:
                continue
            source = apply_changes(SOURCE, case["sourceChanges"])
            bounds = case["expected"]["contentBounds"]
            body = Body(source)
            with self.subTest(case=case["name"]):
                for x in (bounds["x"], bounds["x"] + bounds["width"] / 3, bounds["x"] + bounds["width"]):
                    for y in (bounds["y"], bounds["y"] + bounds["height"] / 2, bounds["y"] + bounds["height"]):
                        self.assertEqual(body.sample({"x": x, "y": y})[1], source["body"])

    def test_arc_boundaries_bracket_the_outline_and_pin_the_exact_edge_interior(self):
        body = Body(case_source("circular-arc-boundaries"))
        tiny = 2.0 ** -40
        case = next(case for case in CASES if case["name"] == "circular-arc-boundaries")
        points = [sample["point"] for sample in case["expected"]["samples"]]
        self.assertNotIn({"x": 80, "y": 62}, points)
        self.assertEqual([body.sample({"x": 80, "y": y})[0] for y in (62 - tiny, 62, 62 + tiny)], [3, 3, 5])
        self.assertEqual([body.sample({"x": 85, "y": y})[0] for y in (58 - tiny, 58, 58 + tiny)], [2, 2, 3])
        self.assertEqual([body.sample({"x": 11, "y": y})[0] for y in (6, 6 + tiny)], [2, 2])

    def test_shadow_boundary_of_the_front_takes_face_paint(self):
        body = Body(case_source("hover-admits-interaction"))
        self.assertEqual(body.sample({"x": 48, "y": 48}), (3, SOURCE["body"]))
        self.assertEqual(body.sample({"x": 48, "y": 0})[0], 4)
        self.assertEqual(body.sample({"x": 48, "y": 1})[0], 2)


class LitBodyGeometryTests(unittest.TestCase):
    def test_circular_normals_match_central_height_differences(self):
        for name in ("hover-admits-interaction", "recessed-toggle-track", "convex-toggle-thumb", "asymmetric-corners-rtl"):
            body = Body(case_source(name))
            step = 1e-6
            for x in [body.width * k / 37 for k in range(1, 37)]:
                for y in [body.height * k / 23 for k in range(1, 23)]:
                    if not body.contains((x, y), 0) or not body.contains((x - step, y - step), 0) or not body.contains((x + step, y + step), 0):
                        continue
                    if not all(body.contains((x + dx, y + dy), 0) for dx in (-step, step) for dy in (-step, step)):
                        continue
                    nx, ny, nz = body.normal((x, y))
                    gx = (body.relief_height((x + step, y)) - body.relief_height((x - step, y))) / (2 * step)
                    gy = (body.relief_height((x, y + step)) - body.relief_height((x, y - step))) / (2 * step)
                    with self.subTest(case=name, x=x, y=y):
                        self.assertAlmostEqual(-nx / nz, gx, delta=1e-6)
                        self.assertAlmostEqual(-ny / nz, gy, delta=1e-6)
                        self.assertAlmostEqual(nx * nx + ny * ny + nz * nz, 1, places=14)

    def test_peak_tilt_matches_the_measured_command_shoulder(self):
        body = Body(case_source("hover-admits-interaction"))
        peak = max(math.hypot(*body.normal((x / 16, y / 16))[:2]) for x in range(0, 24 * 16 + 1) for y in range(0, 24 * 16 + 1))
        self.assertAlmostEqual(peak, 0.469, delta=5e-4)

    def test_edge_interior_equals_the_independently_inset_circular_contour(self):
        for name in ("hover-admits-interaction", "asymmetric-corners-ltr", "recessed-toggle-track", "convex-toggle-thumb"):
            body = Body(case_source(name))
            e = body.edge_width
            inner = [max(radius - e, 0.0) for radius in body.radii]
            width, height = body.width - 2 * e, body.height - 2 * e
            for i in range(-3, 160):
                for j in range(-3, 90):
                    x, y = i * body.width / 150 + 0.0137, j * body.height / 80 + 0.0071
                    px, py = x - e, y - e
                    inside = 0 <= px <= width and 0 <= py <= height
                    for radius, (cx, cy), (sx, sy) in zip(inner, ((0, 0), (width, 0), (width, height), (0, height)), ((-1, -1), (1, -1), (1, 1), (-1, 1))):
                        if radius and inside:
                            ox, oy = cx - sx * radius, cy - sy * radius
                            if sx * (px - ox) > 0 and sy * (py - oy) > 0:
                                inside = math.hypot(px - ox, py - oy) <= radius
                    with self.subTest(case=name, x=x, y=y):
                        self.assertEqual(body.contains((x, y), e), inside)

    def test_swept_membership_matches_dense_parameter_search(self):
        for name in ("hover-admits-interaction", "steep-key-direction", "asymmetric-corners-rtl", "square-corners"):
            body = Body(case_source(name))
            dx, dy = body.offset
            for inset in (0.0, body.edge_width):
                for i in range(-2, 34):
                    for j in range(-2, 20):
                        point = (i * (body.width + 4) / 31 - 2 + 0.0113, j * (body.height + 4) / 17 - 2 + 0.0097)
                        decided = body.swept(point, inset, 3e-2)
                        if decided is None:
                            continue
                        found = any(body.contains((point[0] - t * dx, point[1] - t * dy), inset) for t in (k / 200 for k in range(201)))
                        with self.subTest(case=name, inset=inset, point=point):
                            self.assertEqual(decided, found)

    def test_swept_contours_reproduce_the_public_circular_extrusion_vectors(self):
        cases = load_json(ROOT / "conformance/geometry/extruded-contour-vectors.json")
        checked = 0
        for case in cases:
            request = case["request"]
            segments = (case.get("expected") or {}).get("segments")
            if not segments or request.get("layoutDirection") not in LOGICAL:
                continue
            corners = [request["radii"][name] for name in LOGICAL[request["layoutDirection"]]]
            if any(corner["x"] != corner["y"] for corner in corners):
                continue
            width, height = request["size"]["width"], request["size"]["height"]
            sums = ((corners[0]["x"] + corners[1]["x"], width), (corners[3]["x"] + corners[2]["x"], width),
                    (corners[0]["x"] + corners[3]["x"], height), (corners[1]["x"] + corners[2]["x"], height))
            factor = min([1.0] + [limit / total for total, limit in sums if total > 0])
            radii = [corner["x"] * factor for corner in corners]
            offset = (request["offset"]["x"], request["offset"]["y"])
            with self.subTest(case=case["name"]):
                self.assertIsNone(contour_difference(segments, swept_contour(width, height, radii, offset)))
            checked += 1
        self.assertGreaterEqual(checked, 9)

    def test_layout_direction_maps_logical_corners_without_mirroring_light(self):
        ltr, rtl = case_source("asymmetric-corners-ltr"), case_source("asymmetric-corners-rtl")
        self.assertEqual(physical_radii(ltr), [20, 12, 16, 10])
        self.assertEqual(physical_radii(rtl), [12, 20, 10, 16])
        self.assertEqual(Body(ltr).offset, Body(rtl).offset)
        changed = [s for s, t in zip(CASES[[c["name"] for c in CASES].index("asymmetric-corners-ltr")]["expected"]["samples"],
                                     CASES[[c["name"] for c in CASES].index("asymmetric-corners-rtl")]["expected"]["samples"]) if s != t]
        self.assertGreater(len(changed), 10)

    def test_small_radii_are_rejected_with_the_part_shoulder_diagnostic(self):
        source = copy.deepcopy(SOURCE)
        for radius in (8, 6, 1):
            source["radii"] = {name: {"x": radius, "y": radius} for name in source["radii"]}
            with self.subTest(radius=radius), self.assertRaisesRegex(ValueError, f"topLeft corner radius {radius} px cannot carry shoulderWidth 2 px"):
                Body(source)
        source["radii"] = {name: {"x": 8.0001, "y": 8.0001} for name in source["radii"]}
        Body(source)


class LitBodyEquivalenceTests(unittest.TestCase):
    def test_depth_zero_square_body_equals_the_face_oracle(self):
        for states in (["rest"], ["hover"], ["pressed"]):
            source = case_source("square-corners")
            source["depth"] = 0
            source["identity"]["states"]["states"] = states
            source["points"] = [{"x": x / 4, "y": y / 4} for x in range(-4, 390, 7) for y in range(-4, 198, 5)]
            _, lit = samples(source)
            request = {
                "schemaVersion": "0.1.0",
                "outline": {"schemaVersion": "0.1.0", "size": source["size"], "radii": source["radii"], "layoutDirection": source["environment"]["layoutDirection"], "curvature": "circular", "minimumInset": source["edgeWidth"]},
                "profile": source["relief"],
                "pigment": {"schemaVersion": "0.1.0", "materialFamily": source["identity"]["materialFamily"], "body": source["body"], "profiles": source["pigmentProfiles"]},
                "light": {"schemaVersion": "0.1.0", "keyLight": source["keyLight"], **source["light"]},
                "environment": source["environment"],
                "enabled": True,
                "interactionActive": states[0] in ("hover", "pressed"),
                "edge": source["edge"], "edgeWidth": source["edgeWidth"], "foreground": source["foreground"],
                "minimumContentContrast": source["minimumContentContrast"], "points": source["points"],
            }
            _, face = rectangular_samples(request)
            with self.subTest(states=states):
                self.assertEqual({s["rule"] for s in lit}, {1, 7})
                self.assertEqual([s["color"] for s in lit], [s["color"] for s in face])


class LitBodyIrTests(unittest.TestCase):
    def test_published_ir_fixture_agrees_with_independent_arithmetic(self):
        ir = load_json(ROOT / "conformance/ir/lit-control-body-ir.json")
        body = Body(case_source("hover-admits-interaction"))
        self.assertIsNone(body_invariants(ir))

        def close(actual, expected, path=""):
            if isinstance(expected, dict):
                self.assertEqual(set(actual), set(expected), path)
                for name in expected:
                    close(actual[name], expected[name], f"{path}/{name}")
            elif isinstance(expected, list):
                self.assertEqual(len(actual), len(expected), path)
                for index, (a, e) in enumerate(zip(actual, expected)):
                    close(a, e, f"{path}/{index}")
            elif isinstance(expected, (int, float)) and not isinstance(expected, bool):
                self.assertLessEqual(abs(actual - expected), max(1e-12, 1e-12 * abs(expected)), path)
            else:
                self.assertEqual(actual, expected, path)

        close(ir["face"]["light"], {name: body.light[name] for name in ("keyDirection", "ambient", "interaction")})
        close(ir["face"]["pigment"], body.pigment)
        close(ir["face"]["contentBounds"], body.content_bounds)
        close(ir["face"]["colorRanges"], body.color_ranges())
        close(ir["face"]["contentContrastRatio"], body.content_contrast)
        close(ir["lighting"]["sideOffset"], {"x": body.offset[0], "y": body.offset[1]})
        close(ir["face"]["surface"]["outline"]["corners"], {name: {"x": r, "y": r} for name, r in zip(PHYSICAL, body.radii)})
        self.assertEqual(ir["depth"], body.depth)
        self.assertNotIn("highlightWidth", ir)

    def test_invariants_reject_competing_or_inconsistent_members(self):
        ir = load_json(ROOT / "conformance/ir/lit-control-body-ir.json")
        for pointer, value in (
            ("/face/light/keyDirection", {"x": -1, "y": 0}),
            ("/states/states", ["disabled", "hover"]),
            ("/depth", 3),
            ("/geometry/edgeInterior/contour/bounds/height", 48),
            ("/face/surface/outline/curvature", "quartic"),
            ("/face/contentBounds/width", 94),
        ):
            with self.subTest(pointer=pointer):
                self.assertIsNotNone(body_invariants(apply_changes(ir, [{"path": pointer, "value": value}])))


class LitBodyPixelTests(unittest.TestCase):
    def test_pixels_cover_plateau_side_edge_and_clear_regions(self):
        source = case_source("hover-admits-interaction")
        body_pixel = pixels(source, 1, 1, (47.5, 23.5), 1, 3)[0]
        self.assertEqual(body_pixel[0], body_pixel[1])
        self.assertEqual(body_pixel[0][3], 255)
        self.assertEqual(body_pixel[0][:3], (128, 102, 77))
        clear = pixels(source, 1, 1, (-3, -3), 1, 3)[0]
        self.assertEqual(clear, ((0, 0, 0, 0), (0, 0, 0, 0)))
        side = pixels(source, 1, 1, (47.9, 48.2), 10, 1)[0]
        self.assertEqual(side[0], side[1])
        self.assertEqual(side[0][:3], (115, 92, 69))
        partial = pixels(source, 1, 1, (48.9, 48.9), 1, 3)[0]
        self.assertEqual(partial[0], partial[1])
        self.assertLess(partial[0][3], 255)

    def test_subsamples_on_exact_swept_boundaries_are_bracketed(self):
        source = case_source("square-corners")
        source["keyLight"]["direction"] = {"x": 0, "y": -1}
        body = Body(source)
        self.assertEqual(body.offset, (-0.0, 2.0))
        bounds = pixels(source, 1, 1, (48 - 0.5, 50 - 0.5), 1, 1)[0]
        self.assertEqual(bounds[0][3], 0)
        self.assertEqual(bounds[1], (0, 0, 0, 255))
        settled = pixels(source, 1, 1, (48 - 0.5, 50 - 0.5), 1, 1, band=0.0)[0]
        self.assertEqual(settled[0], settled[1])


if __name__ == "__main__":
    unittest.main()
