import copy
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from PIL import Image

import check_quickshell_lit_body as lit
from check_quickshell_lit_body import (
    device_rect, discriminate, measure, native_command, native_toggle, over, phase_state, plan_case, with_thumbs)


def pair(color, alpha=255):
    return (*color, alpha)


def scene_request(states, checked_role="color.checked", part=None, direction="ltr"):
    body = {"theme": {"themeSource": "{}", "externalSources": {"a": "b"},
                      "environment": {"layoutDirection": direction, "textScale": 2}},
            "size": {"width": 64.5, "height": 40.5},
            "surface": {"states": {"states": states}, "materialRole": "m", "colorRole": "c"},
            "appearance": {"keyLight": 1}, "foregroundRole": "fg", "minimumContentContrast": 1, "minimumEdgeContrast": 3,
            "adjacentColor": {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1},
            "postTreatmentBackdrop": {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1}}
    request = {"surface": {"body": body, "surroundingColor": {"colorSpace": "srgb", "components": [.9, .9, .9], "alpha": 1}},
               "light": {"ambient": .25, "interaction": None}, "relief": {"commandBody": {}},
               "interactionAppearance": {"profiles": {}}, "commandAppearance": {"profiles": {}},
               "checkedColorRole": checked_role}
    if part:
        request["part"] = part
    return {"request": request}


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


class RequestTests(unittest.TestCase):
    def test_a_command_scene_becomes_a_label_free_body_of_the_scene_size(self):
        scene = scene_request(["pressed"])
        scene["request"]["surface"]["surroundingColor"]["components"] = [.5, .5, .5]
        request = native_command(scene)
        self.assertEqual(scene["request"]["surface"]["body"]["size"], request["content"]["minimumSize"])
        self.assertEqual(request["content"]["minimumSize"], request["content"]["maximumSize"])
        self.assertEqual("​", request["content"]["text"])
        self.assertEqual(1, request["content"]["environment"]["textScale"])
        self.assertEqual([.5, .5, .5], request["body"]["surroundingColor"]["components"])
        self.assertEqual({"ambient": .25, "interaction": None}, request["body"]["light"])
        self.assertIsNotNone(request["activation"]["hold"])
        self.assertFalse(request["hovered"])

    def test_a_toggle_frame_carries_the_track_and_thumb_scenes_with_the_scene_state(self):
        track = scene_request(["pressed", "checked"], "track.checked", "track")
        thumb = scene_request(["pressed", "checked"], "thumb.checked", "thumb")
        thumb["request"]["light"] = {"ambient": None, "interaction": None}
        request = native_toggle(track, thumb)
        self.assertEqual("track.checked", request["track"]["checkedColorRole"])
        self.assertEqual("thumb.checked", request["thumb"]["checkedColorRole"])
        self.assertEqual({"ambient": .25, "interaction": None}, request["track"]["light"])
        self.assertEqual({"ambient": None, "interaction": None}, request["thumb"]["light"])
        self.assertTrue(request["checked"])
        self.assertIsNotNone(request["activation"]["hold"])
        self.assertEqual({"width": 64.5, "height": 40.5}, request["layout"]["trackSize"])
        self.assertEqual({"width": 16, "height": 16}, request["layout"]["thumbSize"])
        self.assertEqual({12.25}, set(request["layout"]["insets"].values()))
        for name in ("track", "thumb"):
            self.assertEqual(["rest"], request[name]["surface"]["states"]["states"])
        self.assertEqual(1, request["content"]["environment"]["textScale"])

    def test_the_toggle_frame_takes_the_layout_direction_of_the_scene(self):
        track = scene_request(["hover"], part="track", direction="rtl")
        request = native_toggle(track, scene_request(["hover"], part="thumb", direction="rtl"))
        self.assertEqual("rtl", request["content"]["environment"]["layoutDirection"])
        self.assertLess(request["labelOrigin"]["x"], 0)
        self.assertFalse(request["checked"])
        self.assertTrue(request["hovered"])

    def test_every_track_scene_gains_a_thumb_scene_without_a_focus_ring(self):
        def track(name, states, checked=False):
            return {"name": name, "component": "toggle", "part": "track", "checked": checked, "states": states}
        manifest = {"scenarios": [
            track("t-track-off-hover", ["hover"]),
            {"name": "t-thumb-off-hover", "component": "toggle", "part": "thumb", "checked": False, "states": ["hover"]},
            track("t-track-off-focused", ["rest", "focused"]),
            {"name": "t-command", "component": "command", "variant": "standard", "states": ["rest"]}]}
        before = copy.deepcopy(manifest)
        derived = with_thumbs(manifest)["scenarios"][len(before["scenarios"]):]
        self.assertEqual(before, manifest)
        self.assertEqual(1, len(derived))
        self.assertEqual(("t-thumb-off-focused", "thumb", ["rest"]),
                         (derived[0]["name"], derived[0]["part"], derived[0]["states"]))


class MeasureTests(unittest.TestCase):
    def capture(self, directory, pixels):
        path = Path(directory) / "frame.png"
        image = Image.new("RGBA", (4, 2), (255, 255, 255, 255))
        for position, color in pixels.items():
            image.putpixel(position, color)
        image.save(path)
        return path

    def task(self, path):
        bounds = [((9, 9, 9, 255), (9, 9, 9, 255))] * 8
        return ("frame", path, (0, 0, 4, 2), [(("scene", None, None, (0, 0)), (0, 0, 4, 2))], (255, 255, 255), 1), bounds

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

    def test_a_translucent_capture_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = self.capture(directory, {(x, y): (9, 9, 9, 255) for x in range(4) for y in range(2)} | {(0, 0): (9, 9, 9, 200)})
            task, bounds = self.task(path)
            with patch.object(lit, "part_bounds", return_value=bounds), self.assertRaisesRegex(ValueError, "not opaque"):
                measure(task)

    def test_a_two_level_shift_is_detected_and_a_looser_tolerance_would_miss_it(self):
        with tempfile.TemporaryDirectory() as directory:
            path = self.capture(directory, {(x, y): (9, 9, 9, 255) for x in range(4) for y in range(2)})
            task, bounds = self.task(path)
            with patch.object(lit, "part_bounds", return_value=bounds):
                self.assertEqual(("frame", 1, 0, 0), discriminate(task))
                with patch.object(lit, "TOLERANCE", 2), self.assertRaisesRegex(ValueError, "was not detected"):
                    discriminate(task)


class PlanTests(unittest.TestCase):
    def report(self, snapshot, scale=1, renderer="software"):
        return {"deviceScale": scale, "renderer": renderer, "snapshot": json.dumps(snapshot)}

    def command_snapshot(self, states=("hover",), width=96):
        return {"label": {"size": {"width": width, "height": 48}},
                "paint": {"paint": {"body": {"states": {"states": list(states)}}}}}

    def toggle_snapshot(self, thumb_x=36.25, thumb_states=("rest",), thumb_size=16):
        return {"layout": {"trackBounds": {"x": 0, "y": 0, "width": 64.5, "height": 40.5},
                           "thumbBounds": {"x": thumb_x, "y": 12.25, "width": thumb_size, "height": thumb_size}},
                "track": {"paint": {"body": {"states": {"states": ["rest"]}}}},
                "thumb": {"paint": {"body": {"states": {"states": list(thumb_states)}}}}}

    def plan(self, directory, kind, report, scale=1, expected=("hover",)):
        for name in ("scene", "thumb"):
            Image.new("RGBA", (round(500 * scale), round(400 * scale))).save(Path(directory) / (name + ".png"))
        scenes = {"scene": scene_request(list(expected)), "thumb": scene_request(list(expected))}
        scenes["thumb"]["request"]["surface"]["body"]["size"] = {"width": 16, "height": 16}
        case = {"name": "scene", "kind": kind, "background": [1, 1, 1],
                "scenes": {"body": "scene"} if kind == "command" else {"track": "scene", "thumb": "thumb"}}
        scenes["scene"]["request"]["surface"]["body"]["size"] = (
            {"width": 96, "height": 48} if kind == "command" else {"width": 64.5, "height": 40.5})
        context = (scenes, {"scene": (None, {}), "thumb": (None, {})}, 8)
        return plan_case(context, Path(directory), case, report, scale)

    def test_a_matching_command_report_plans_the_whole_body(self):
        with tempfile.TemporaryDirectory() as directory:
            name, _, rect, layers, background, scale = self.plan(directory, "command", self.report(self.command_snapshot()))
            self.assertEqual(("scene", (42, 42, 112, 64), (255, 255, 255), 1), (name, rect, background, scale))
            self.assertEqual([(50, 50)], [layer[0][3] for layer in layers])

    def test_a_toggle_report_plans_the_track_and_the_thumb_at_the_native_placement(self):
        with tempfile.TemporaryDirectory() as directory:
            _, _, rect, layers, _, _ = self.plan(directory, "toggle", self.report(self.toggle_snapshot()), expected=("rest",))
            self.assertEqual([(50, 50), (86.25, 62.25)], [layer[0][3] for layer in layers])
            self.assertEqual((42, 42, 81, 57), rect)
            self.assertEqual((78, 54, 33, 33), layers[1][1])

    def test_a_toggle_thumb_in_other_states_than_the_track_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "different states"):
                self.plan(directory, "toggle", self.report(self.toggle_snapshot(thumb_states=("hover",))), expected=("rest",))

    def test_a_toggle_layout_other_than_the_scene_sizes_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "layout differs"):
                self.plan(directory, "toggle", self.report(self.toggle_snapshot(thumb_size=24)), expected=("rest",))

    def test_other_states_than_the_scene_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "native states"):
                self.plan(directory, "command", self.report(self.command_snapshot(("pressed",))))

    def test_another_device_scale_or_renderer_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "software renderer"):
                self.plan(directory, "command", self.report(self.command_snapshot(), scale=2))
            with self.assertRaisesRegex(ValueError, "software renderer"):
                self.plan(directory, "command", self.report(self.command_snapshot(), renderer="unsupported"))

    def test_another_body_size_than_the_scene_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "native body size"):
                self.plan(directory, "command", self.report(self.command_snapshot(width=100)))


if __name__ == "__main__":
    unittest.main()
