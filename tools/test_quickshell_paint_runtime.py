import unittest

from PIL import Image

from check_quickshell_paint_runtime import compare_images


class PaintCaptureTests(unittest.TestCase):
    def setUp(self):
        self.expected = Image.new("RGBA", (8, 6), (0, 0, 0, 0))
        for x in range(2, 6):
            for y in range(1, 5):
                self.expected.putpixel((x, y), (60, 120, 180, 255))
        self.expected.putpixel((1, 2), (60, 120, 180, 64))

    def test_identical_and_native_premultiplied_rounding_pass(self):
        compare_images(self.expected, self.expected.copy())
        actual = self.expected.copy()
        actual.putpixel((1, 2), (59, 119, 179, 64))
        compare_images(self.expected, actual)

    def test_blank_clipped_shifted_pigment_and_alpha_damage_fail(self):
        damaged = [Image.new("RGBA", self.expected.size), self.expected.copy(), Image.new("RGBA", self.expected.size), self.expected.copy(), self.expected.copy(), self.expected.copy()]
        damaged[1].putpixel((2, 1), (0, 0, 0, 0))
        damaged[2].paste(self.expected, (1, 0))
        damaged[3].putpixel((3, 3), (80, 120, 180, 255))
        damaged[4].putpixel((1, 2), (60, 120, 180, 80))
        damaged[5].putpixel((1, 2), (120, 180, 220, 64))
        for image in damaged:
            with self.subTest(image=image), self.assertRaises(ValueError):
                compare_images(self.expected, image)
        with self.assertRaises(ValueError):
            compare_images(self.expected, Image.new("RGBA", (9, 6)))


if __name__ == "__main__":
    unittest.main()
