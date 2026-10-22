import io
from types import SimpleNamespace
import unittest

from PIL import Image
from PIL.PngImagePlugin import PngInfo

from check_command_paint_backend import control_body_source
from check_lit_body_raster_backend import SCENES, check_bounded_png, focus_ring, response_for, scene_request
from check_schemas import ROOT, load_json
from lit_body_arithmetic import pixels


def result(pixels):
    image = Image.new("RGBA", (len(pixels), 1))
    image.putdata(pixels)
    metadata = PngInfo()
    metadata.add(b"sRGB", b"\x00")
    output = io.BytesIO()
    image.save(output, format="PNG", pnginfo=metadata)
    return SimpleNamespace(returncode=0, stderr=b"", stdout=output.getvalue())


class LitBodyRasterProofTests(unittest.TestCase):
    clear = ((0, 0, 0, 0), (0, 0, 0, 0))
    body = ((128, 102, 77, 255), (128, 102, 77, 255))
    edge = ((40, 40, 40, 85), (40, 40, 40, 170))

    def test_bounds_accept_one_step_and_both_bracketed_coverages(self):
        bounds = [self.clear, self.body, self.edge]
        self.assertEqual(check_bounded_png(result([(0, 0, 0, 0), (129, 101, 77, 255), (40, 40, 40, 85)]),
                                           3, 1, bounds), 1)
        self.assertEqual(check_bounded_png(result([(0, 0, 0, 0), (128, 102, 77, 255), (40, 40, 40, 170)]),
                                           3, 1, bounds), 0)
        with self.assertRaisesRegex(ValueError, "complete"):
            check_bounded_png(result([(0, 0, 0, 0)] * 3), 3, 1, bounds[:2])

    def test_pixel_sensitivity_rejects_color_coverage_and_unclear_transparency(self):
        bounds = [self.clear, self.body, self.edge]
        good = [(0, 0, 0, 0), (128, 102, 77, 255), (40, 40, 40, 128)]
        for index, pixel, message in ((1, (130, 102, 77, 255), "color"),
                                      (1, (128, 102, 75, 255), "color"),
                                      (1, (128, 102, 77, 254), "coverage"),
                                      (2, (40, 40, 40, 84), "coverage"),
                                      (2, (40, 40, 40, 171), "coverage"),
                                      (0, (1, 0, 0, 0), "clear"),
                                      (0, (0, 0, 0, 1), "clear")):
            actual = list(good)
            actual[index] = pixel
            with self.subTest(pixel=pixel), self.assertRaisesRegex(ValueError, message):
                check_bounded_png(result(actual), 3, 1, bounds)
        bracketed = [self.clear, self.body, ((40, 40, 40, 0), (40, 40, 40, 170))]
        with self.assertRaisesRegex(ValueError, "clear"):
            check_bounded_png(result([(0, 0, 0, 0), (128, 102, 77, 255), (40, 40, 40, 0)]), 3, 1, bracketed)

    def test_focus_ring_is_the_gap_and_stroke_dilation_outside_the_body(self):
        base = load_json(ROOT / "conformance/ir/command-paint-request.json")
        _, family, changes = next(scene for scene in SCENES if "ring around a square" in scene[0])
        request = scene_request(base, family, changes)
        ring = focus_ring(request)
        self.assertEqual(ring[1:], (2, 2))
        source = control_body_source(request, response_for(request))
        middle = source["size"]["height"] / 2
        row = pixels(source, 7, 1, (-7, middle - 0.5), 1, 1, ring=ring)
        expected = tuple(round(255 * c) for c in ring[0]) + (255,)
        # Pixel centers lie 6.5 to 0.5 px left of the front edge; the body never sweeps sideways here.
        for index, distance in enumerate((6.5, 5.5, 4.5, 3.5, 2.5, 1.5, 0.5)):
            lower, upper = row[index]
            with self.subTest(distance=distance):
                self.assertEqual(lower, upper)
                self.assertEqual(lower, expected if 2 < distance <= 4 else (0, 0, 0, 0))
        without = pixels(source, 7, 1, (-7, middle - 0.5), 1, 1)
        self.assertTrue(all(bounds == ((0, 0, 0, 0), (0, 0, 0, 0)) for bounds in without))


if __name__ == "__main__":
    unittest.main()
