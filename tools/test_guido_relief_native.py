import unittest

from PIL import Image
from check_guido_relief_native import capture_matches


class NativeReliefComparisonTests(unittest.TestCase):
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
