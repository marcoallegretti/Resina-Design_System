import contextlib
import io
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image, ImageDraw
from check_quickshell_focus_runtime import check_image, main


def reference_image():
    samples = 8
    alpha = Image.new("L", (160 * samples, 128 * samples))
    draw = ImageDraw.Draw(alpha)
    draw.rounded_rectangle((16 * samples, 16 * samples, 128 * samples - 1, 104 * samples - 1),
                           radius=16 * samples, fill=255)
    draw.rounded_rectangle((24 * samples, 24 * samples, 120 * samples - 1, 96 * samples - 1),
                           radius=8 * samples, fill=0)
    image = Image.new("RGBA", (160, 128), (255, 255, 255, 255))
    image.putalpha(alpha.resize(image.size, Image.Resampling.BOX))
    return image


class QuickshellRuntimeTests(unittest.TestCase):
    def test_independent_supersampled_round_rectangle_ring_passes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "capture.png"
            reference_image().save(path)
            count, area = check_image(path)
            self.assertGreater(count, 17000)
            self.assertGreater(area, 173)
            self.assertLess(area, 175)

    def test_blank_filled_shifted_clipped_wrong_pigment_and_translucent_captures_fail(self):
        good = reference_image()
        blank = Image.new("RGBA", good.size)
        filled = Image.new("RGBA", good.size, (255, 255, 255, 255))
        shifted = Image.new("RGBA", good.size)
        shifted.paste(good, (4, 0))
        clipped = good.copy()
        ImageDraw.Draw(clipped).rectangle((0, 0, 40, 128), fill=(0, 0, 0, 0))
        wrong = good.copy()
        wrong.putpixel((40, 20), (254, 255, 255, 255))
        faded = good.copy()
        faded.putalpha(good.getchannel("A").point(lambda value: value // 2))
        hole = good.copy()
        hole.putpixel((80, 64), (255, 255, 255, 255))
        exterior = good.copy()
        exterior.putpixel((0, 0), (255, 255, 255, 255))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "capture.png"
            for name, image in (("blank", blank), ("filled", filled), ("shifted", shifted),
                                ("clipped", clipped), ("pigment", wrong), ("translucent", faded),
                                ("hole", hole), ("exterior", exterior)):
                with self.subTest(name=name):
                    image.save(path)
                    with self.assertRaises(ValueError):
                        check_image(path)

    def test_boundary_damage_cannot_hide_in_antialiasing_exclusion(self):
        good = reference_image()
        clipped = good.copy()
        ImageDraw.Draw(clipped).rectangle((16, 0, 16, 127), fill=(0, 0, 0, 0))
        lost_area = good.copy()
        ImageDraw.Draw(lost_area).rectangle((0, 16, 159, 17), fill=(0, 0, 0, 0))
        lost_area.putpixel((80, 16), good.getpixel((80, 16)))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "capture.png"
            clipped.save(path)
            with self.assertRaisesRegex(ValueError, "paint bounds"):
                check_image(path)
            lost_area.save(path)
            with self.assertRaisesRegex(ValueError, "coverage area"):
                check_image(path)

    def test_decoding_format_and_dimensions_are_verified(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "capture.png"
            for size, image_format in (((159, 128), "PNG"), ((160, 128), "BMP")):
                with self.subTest(size=size, image_format=image_format):
                    Image.new("RGBA", size).save(path, format=image_format)
                    with self.assertRaises(ValueError):
                        check_image(path)
            path.write_bytes(b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR" + struct.pack(">II", 160, 128))
            with self.assertRaises((OSError, ValueError)):
                check_image(path)

    def test_runtime_pass_log_cannot_certify_blank_or_header_only_capture(self):
        for capture in ("blank", "header-only"):
            with self.subTest(capture=capture), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "capture.png"
                def probe(*args, **kwargs):
                    if capture == "blank":
                        Image.new("RGBA", (160, 128)).save(path)
                    else:
                        path.write_bytes(b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR" + struct.pack(">II", 160, 128))
                    return subprocess.CompletedProcess(args[0], 0, "RESINA_FOCUS_PASS 3197 4", "")
                with patch.object(sys, "argv", ["checker", "--image", str(path), "--", "probe"]), \
                        patch("check_quickshell_focus_runtime.subprocess.run", side_effect=probe), \
                        contextlib.redirect_stderr(io.StringIO()) as error:
                    self.assertEqual(main(), 1)
                    self.assertIn("FAIL Quickshell focus runtime", error.getvalue())

    def test_success_and_probe_failures_keep_exit_status_and_fresh_capture_guard(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "capture.png"
            def probe(*args, **kwargs):
                reference_image().save(path)
                return subprocess.CompletedProcess(args[0], 0, "RESINA_FOCUS_PASS 3197 3", "")
            with patch.object(sys, "argv", ["checker", "--image", str(path), "--", "probe"]), \
                    patch("check_quickshell_focus_runtime.subprocess.run", side_effect=probe), \
                    contextlib.redirect_stdout(io.StringIO()) as output:
                self.assertEqual(main(), 0)
                self.assertIn("pixels", output.getvalue())
            with patch.object(sys, "argv", ["checker", "--image", str(path), "--", "probe"]), \
                    patch("check_quickshell_focus_runtime.subprocess.run") as run, \
                    contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as error:
                    main()
                self.assertEqual(error.exception.code, 2)
                run.assert_not_called()
            path.unlink()
            for code, log in ((1, "RESINA_FOCUS_PASS 3197 3"), (0, "RESINA_FOCUS_FAIL"), (0, "RESINA_FOCUS_PASS 3196 3")):
                with self.subTest(code=code, log=log), \
                        patch.object(sys, "argv", ["checker", "--image", str(path), "--", "probe"]), \
                        patch("check_quickshell_focus_runtime.subprocess.run", return_value=subprocess.CompletedProcess(["probe"], code, log, "")), \
                        contextlib.redirect_stderr(io.StringIO()):
                    self.assertEqual(main(), 1)


if __name__ == "__main__":
    unittest.main()
