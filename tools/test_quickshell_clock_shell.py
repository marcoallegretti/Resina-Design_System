import unittest

from check_quickshell_clock_shell import (COLOR_SCHEME_CASE, PLATFORM_CASES, edge_points, platform_expectations,
                                          verify_capture_size, verify_command_scale, verify_device_scale, verify_platform_preferences,
                                          verify_publication_scales)

AUTHORED = {"reducedMotion": False, "reducedTransparency": False, "highContrast": False}
FULL = {"reducedMotion": True, "highContrast": False, "colorScheme": "light"}


def command_request(scale):
    environment = {"schemaVersion": "0.2.0"}
    if scale is not None:
        environment["scale"] = scale
    return {"content": {"environment": environment}}


class ClockShellScaleTests(unittest.TestCase):
    def test_a_device_scale_must_equal_the_requested_scale(self):
        verify_device_scale(1.25, 1.25, "the device scale")
        verify_device_scale(2, 2.0, "the device scale")
        for observed in (None, True, "1.25", 2):
            with self.subTest(observed=observed), self.assertRaisesRegex(ValueError, "is not device scale 1.25"):
                verify_device_scale(observed, 1.25, "the device scale")

    def test_a_command_and_its_environment_must_use_the_presented_scale(self):
        observed = {"deviceScale": {"showClock": 1.25, "close": None}}
        verify_command_scale(observed, "showClock", command_request(1.25), 1.25)
        cases = (
            ({"deviceScale": {"showClock": 2}}, command_request(1.25), "showClock Command's device scale"),
            ({}, command_request(1.25), "showClock Command's device scale"),
            (observed, command_request(2), "showClock request's environment scale"),
            (observed, command_request(None), "showClock request's environment scale"),
        )
        for current, request, message in cases:
            with self.subTest(message=message, current=current), self.assertRaisesRegex(ValueError, message):
                verify_command_scale(current, "showClock", request, 1.25)

    def test_a_capture_must_cover_a_whole_logical_output(self):
        verify_capture_size((1024, 600), (1024, 600), 1)
        verify_capture_size((1280, 720), (1024, 576), 1.25)
        verify_capture_size((1024, 600), (512, 300), 2)
        verify_capture_size((1920, 1080), (1280, 720), 1.5)
        for capture, output, scale in (((1024, 600), (819, 480), 1.25), ((1024, 600), (682, 400), 1.5),
                                       ((1280, 720), (1024, 576), 2)):
            with self.subTest(output=output, scale=scale), self.assertRaisesRegex(ValueError, "must be whole"):
                verify_capture_size(capture, output, scale)

    def test_edge_points_include_the_last_row_and_column(self):
        points = edge_points(1280, 720)
        self.assertEqual({x for x, _ in points}, {0, 640, 1279})
        self.assertEqual({y for _, y in points}, {0, 360, 719})
        self.assertIn((1279, 719), points)

    def test_publications_from_startup_use_the_window_device_scale(self):
        verify_publication_scales({"bar": [[1.25, 2]], "showClock": [[1.25, 9]]}, 1.25)
        verify_publication_scales({"bar": [[1.25, 1], [2, 1]], "showClock": [[1.25, 3], [2, 4]],
                                   "clock": [[2, 1]], "close": [[1.25, 1]]}, 1.25, 2)
        cases = (
            ({"bar": [[2, 1], [1.25, 1]], "showClock": [[1.25, 1]]}, None, "bar was published at device scales"),
            ({"bar": [[1.25, 1]], "showClock": [[1.25, 1], [2, 1]]}, None, "showClock was published"),
            ({"bar": [[1.25, 1]], "showClock": [[1.25, 1], [2, 1], [1.25, 1]]}, 2, "showClock was published"),
            ({"bar": [[1.25, 1]], "showClock": [[None, 1]]}, None, "has no device scale"),
            ({"bar": [[1.25, 1]]}, None, "no bar or Show clock publication"),
        )
        for runs, final, message in cases:
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                verify_publication_scales(runs, 1.25, final)


class ClockShellPlatformTests(unittest.TestCase):
    def test_platform_values_are_expected_whenever_the_portal_provides_them(self):
        expected, sources, skips = platform_expectations(FULL, AUTHORED, None)
        self.assertEqual(expected, {"reducedMotion": True, "reducedTransparency": False, "highContrast": False})
        self.assertEqual(sources, {"reducedMotion": "platform", "highContrast": "platform"})
        self.assertEqual(skips, [])

    def test_cases_the_portal_cannot_provide_are_recorded_as_skips(self):
        portal = dict(FULL, reducedMotion=None)
        expected, sources, skips = platform_expectations(portal, dict(AUTHORED, reducedMotion=True), None)
        self.assertEqual(expected["reducedMotion"], True)
        self.assertEqual(sources["reducedMotion"], "authored")
        self.assertEqual(skips, [{"case": PLATFORM_CASES["reducedMotion"][1],
                                  "reason": "the portal does not provide org.freedesktop.appearance reduced-motion; "
                                            "the authored reducedMotion true is in effect"}])
        _, sources, skips = platform_expectations({name: None for name in FULL}, AUTHORED, None)
        self.assertEqual(sources, {"reducedMotion": "authored", "highContrast": "authored"})
        self.assertEqual([skip["case"] for skip in skips], [PLATFORM_CASES["reducedMotion"][1],
                                                          PLATFORM_CASES["highContrast"][1], COLOR_SCHEME_CASE])
        self.assertIn("color-scheme; the light theme is in effect", skips[2]["reason"])

    def test_a_fixed_theme_skips_the_colour_scheme_case(self):
        _, _, skips = platform_expectations(FULL, AUTHORED, "dark")
        self.assertEqual(skips, [{"case": COLOR_SCHEME_CASE, "reason": "--theme fixes the dark theme"}])

    def test_the_shell_must_not_fall_back_when_the_platform_provides_a_value(self):
        expected, sources, _ = platform_expectations(FULL, AUTHORED, None)
        observed = {"platform": dict(FULL, ready=True, error=""), "preferenceSources": dict(sources),
                    "preferences": dict(expected)}
        environment = {"accessibilityPreferences": dict(expected)}
        verify_platform_preferences(observed, environment, FULL, expected, sources)
        fallback = dict(observed, preferences=dict(expected, reducedMotion=False),
                        preferenceSources=dict(sources, reducedMotion="authored"))
        cases = (
            (dict(observed, platform=dict(FULL, reducedMotion=None)), environment, "independent portal read"),
            (dict(observed, preferenceSources=dict(sources, highContrast="authored")), environment, "takes preferences"),
            (fallback, environment, "takes preferences"),
            (dict(observed, preferences=dict(expected, reducedMotion=False)), environment, "did not reach"),
            (observed, {"accessibilityPreferences": dict(expected, highContrast=True)}, "did not reach"),
        )
        for current, request, message in cases:
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                verify_platform_preferences(current, request, FULL, expected, sources)


if __name__ == "__main__":
    unittest.main()
