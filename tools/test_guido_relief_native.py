import contextlib
import io
import sys
import unittest
from unittest.mock import patch

from PIL import Image
from check_guido_relief_native import capture_matches, diagnostic_outline_focus_request, main, validate_canvas_reference
from check_schemas import validator_for


class NativeReliefComparisonTests(unittest.TestCase):
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
