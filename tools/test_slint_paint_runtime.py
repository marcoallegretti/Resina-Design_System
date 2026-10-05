import unittest
import sys

from PIL import Image

from check_slint_paint_runtime import compare_images, run


class PaintComparisonTests(unittest.TestCase):
    def setUp(self):
        self.background = Image.new("RGBA", (12, 10), (32, 32, 32, 255))
        self.expected = self.background.copy()
        paint = Image.new("RGBA", (4, 3), (80, 150, 120, 255))
        paint.putpixel((0, 0), (80, 150, 120, 128))
        self.expected.alpha_composite(paint, (3, 4))

    def test_single_byte_compositing_rounding_is_accepted(self):
        actual = self.expected.copy()
        pixel = actual.getpixel((3, 4))
        actual.putpixel((3, 4), (pixel[0] + 1, pixel[1] - 1, pixel[2], 255))
        compare_images(self.expected, actual)

    def test_missing_clipped_shifted_pigment_and_alpha_errors_fail(self):
        clipped = self.expected.copy()
        clipped.paste(self.background.crop((6, 4, 7, 7)), (6, 4))
        shifted = self.background.copy()
        shifted.paste(self.expected.crop((3, 4, 7, 7)), (4, 4))
        opaque = self.expected.copy()
        opaque.putpixel((4, 5), (83, 150, 120, 255))
        covered = self.expected.copy()
        pixel = covered.getpixel((3, 4))
        covered.putpixel((3, 4), (pixel[0] + 3, pixel[1], pixel[2], 255))
        alpha = self.expected.copy()
        alpha.putpixel((4, 5), (80, 150, 120, 254))
        for actual in [self.background, clipped, shifted, opaque, covered, alpha,
                       self.expected.crop((0, 0, 11, 10))]:
            with self.subTest(size=actual.size):
                with self.assertRaises(ValueError):
                    compare_images(self.expected, actual)


class CommandFailureTests(unittest.TestCase):
    def test_nonzero_exit_and_success_with_diagnostics_fail(self):
        for code in [0, 3]:
            with self.subTest(code=code):
                command = [sys.executable, "-c",
                           f"import sys; sys.stderr.write('probe diagnostic'); sys.exit({code})"]
                with self.assertRaisesRegex(ValueError, f"exited {code}: probe diagnostic"):
                    run(command)


if __name__ == "__main__":
    unittest.main()
