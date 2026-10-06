from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest

from PIL import Image, ImageDraw

from check_quickshell_label_render_runtime import check_image, check_probe_output


def ink_image(x=5):
    image = Image.new("RGBA", (400, 640), "white")
    draw = ImageDraw.Draw(image)
    draw.rectangle((x, 10, x + 5, 20), fill="black")
    return image


class NativeLabelCaptureTests(unittest.TestCase):
    def test_matching_opaque_pixels_and_overhang_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "frame.png"
            ink_image().save(path)
            ink_image().save(path.with_name("frame-reference.png"))
            self.assertEqual(len(check_image(path, 1, True)), 32)

    def test_blank_shifted_translucent_wrong_dimensions_and_missing_overhang_fail(self):
        blank = Image.new("RGBA", (400, 640), "white")
        shifted = ink_image(6)
        translucent = ink_image()
        translucent.putpixel((0, 0), (255, 255, 255, 254))
        wrong_size = Image.new("RGBA", (399, 640), "white")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "frame.png"
            ink_image().save(path.with_name("frame-reference.png"))
            for name, image in (("blank", blank), ("shifted", shifted),
                ("translucent", translucent), ("dimensions", wrong_size), ("overhang", ink_image(25))):
                with self.subTest(name=name):
                    image.save(path)
                    (image if name in ("blank", "overhang") else ink_image()).save(
                        path.with_name("frame-reference.png"))
                    with self.assertRaises(ValueError):
                        check_image(path, 1, True)

    def test_reference_validation_and_channel_tolerance(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "frame.png"
            reference_path = path.with_name("frame-reference.png")
            ink_image().save(path)
            with self.assertRaises(OSError):
                check_image(path, 1)
            for delta in (1, 2):
                reference = ink_image()
                reference.putpixel((5, 10), (delta, delta, delta, 255))
                reference.save(reference_path)
                if delta == 1:
                    check_image(path, 1)
                else:
                    with self.assertRaises(ValueError):
                        check_image(path, 1)
            for reference in (Image.new("RGBA", (399, 640), "white"),
                Image.new("RGBA", (400, 640), (255, 255, 255, 254))):
                reference.save(reference_path)
                with self.assertRaises(ValueError):
                    check_image(path, 1)

    def test_native_success_requires_exact_counts_scale_and_terminal_status(self):
        success = "RESINA_LABEL_RENDER_PASS 25 9 1.25\n"
        check_probe_output(SimpleNamespace(stdout=success, stderr="", returncode=0), 1.25)
        for log, status in (("", 0), (success + success, 0), (success, 1),
            (success + "RESINA_LABEL_RENDER_FAIL failed\n", 0),
            (success.replace("25", "24"), 0), (success.replace("1.25", "1"), 0)):
            with self.subTest(log=log, status=status):
                with self.assertRaises(ValueError):
                    check_probe_output(SimpleNamespace(stdout=log, stderr="", returncode=status), 1.25)


if __name__ == "__main__":
    unittest.main()
