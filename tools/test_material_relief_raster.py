import io
from types import SimpleNamespace
import unittest

from PIL import Image
from PIL.PngImagePlugin import PngInfo

from check_material_relief_raster_backend import check_png


def result(pixels, srgb=True):
    image = Image.new("RGBA", (len(pixels), 1))
    image.putdata(pixels)
    metadata = PngInfo()
    if srgb:
        metadata.add(b"sRGB", b"\x00")
    output = io.BytesIO()
    image.save(output, format="PNG", pnginfo=metadata)
    return SimpleNamespace(returncode=0, stderr=b"", stdout=output.getvalue())


class MaterialReliefRasterProofTests(unittest.TestCase):
    def test_complete_rgba_metadata_and_pixels_are_required(self):
        expected = [(0, 0, 0, 0), (128, 102, 77, 255)]
        actual = result(expected)
        self.assertEqual(check_png(actual, 2, 1, expected), 0)
        with self.assertRaisesRegex(ValueError, "single PNG"):
            check_png(SimpleNamespace(returncode=0, stderr=b"", stdout=actual.stdout + b"junk"), 2, 1, expected)
        with self.assertRaisesRegex(ValueError, "complete"):
            check_png(actual, 2, 1, expected[:1])
        with self.assertRaisesRegex(ValueError, "perceptual"):
            check_png(result(expected, srgb=False), 2, 1, expected)
        changed = bytearray(actual.stdout)
        changed[24] = 16
        with self.assertRaisesRegex(ValueError, "8-bit RGBA"):
            check_png(SimpleNamespace(returncode=0, stderr=b"", stdout=bytes(changed)), 2, 1, expected)

    def test_pixel_sensitivity_keeps_exact_coverage_and_clear_pixels(self):
        expected = [(0, 0, 0, 0), (128, 102, 77, 255)]
        for actual, message in (([(1, 0, 0, 0), expected[1]], "clear"),
                                ([expected[0], (128, 102, 77, 254)], "coverage"),
                                ([expected[0], (130, 102, 77, 255)], "color")):
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                check_png(result(actual), 2, 1, expected)
        self.assertEqual(check_png(result([expected[0], (129, 102, 77, 255)]), 2, 1, expected), 1)


if __name__ == "__main__":
    unittest.main()
