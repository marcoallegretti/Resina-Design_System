import contextlib
import copy
import io
import sys
import unittest
from unittest.mock import patch

from PIL import Image
from check_guido_relief_native import board_dimensions, capture_matches, diagnostic_content_focus_request, diagnostic_outline_focus_request, diagnostic_request, main, validate_canvas_reference
from check_schemas import validator_for
import check_guido_relief_native as native


class NativeReliefComparisonTests(unittest.TestCase):
    def test_material_fixture_preserves_guards_and_has_one_active_light_source(self):
        original = diagnostic_request()
        request = native.diagnostic_material_request()
        self.assertEqual(list(validator_for("schemas/material-relief-paint-request.schema.json").iter_errors(request)), [])
        self.assertNotIn("keyLight", request)
        self.assertEqual(request["light"]["keyLight"], original["keyLight"])
        for member in ("outline", "profile", "pigment", "edge", "edgeWidth", "foreground", "minimumContentContrast", "points"):
            self.assertEqual(request[member], original[member])
        self.assertIsNotNone(request["light"]["ambient"])
        self.assertIsNotNone(request["light"]["interaction"])
        self.assertTrue(request["enabled"])
        self.assertTrue(request["interactionActive"])
        self.assertTrue(request["environment"]["rendererCapabilities"]["dynamicLighting"])

    def test_material_fixture_focus_retains_the_actual_guarded_footprint(self):
        paint = native.diagnostic_material_request()
        focus = diagnostic_content_focus_request(paint)
        self.assertEqual(list(validator_for("schemas/outline-focus-request.schema.json").iter_errors(focus)), [])
        self.assertEqual(focus["outline"], paint["outline"])
        self.assertEqual(focus["focus"]["surroundingColor"], paint["edge"]["adjacentColor"])

    def test_board_reference_requires_complete_opaque_integral_surface_dimensions(self):
        image = Image.new("RGBA", (10, 15), (2, 3, 4, 255))
        self.assertEqual(board_dimensions(image, 1.25), (8, 12))
        with self.assertRaisesRegex(ValueError, "integral logical"):
            board_dimensions(image, 1.5)
        image.putpixel((9, 14), (2, 3, 4, 254))
        with self.assertRaisesRegex(ValueError, "opaque canvas"):
            board_dimensions(image, 1.25)

    def test_board_mode_rejects_missing_sources_and_mixed_probe_modes(self):
        for extra in ([], ["--content-font", "font"], ["--request", "scenes"],
                      ["--content-font", "font", "--request", "scenes", "--content-focus"],
                      ["--content-font", "font", "--request", "scenes", "--outline-focus"],
                      ["--content-font", "font", "--request", "scenes", "--material-light"]):
            arguments = ["checker", "--probe", "probe", "--output-dir", "unused", "--relief-board", "convex", *extra]
            with patch.object(sys, "argv", arguments), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as raised:
                    main()
                self.assertEqual(raised.exception.code, 2)

    def test_content_focus_supports_every_authored_material_family(self):
        for family in ("cast", "frost", "elastomer", "gel"):
            paint = diagnostic_request()
            paint["pigment"]["materialFamily"] = family
            request = diagnostic_content_focus_request(paint)
            scenario = request["focus"]["scenario"]
            namespace, role = scenario["surface"]["materialRole"].split(".")
            self.assertEqual(scenario["resolution"]["materialAssignments"][namespace][role], family)
            self.assertEqual(list(validator_for("schemas/outline-focus-request.schema.json").iter_errors(request)), [])

    def test_content_focus_diagnoses_an_unassigned_material_family(self):
        paint = diagnostic_request()
        paint["pigment"]["materialFamily"] = "unknown"
        with self.assertRaisesRegex(ValueError, "no authored focus material role"):
            diagnostic_content_focus_request(paint)

    def test_content_focus_retains_actual_outline_canvas_family_and_concurrent_states(self):
        from check_focus_indicator_backend import contrast
        for canvas in ([1, 1, 1], [0.01, 0.015, 0.02]):
            paint = diagnostic_request()
            paint["edge"]["adjacentColor"]["components"] = canvas
            original = copy.deepcopy(paint)
            request = diagnostic_content_focus_request(paint)
            self.assertEqual(list(validator_for("schemas/outline-focus-request.schema.json").iter_errors(request)), [])
            self.assertEqual(request["outline"], original["outline"])
            self.assertEqual(paint, original)
            self.assertEqual(request["sideOffset"], {"x": 0, "y": 0})
            self.assertEqual(request["focus"]["surroundingColor"], paint["edge"]["adjacentColor"])
            self.assertEqual(request["focus"]["scenario"]["surface"]["states"]["states"], ["selected", "focused", "hover"])
            resolution = request["focus"]["scenario"]["resolution"]
            token = resolution["opaqueColorAssignments"]["roles"]["focus"].split(".")[1]
            self.assertGreaterEqual(contrast(resolution["tokens"]["palette"][token]["$value"]["components"], canvas), 3)
            request["outline"]["radii"]["topStart"]["x"] += 1
            self.assertEqual(paint, original)

    def test_content_focus_requires_an_actual_complete_content_probe(self):
        with patch.object(sys, "argv", ["checker", "--content-focus", "--probe", "probe", "--output-dir", "unused"]), contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as raised:
                main()
            self.assertEqual(raised.exception.code, 2)

    def test_focus_usage_rejects_mixed_probe_modes_and_partial_reference_pixels(self):
        for extra in [["--content-font", "font.ttf"], ["--scales", "1.2"], ["--scales", "3.5"]]:
            arguments = ["checker", "--outline-focus", "--probe", "probe", "--output-dir", "unused", *extra]
            with self.subTest(extra=extra), patch.object(sys, "argv", arguments), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as raised:
                    main()
                self.assertEqual(raised.exception.code, 2)

    def test_default_focus_fixture_preserves_actual_signed_sweep_and_schema(self):
        request = diagnostic_outline_focus_request()
        self.assertEqual(list(validator_for("schemas/outline-focus-request.schema.json").iter_errors(request)), [])
        self.assertEqual(request["outline"]["layoutDirection"], "rtl")
        self.assertEqual(request["sideOffset"], {"x": -6, "y": 4})
        self.assertEqual(request["directions"], [])

    def test_full_canvas_focus_reference_rejects_hidden_partial_pixels(self):
        reference = Image.new("RGBA", (200, 120), (120, 80, 40, 255))
        validate_canvas_reference(reference, (160, 96), 1.25)
        reference.putpixel((199, 119), (120, 80, 40, 254))
        with self.assertRaisesRegex(ValueError, "opaque"):
            validate_canvas_reference(reference, (160, 96), 1.25)

    def test_full_canvas_reference_rejects_resampled_or_cropped_dimensions(self):
        for size in [(199, 120), (200, 119), (160, 96)]:
            with self.subTest(size=size), self.assertRaisesRegex(ValueError, "dimensions"):
                validate_canvas_reference(Image.new("RGBA", size, (0, 0, 0, 255)), (160, 96), 1.25)

    def test_opaque_reference_covers_complete_content_canvas(self):
        reference = Image.new("RGBA", (3, 2), (120, 80, 40, 255))
        capture = Image.new("RGBA", (8, 6), (255, 255, 255, 255))
        capture.paste(reference, (2, 1))
        self.assertEqual(capture_matches(capture, reference, (2, 1)), (6, 0))
        capture.putpixel((4, 2), (120, 82, 40, 255))
        self.assertEqual(capture_matches(capture, reference, (2, 1)), (6, 2))

    def test_complete_reference_cannot_be_clipped(self):
        reference = Image.new("RGBA", (3, 2), (120, 80, 40, 255))
        capture = Image.new("RGBA", (3, 2), (255, 255, 255, 255))
        for origin in [(-1, 0), (0, -1), (1, 0), (0, 1)]:
            with self.subTest(origin=origin), self.assertRaisesRegex(ValueError, "clips"):
                capture_matches(capture, reference, origin)

    def test_face_only_scope_does_not_claim_partial_coverage(self):
        reference = Image.new("RGBA", (2, 1), (0, 0, 0, 0))
        capture = Image.new("RGBA", (2, 1), (200, 200, 200, 255))
        reference.putpixel((0, 0), (200, 200, 200, 255))
        reference.putpixel((1, 0), (0, 0, 0, 128))
        self.assertEqual(capture_matches(capture, reference, (0, 0)), (1, 0))
        reference.putpixel((0, 0), (200, 200, 200, 128))
        with self.assertRaisesRegex(ValueError, "no opaque"):
            capture_matches(capture, reference, (0, 0))


if __name__ == "__main__":
    unittest.main()
