import unittest

from PIL import Image

from composited_capture import (Layer, TextRegion, Window, compare, compose, compositor_length, compositor_origin,
                                placed_viewport, text_rectangle)

BACKDROP = (236, 238, 242)
BODY = (250, 251, 253)
EDGE = (180, 186, 196)
TEXT = (20, 24, 32)


def byte_mul(value, alpha):
    product = value * alpha
    return (product + (product >> 8) + 0x80) >> 8


def premultiplying_renderer(size, backdrop, layers):
    """Composite as a premultiplying 8-bit renderer does, with its own rounding at each step."""
    image = Image.new("RGB", size, backdrop)
    for layer in layers:
        for y in range(layer.image.height):
            for x in range(layer.image.width):
                *color, alpha = layer.image.getpixel((x, y))
                point = (layer.x + x, layer.y + y)
                below = image.getpixel(point)
                image.putpixel(point, tuple(byte_mul(c, alpha) + byte_mul(d, 255 - alpha)
                                            for c, d in zip(color, below)))
    return image


def rounded_body(width=24, height=16, radius=6, fill=BODY, edge=EDGE):
    """An opaque body with a one-pixel edge band and antialiased corner arcs."""
    image = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    for y in range(height):
        for x in range(width):
            cx = min(max(x + .5, radius), width - radius)
            cy = min(max(y + .5, radius), height - radius)
            distance = ((x + .5 - cx) ** 2 + (y + .5 - cy) ** 2) ** .5
            coverage = max(0, min(1, radius + .5 - distance))
            if coverage == 0:
                continue
            color = edge if distance > radius - 1.5 or x in (0, width - 1) or y in (0, height - 1) else fill
            image.putpixel((x, y), (*color, round(coverage * 255)))
    return image


def arc_pixel(image):
    return next((x, y) for y in range(image.height) for x in range(image.width) if 0 < image.getpixel((x, y))[3] < 255)


def edge_pixel(image):
    return next((x, image.height // 2) for x in range(image.width) if image.getpixel((x, image.height // 2))[3] == 255)


def window(layers, texts=(), size=(48, 40), name="bar"):
    return Window(name, {"x": 0, "y": 0, "width": size[0], "height": size[1]}, 1, BACKDROP, layers, texts)


def changed(image, point, delta):
    image = image.copy()
    image.putpixel(point, tuple(min(255, max(0, channel + delta)) for channel in image.getpixel(point)))
    return image


class CompositionTests(unittest.TestCase):
    def setUp(self):
        self.size = (48, 40)
        self.layers = [Layer("surface", 4, 6, rounded_body(36, 28, 9)),
                       Layer("command", 10, 12, rounded_body(20, 14, 5, fill=(214, 220, 230)))]
        self.capture = premultiplying_renderer(self.size, BACKDROP, self.layers)

    def compare(self, capture, layers=None):
        return compare(capture, BACKDROP, [window(self.layers if layers is None else layers)])

    def test_a_premultiplying_renderer_meets_the_stated_bounds(self):
        summary, _ = self.compare(self.capture)
        self.assertEqual(summary["pixels"], 48 * 40)
        self.assertGreater(summary["partiallyCovered"], 0)
        self.assertLessEqual(summary["maxDifference"], 1)
        self.assertEqual((summary["resampled"], summary["resampledPixels"]), ([], 0))
        self.assertEqual(summary["windows"]["bar"]["footprints"],
                         {"surface": [4, 6, 40, 34], "command": [10, 12, 30, 26]})

    def test_a_one_step_error_on_an_arc_pixel_beyond_its_bound_fails(self):
        expected, tolerance = compose(self.size, BACKDROP, self.layers)
        x, y = arc_pixel(self.layers[1].image)
        point = (self.layers[1].x + x, self.layers[1].y + y)
        bound = tolerance.getpixel(point)
        self.assertEqual(bound, 1)
        capture = self.capture.copy()
        capture.putpixel(point, tuple(channel + bound + 1 for channel in expected.getpixel(point)))
        with self.assertRaisesRegex(ValueError, rf"\({point[0]}, {point[1]}\) in the command footprint"):
            self.compare(capture)

    def test_a_one_step_error_on_an_opaque_edge_band_pixel_fails(self):
        x, y = edge_pixel(self.layers[0].image)
        point = (self.layers[0].x + x, self.layers[0].y + y)
        with self.assertRaisesRegex(ValueError, rf"\({point[0]}, {point[1]}\) in the surface footprint"):
            self.compare(changed(self.capture, point, 1))

    def test_a_changed_edge_colour_fails(self):
        layers = [self.layers[0], Layer("command", 10, 12, rounded_body(20, 14, 5, fill=(214, 220, 230),
                                                                        edge=(182, 188, 198)))]
        with self.assertRaisesRegex(ValueError, "in the command footprint"):
            self.compare(premultiplying_renderer(self.size, BACKDROP, layers))

    def test_a_layer_offset_by_one_device_pixel_fails(self):
        for dx, dy in ((1, 0), (0, 1), (-1, 0)):
            moved = [self.layers[0], Layer("command", 10 + dx, 12 + dy, self.layers[1].image)]
            with self.subTest(offset=(dx, dy)), self.assertRaisesRegex(ValueError, "differs from the reference"):
                self.compare(premultiplying_renderer(self.size, BACKDROP, moved))

    def test_a_changed_corner_radius_fails(self):
        layers = [self.layers[0], Layer("command", 10, 12, rounded_body(20, 14, 4, fill=(214, 220, 230)))]
        with self.assertRaisesRegex(ValueError, "in the command footprint"):
            self.compare(premultiplying_renderer(self.size, BACKDROP, layers))

    def test_the_window_background_and_the_backdrop_must_match_exactly(self):
        with self.assertRaisesRegex(ValueError, r"\(0, 0\) in the bar window background"):
            self.compare(changed(self.capture, (0, 0), 1))
        narrow = window([], size=(40, 40))
        with self.assertRaisesRegex(ValueError, r"\(45, 3\) in the backdrop"):
            compare(changed(Image.new("RGB", self.size, BACKDROP), (45, 3), 1), BACKDROP, [narrow])

    def test_each_further_partial_layer_allows_one_more_step(self):
        half = Image.new("RGBA", (2, 2), (*EDGE, 128))
        layers = [Layer("lower", 1, 1, half), Layer("upper", 1, 1, half)]
        _, tolerance = compose((4, 4), BACKDROP, layers)
        self.assertEqual(tolerance.getpixel((1, 1)), 2)
        self.assertEqual(tolerance.getpixel((0, 0)), 0)
        opaque = [Layer("lower", 1, 1, half), Layer("upper", 1, 1, Image.new("RGBA", (1, 1), (*EDGE, 255)))]
        _, tolerance = compose((4, 4), BACKDROP, opaque)
        self.assertEqual((tolerance.getpixel((1, 1)), tolerance.getpixel((2, 1))), (0, 1))

    def test_a_layer_outside_its_window_or_a_window_outside_the_capture_fails(self):
        with self.assertRaisesRegex(ValueError, "leaves the 48x40 buffer"):
            self.compare(self.capture, [Layer("command", 40, 30, self.layers[1].image)])
        with self.assertRaisesRegex(ValueError, "leaves the 48x40 capture"):
            compare(self.capture, BACKDROP, [window([], size=(48, 41))])


class ResampledWindowTests(unittest.TestCase):
    """A 202 px high window at y = 99 and scale 1.25: a 253 px buffer in a 252 px destination box."""

    def setUp(self):
        self.size = (40, 400)
        self.layers = [Layer("surface", 4, 10, rounded_body(30, 230, 9))]
        self.window = Window("clock", {"x": 0, "y": 99, "width": 32, "height": 202}, 1.25, BACKDROP, self.layers)
        self.capture = self.resampled(self.layers)

    def resampled(self, layers):
        buffer = premultiplying_renderer(self.window.buffer, BACKDROP, layers)
        capture = Image.new("RGB", self.size, BACKDROP)
        capture.paste(buffer.resize(self.window.destination, Image.Resampling.BILINEAR), self.window.origin)
        return capture

    def test_the_destination_box_and_buffer_sizes_follow_wlroots_and_qt(self):
        self.assertEqual((self.window.origin, self.window.destination, self.window.buffer),
                         ((0, 124), (40, 252), (40, 253)))
        self.assertTrue(self.window.resampled)
        aligned = Window("clock", {"x": 0, "y": 99, "width": 32, "height": 203}, 1.25, BACKDROP)
        self.assertEqual((aligned.destination, aligned.buffer, aligned.resampled), ((40, 254), (40, 254), False))

    def test_a_bilinear_resampling_lies_within_the_bracketing_bounds(self):
        summary, _ = compare(self.capture, BACKDROP, [self.window])
        self.assertEqual(summary["resampled"], ["clock"])
        self.assertEqual(summary["resampledPixels"], 40 * 252)

    def test_a_colour_error_in_a_flat_region_still_fails(self):
        with self.assertRaisesRegex(ValueError, r"\(20, 250\) in the resampled clock window"):
            compare(changed(self.capture, (20, 250), 1), BACKDROP, [self.window])

    def test_a_layer_one_buffer_pixel_away_fails_on_either_axis(self):
        for dx, dy in ((1, 0), (0, 1), (0, -1)):
            with self.subTest(offset=(dx, dy)), self.assertRaisesRegex(ValueError, "in the resampled clock window"):
                compare(self.resampled([Layer("surface", 4 + dx, 10 + dy, self.layers[0].image)]), BACKDROP,
                        [self.window])


class TextRegionTests(unittest.TestCase):
    def setUp(self):
        self.layers = [Layer("command", 2, 2, Image.new("RGBA", (20, 12), (*BODY, 255)))]
        self.capture = premultiplying_renderer((24, 16), BACKDROP, self.layers)
        self.region = TextRegion("command label", (6, 5, 18, 11), TEXT)
        for x, y, coverage in ((7, 6, 1), (8, 6, .5), (12, 9, .25), (17, 10, 1)):
            self.capture.putpixel((x, y), tuple(round(t * coverage + b * (1 - coverage)) for t, b in zip(TEXT, BODY)))

    def compare(self, capture, region=None):
        return compare(capture, BACKDROP, [window(self.layers, [region or self.region], size=(24, 16))])

    def test_text_inside_its_published_bounds_is_bounded_not_compared(self):
        summary, _ = self.compare(self.capture)
        self.assertEqual(summary["masked"], 12 * 6)
        self.assertEqual(summary["pixels"], 24 * 16 - 12 * 6)
        self.assertEqual(summary["windows"]["bar"]["texts"], {"command label": [6, 5, 18, 11]})

    def test_text_one_device_pixel_outside_its_bounds_fails(self):
        for point in ((5, 7), (18, 7), (10, 4), (10, 11)):
            capture = self.capture.copy()
            capture.putpixel(point, TEXT)
            with self.subTest(point=point), self.assertRaisesRegex(
                    ValueError, rf"\({point[0]}, {point[1]}\) in the command footprint"):
                self.compare(capture)

    def test_a_pixel_outside_the_text_and_background_range_fails(self):
        capture = self.capture.copy()
        capture.putpixel((10, 7), (255, 0, 0))
        with self.assertRaisesRegex(ValueError, r"\(10, 7\) in the command label text region"):
            self.compare(capture)

    def test_a_background_beyond_the_body_under_text_fails(self):
        capture = self.capture.copy()
        capture.putpixel((14, 7), tuple(min(255, channel + 2) for channel in BODY))
        with self.assertRaisesRegex(ValueError, "command label text region"):
            self.compare(capture)

    def test_an_empty_or_escaping_text_region_fails(self):
        for rectangle in ((6, 5, 6, 11), (20, 5, 26, 11)):
            with self.subTest(rectangle=rectangle), self.assertRaisesRegex(ValueError, "empty or leaves"):
                self.compare(self.capture, TextRegion("label", rectangle, TEXT))


class PlacementTests(unittest.TestCase):
    def test_paint_bounds_round_outward_on_the_device_grid_after_placement(self):
        bounds = {"x": -2, "y": -2, "width": 100.5, "height": 52}
        self.assertEqual(placed_viewport(bounds, (16.3, 10), 1.25), ((17, 10, 127, 65), (13.6 - 16.3, -2.0)))
        self.assertEqual(placed_viewport(bounds, (16, 10), 2), ((28, 16, 201, 104), (-2.0, -2.0)))

    def test_text_bounds_cover_every_device_pixel_they_touch(self):
        self.assertEqual(text_rectangle({"x": 20.1, "y": 30.5, "width": 40, "height": 20}, 1.25), (25, 38, 76, 64))

    def test_window_edges_round_half_away_from_zero_like_the_wlroots_scene(self):
        self.assertEqual(compositor_origin(96, 1.25, "the clock window's logical y"), 120)
        self.assertEqual(compositor_origin(99, 1.25, "the clock window's logical y"), 124)
        self.assertEqual(compositor_origin(97, 1.25, "the clock window's logical y"), 121)
        self.assertEqual(compositor_origin(97, 1.5, "the clock window's logical y"), 146)
        self.assertEqual(compositor_origin(97.0, 2, "the clock window's logical y"), 194)
        self.assertEqual(compositor_length(202, 99, 1.25, "the clock window's logical height"), 252)
        for value in (97.5, None, "97"):
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, "not a whole logical pixel"):
                compositor_origin(value, 1.25, "the clock window's logical y")


if __name__ == "__main__":
    unittest.main()
