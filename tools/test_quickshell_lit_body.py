import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from PIL import Image

import check_quickshell_lit_body as lit
from check_quickshell_lit_body import device_rect, measure, over, phase_state, plan_case


def pair(color, alpha=255):
    return (*color, alpha)


class BoundsTests(unittest.TestCase):
    def test_opaque_bounds_replace_the_background(self):
        self.assertEqual(((10, 20, 30), (10, 20, 30)), over((0, 0, 0), (0, 0, 0), pair((10, 20, 30)), pair((10, 20, 30))))

    def test_half_coverage_blends_with_the_background(self):
        lower, upper = over((100, 100, 100), (100, 100, 100), pair((200, 0, 50), 51), pair((200, 0, 50), 51))
        self.assertEqual(lower, upper)
        self.assertEqual((120, 80, 90), tuple(round(channel) for channel in lower))

    def test_ambiguous_pixels_keep_both_outcomes(self):
        lower, upper = over((0, 0, 0), (0, 0, 0), pair((100, 100, 100), 0), pair((200, 200, 200), 255))
        self.assertEqual((0, 0, 0), lower)
        self.assertEqual((200, 200, 200), upper)

    def test_device_rectangles_cover_fractional_edges(self):
        self.assertEqual((61, 61, 8, 4), device_rect(48.8, 48.8, 6.01, 3.01, 1.25))
        self.assertEqual((42, 42, 112, 64), device_rect(42, 42, 112, 64, 1))


class PhaseTests(unittest.TestCase):
    def test_states_map_to_activation_hover_and_hold(self):
        cases = {
            ("rest",): (True, False, False, False),
            ("hover",): (True, False, False, True),
            ("pressed",): (True, False, True, False),
            ("disabled",): (False, False, False, False),
            ("rest", "focused"): (True, True, False, False),
        }
        for states, expected in cases.items():
            with self.subTest(states=states):
                activation, hovered = phase_state(list(states))
                self.assertEqual(expected, (activation["enabled"], activation["focused"],
                                            activation["hold"] is not None, hovered))


class MeasureTests(unittest.TestCase):
    def capture(self, directory, pixels):
        path = Path(directory) / "frame.png"
        image = Image.new("RGBA", (4, 2), (255, 255, 255, 255))
        for position, color in pixels.items():
            image.putpixel(position, color)
        image.save(path)
        return path

    def task(self, path, skipped=()):
        bounds = [((9, 9, 9, 255), (9, 9, 9, 255))] * 8
        return ("frame", path, (0, 0, 4, 2), [(("scene", None, None, (0, 0)), (0, 0, 4, 2))], (255, 255, 255), list(skipped), 1), bounds

    def test_matching_pixels_pass_and_report_the_worst_distance(self):
        with tempfile.TemporaryDirectory() as directory:
            path = self.capture(directory, {(x, y): (9, 9, 10, 255) for x in range(4) for y in range(2)})
            task, bounds = self.task(path)
            with patch.object(lit, "part_bounds", return_value=bounds):
                self.assertEqual(("frame", 1, 8, 1), measure(task))

    def test_a_pixel_outside_the_bounds_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            path = self.capture(directory, {(x, y): (9, 9, 9, 255) for x in range(4) for y in range(2)} | {(2, 1): (9, 9, 11, 255)})
            task, bounds = self.task(path)
            with patch.object(lit, "part_bounds", return_value=bounds), self.assertRaisesRegex(ValueError, r"pixel \(2, 1\)"):
                measure(task)

    def test_skipped_pixels_are_not_compared(self):
        with tempfile.TemporaryDirectory() as directory:
            path = self.capture(directory, {(x, y): (9, 9, 9, 255) for x in range(4) for y in range(2)} | {(2, 1): (200, 0, 0, 255)})
            task, bounds = self.task(path, skipped=[(2, 1, 3, 2)])
            with patch.object(lit, "part_bounds", return_value=bounds):
                self.assertEqual(0, measure(task)[3])

    def test_a_translucent_capture_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = self.capture(directory, {(x, y): (9, 9, 9, 255) for x in range(4) for y in range(2)} | {(0, 0): (9, 9, 9, 200)})
            task, bounds = self.task(path)
            with patch.object(lit, "part_bounds", return_value=bounds), self.assertRaisesRegex(ValueError, "not opaque"):
                measure(task)


class PlanTests(unittest.TestCase):
    def scene(self, states):
        return {"request": {"surface": {"body": {"size": {"width": 96, "height": 48},
                                                 "surface": {"states": {"states": states}}}}}}

    def report(self, scale=1, renderer="software", states=("hover",)):
        snapshot = {"label": {"size": {"width": 96, "height": 48},
                              "labelBounds": {"x": 30, "y": 14, "width": 36, "height": 20}},
                    "paint": {"paint": {"body": {"states": {"states": list(states)}}}}}
        return {"deviceScale": scale, "renderer": renderer, "snapshot": json.dumps(snapshot)}

    def plan(self, directory, report, expected=("hover",), scale=1):
        name = "scene"
        Image.new("RGBA", (round(500 * scale), round(400 * scale))).save(Path(directory) / (name + ".png"))
        case = {"name": name, "kind": "command", "background": [1, 1, 1], "scenes": {"body": name}}
        context = ({name: self.scene(list(expected))}, {name: (None, {})}, 8)
        return plan_case(context, Path(directory), case, report, scale)

    def test_a_matching_report_plans_the_body_with_its_label_excluded(self):
        with tempfile.TemporaryDirectory() as directory:
            name, _, rect, layers, background, skipped, scale = self.plan(directory, self.report())
            self.assertEqual(("scene", (42, 42, 112, 64), (255, 255, 255), 1), (name, rect, background, scale))
            self.assertEqual([(79, 63, 117, 85)], skipped)
            self.assertEqual(1, len(layers))

    def test_other_states_than_the_scene_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "native states"):
                self.plan(directory, self.report(states=("pressed",)))

    def test_another_device_scale_or_renderer_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "software renderer"):
                self.plan(directory, self.report(scale=2))
            with self.assertRaisesRegex(ValueError, "software renderer"):
                self.plan(directory, self.report(renderer="unsupported"))

    def test_another_body_size_than_the_scene_is_rejected(self):
        report = self.report()
        snapshot = json.loads(report["snapshot"])
        snapshot["label"]["size"] = {"width": 100, "height": 48}
        report["snapshot"] = json.dumps(snapshot)
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "native body size"):
                self.plan(directory, report)


if __name__ == "__main__":
    unittest.main()
