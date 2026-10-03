import tempfile
from pathlib import Path
import unittest

from PIL import Image
from check_slint_focus_runtime import check_image


class SlintRuntimeTests(unittest.TestCase):
    def test_invalid_size_format_and_filled_hole_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "capture.png"
            for size, image_format in [((39, 34), "PNG"), ((40, 34), "BMP")]:
                Image.new("RGBA", size, (32, 32, 32, 255)).save(path, format=image_format)
                with self.assertRaisesRegex(ValueError, "format or dimensions"):
                    check_image(path, 1)
            Image.new("RGBA", (40, 34), (255, 255, 255, 255)).save(path)
            with self.assertRaisesRegex(ValueError, "distance oracle"):
                check_image(path, 1)


if __name__ == "__main__":
    unittest.main()
