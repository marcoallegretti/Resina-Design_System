import copy
import math
import unittest
from fractions import Fraction

from check_schemas import ROOT, apply_changes, load_json
from lit_body_arithmetic import (
    LOGICAL, PHYSICAL, Body, body_invariants, contour_difference, filled_label_bounds, layout_lit_outcomes,
    lit_body_source, lit_containment_outcomes, physical_radii, pixels, samples, swept_contour,
)
from material_relief_arithmetic import rectangular_samples

SOURCE = load_json(ROOT / "conformance/ir/lit-control-body-source.json")
CASES = load_json(ROOT / "conformance/ir/lit-control-body-vectors.json")
CONTAINMENT = load_json(ROOT / "conformance/content/command-lit-containment.json")


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


def containment_case(name):
    return next(case for case in CONTAINMENT["cases"] if case["name"] == name)


def containment_body(case, direction="ltr"):
    relief = dict(CONTAINMENT["profile"], shoulderWidth=case["shoulderWidth"])
    return Body(lit_body_source(SOURCE, case["size"], case.get("radius", CONTAINMENT["radius"]), relief,
                                case.get("edgeWidth", CONTAINMENT["edgeWidth"]), direction))


def token_value(trees, path):
    for tree in trees:
        node = tree
        for segment in path.split("."):
            node = node.get(segment) if isinstance(node, dict) else None
        if isinstance(node, dict) and "$value" in node:
            return node["$value"]
    raise KeyError(path)


class LitContainmentTests(unittest.TestCase):
    def test_matrix_outcomes_follow_the_exact_vertex_law(self):
        expected = {case["name"]: case["contained"] for case in CONTAINMENT["cases"]}
        outcomes = list(lit_containment_outcomes(CONTAINMENT, SOURCE))
        self.assertEqual(len(outcomes), 2 * len(expected))
        for name, direction, contained in outcomes:
            with self.subTest(case=name, direction=direction):
                self.assertEqual(contained, expected[name])

    def test_contained_labels_outside_the_published_rectangle_are_the_named_plateau_cases(self):
        beyond_c = set()
        for case in CONTAINMENT["cases"]:
            for direction in ("ltr", "rtl"):
                body = containment_body(case, direction)
                box = filled_label_bounds(case["size"], case["padding"], case["labelHeight"], direction)
                c = body.content_bounds
                inside_c = (box["x"] >= c["x"] and box["y"] >= c["y"]
                            and Fraction(box["x"]) + Fraction(box["width"]) <= Fraction(c["x"]) + Fraction(c["width"])
                            and Fraction(box["y"]) + Fraction(box["height"]) <= Fraction(c["y"]) + Fraction(c["height"]))
                if inside_c:
                    self.assertTrue(body.uniform_contains_box(box), case["name"])
                elif case["contained"]:
                    beyond_c.add(case["name"])
        self.assertTrue({
            "text-scale-1-shoulder-3", "text-scale-2-shoulder-3", "start-padding-10-inside-plateau",
            "end-padding-10-inside-plateau", "compact-minimum-inline-12-fills-block", "compact-text-scale-1-one-line",
            "plateau-arc-vertex", "plateau-arc-inside-by-2-40", "plateau-on-edge-interior-boundary",
        } <= beyond_c, beyond_c)

    def test_vertices_two_to_the_minus_forty_either_side_of_the_plateau_arc(self):
        body = containment_body(containment_case("plateau-arc-vertex"))
        tiny = Fraction(2) ** -40
        # The plateau disc about (24, 24) has squared radius 24*24 - 4*3.65625*24 = 15*15.
        on = (Fraction(15), Fraction(12))
        self.assertTrue(body.uniform_contains(on))
        self.assertTrue(body.uniform_contains((on[0], on[1] + tiny)))
        self.assertTrue(body.uniform_contains((on[0] + tiny, on[1])))
        self.assertFalse(body.uniform_contains((on[0], on[1] - tiny)))
        self.assertFalse(body.uniform_contains((on[0] - tiny, on[1])))
        self.assertEqual(body.normal((15.0, 12.0 + 2.0 ** -40)), (0.0, 0.0, 1.0))
        self.assertNotEqual(body.normal((15.0, 12.0 - 2.0 ** -20)), (0.0, 0.0, 1.0))

    def test_straight_shoulder_and_edge_interior_reject_vertices_separately(self):
        square = containment_body(containment_case("square-body-inside-straight-shoulder"))
        self.assertTrue(square.exact_contains((Fraction(16), Fraction(3, 2)), square.edge_width))
        self.assertFalse(square.uniform_contains((Fraction(16), Fraction(3, 2))))
        self.assertTrue(square.uniform_contains((Fraction(16), Fraction(2))))
        wide = containment_body(containment_case("plateau-outside-edge-interior-side"))
        point = (24.0, 4.75)
        self.assertEqual(wide.normal(point), (0.0, 0.0, 1.0))
        self.assertEqual(wide.relief_height(point), wide.amplitude)
        self.assertFalse(wide.uniform_contains((Fraction(24), Fraction(19, 4))))
        self.assertTrue(wide.uniform_contains((Fraction(24), Fraction(5))))
        corner = containment_body(containment_case("plateau-outside-edge-interior-corner"))
        self.assertEqual(corner.normal((10.25, 10.25)), (0.0, 0.0, 1.0))
        self.assertFalse(corner.uniform_contains((Fraction(41, 4), Fraction(41, 4))))
        self.assertTrue(corner.uniform_contains((Fraction(43, 4), Fraction(43, 4))))

    def test_plateau_law_agrees_with_the_relief_gates(self):
        margin = 1e-6
        checked = {True: 0, False: 0}
        for name in ("text-scale-1-reference-shoulder", "plateau-arc-vertex", "compact-text-scale-1-one-line",
                     "plateau-outside-edge-interior-side"):
            body = containment_body(containment_case(name))
            steps = 41
            for i in range(steps + 1):
                for j in range(steps + 1):
                    x = body.width * i / steps
                    y = body.height * j / steps
                    inside = body.uniform_contains((Fraction(x), Fraction(y)))
                    nudged = [body.uniform_contains((Fraction(x + dx), Fraction(y + dy)))
                              for dx in (-margin, margin) for dy in (-margin, margin)]
                    if any(value != inside for value in nudged) or not body.exact_contains(
                            (Fraction(x), Fraction(y)), body.edge_width):
                        continue
                    flat = body.normal((x, y)) == (0.0, 0.0, 1.0) and body.relief_height((x, y)) == body.amplitude
                    with self.subTest(case=name, point=(x, y)):
                        self.assertEqual(flat, inside)
                    checked[inside] += 1
        self.assertGreater(min(checked.values()), 300, checked)


class LayoutLitMatrixTests(unittest.TestCase):
    def test_every_density_text_scale_and_wrap_contains_its_label_in_the_lit_body(self):
        foundation = load_json(ROOT / "tokens/foundation.json")
        cases = load_json(ROOT / "conformance/layout/command-layout-cases.json")
        request = next(case for case in cases if case["name"] == "compact text 1 fine ltr")["request"]
        appearance = load_json(ROOT / "definitions/tier0-surface-appearance.json")
        relief = load_json(ROOT / "definitions/control-relief.json")["commandBody"]
        shape = load_json(ROOT / "definitions/tier0-shapes.json")["profiles"]["rounded"]["radius"]
        radius = token_value([foundation], shape)
        self.assertEqual(radius["unit"], "px")
        for theme_path in ("tokens/themes/light.json", "tokens/themes/dark.json"):
            theme = load_json(ROOT / theme_path)
            trees = [foundation] + theme["tokenResolver"]["sets"]["theme"]["sources"]
            binding = {}
            for role, path in theme["spatialAssignments"]["roles"].items():
                binding[role] = ({mode: token_value(trees, value) for mode, value in path.items()}
                                 if isinstance(path, dict) else token_value(trees, path))
            self.assertEqual(binding, request["spatialBinding"], theme_path)
            typography = theme["typographyAssignments"]["roles"]["label"]
            self.assertEqual(typography["minimumTextScale"], 1)
            label = {"fontSize": token_value(trees, typography["fontSize"])["value"],
                     "lineHeight": token_value(trees, typography["lineHeight"])}
            edge = appearance["bands"]["elastomer"]["edgeWidth"]
            outcomes = list(layout_lit_outcomes(request, label, SOURCE, radius["value"], relief, edge,
                                                (1, 1.5, 2), (1, 2)))
            self.assertEqual(len(outcomes), 4 * 3 * 2 * 2)
            for density, scale, lines, direction, bounds, contained in outcomes:
                with self.subTest(theme=theme_path, density=density, scale=scale, lines=lines, direction=direction):
                    self.assertTrue(contained, bounds)


if __name__ == "__main__":
    unittest.main()
