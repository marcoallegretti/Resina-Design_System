import unittest
from unittest import mock

import check_quickshell_clock_shell
from check_quickshell_clock_shell import (COLOR_SCHEME_CASE, PLATFORM_CASES, DesktopBackend, PortalTestBackend, Shell,
                                          closed, command_paint_request, label_foreground, opened, paint_bounds,
                                          platform_expectations, require_keys,
                                          tree_mismatch, verify_capture_size, verify_command_scale,
                                          verify_device_scale, verify_platform_preferences, verify_publication_scales)

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


class CommandPaintRequestTests(unittest.TestCase):
    def setUp(self):
        from check_qt_command_snapshot_runtime import request_for
        self.request = request_for("light", "Show clock")
        self.snapshot = {"label": {"size": {"width": 120, "height": 48}}}

    def test_the_portable_request_takes_the_published_size_and_states(self):
        portable = command_paint_request(self.request, self.snapshot, ["rest", "focused"])
        body = portable["surface"]["body"]
        self.assertEqual(portable["commandAppearance"], self.request["body"]["commandAppearance"])
        self.assertEqual(portable["surface"]["surroundingColor"], self.request["body"]["surroundingColor"])
        self.assertEqual(body["size"], {"width": 120, "height": 48})
        self.assertEqual(body["surface"]["states"]["states"], ["rest", "focused"])
        self.assertEqual(body["theme"]["environment"], self.request["content"]["environment"])
        self.assertNotIn("commandAppearance", body)
        self.assertNotIn("surroundingColor", body)

    def test_members_beside_the_surface_follow_the_command_paint_request_schema(self):
        self.request["body"]["light"] = {"ambient": None, "interaction": None}
        properties = {"schemaVersion": {"const": "0.2.0"}, "surface": {}, "commandAppearance": {}, "light": {}}
        versions = {"command-paint-request": properties}
        original = check_quickshell_clock_shell.schema_properties
        with (mock.patch.object(check_quickshell_clock_shell, "schema_properties",
                                side_effect=lambda name: versions.get(name) or original(name)),
              mock.patch.object(check_quickshell_clock_shell, "validator_for") as validator):
            portable = command_paint_request(self.request, self.snapshot, ["rest"])
        validator.return_value.validate.assert_called_once_with(portable)
        self.assertEqual(portable["schemaVersion"], "0.2.0")
        self.assertEqual(portable["light"], {"ambient": None, "interaction": None})
        self.assertNotIn("light", portable["surface"]["body"])

    def test_a_request_the_schema_rejects_fails(self):
        self.request["body"]["unknown"] = True
        with self.assertRaisesRegex(Exception, "unknown"):
            command_paint_request(self.request, self.snapshot, ["rest"])

    def test_the_label_colour_is_the_foreground_of_an_opaque_or_lit_body(self):
        color = {"colorSpace": "srgb", "components": [0.1, 0.2, 0.3], "alpha": 1}
        self.assertEqual(label_foreground({"foreground": color}), color)
        self.assertEqual(label_foreground({"face": {"foreground": color}}), color)

    def test_paint_bounds_include_the_focus_ring(self):
        body = {"geometry": {"silhouette": {"bounds": {"x": 0, "y": 0, "width": 96, "height": 48}}}}
        self.assertEqual(paint_bounds({"body": body}), {"x": 0, "y": 0, "width": 96, "height": 48})
        focus = {"geometry": {"outer": {"contour": {"bounds": {"x": 0, "y": 0, "width": 104, "height": 56}},
                                        "offset": {"x": -4, "y": -4}}}}
        self.assertEqual(paint_bounds({"body": body, "focus": focus}), {"x": -4, "y": -4, "width": 104, "height": 56})


def control(enabled, focused):
    return {"state": {"enabled": enabled, "focused": focused, "hold": None}, "hovered": False}


def open_shell(**changes):
    observed = {"generation": "b", "open": True, "clock": {"body": {}}, "showClock": control(False, False),
                "close": control(True, True), "focus": {"showClock": False, "close": True},
                "targets": {"showClock": None, "close": {"x": 60, "y": 200}},
                "deviceScale": {"showClock": 2, "close": 2},
                "errors": {"backdrop": "", "bar": "", "clock": "", "showClock": "", "close": ""}}
    observed.update(changes)
    return observed


class Running:
    def poll(self):
        return None


class ClockShellStateTests(unittest.TestCase):
    def test_an_open_surface_needs_every_opened_clause(self):
        self.assertTrue(opened(open_shell()))
        self.assertFalse(closed(open_shell()))
        for changes in ({"open": False}, {"clock": None}, {"close": None}, {"showClock": None},
                        {"focus": {"showClock": False, "close": False}}, {"showClock": control(True, False)}):
            with self.subTest(changes=changes):
                self.assertFalse(opened(open_shell(**changes)))

    def test_an_unreached_state_names_the_unmet_clauses_and_the_controls(self):
        shell = Shell.__new__(Shell)
        shell.process = Running()
        shell.observe = lambda: open_shell(focus={"showClock": False, "close": False},
                                           close=control(True, False))
        with self.assertRaises(ValueError) as raised:
            shell.until(opened, "soft reload keeps the clock surface open", timeout=.05)
        message = str(raised.exception)
        self.assertIn("soft reload keeps the clock surface open was not reached:", message)
        self.assertIn('"opened": ["Close has active focus"]', message)
        self.assertIn('"activeFocus": {"showClock": false, "close": false}', message)
        self.assertIn('"close": {"enabled": true, "focused": false, "hold": null, "hovered": false, '
                      '"target": {"x": 60, "y": 200}, "deviceScale": 2}', message)
        self.assertIn('"generation": "b"', message)
        self.assertIn('"errors": {}', message)

    def test_an_unobserved_shell_is_reported(self):
        shell = Shell.__new__(Shell)
        shell.process = Running()
        with self.assertRaisesRegex(ValueError, "Show clock ready was not reached: the shell was never observed"):
            shell.until(closed, "Show clock ready", timeout=0)


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

    def test_a_required_key_the_portal_does_not_provide_fails_instead_of_skipping(self):
        require_keys(FULL, ["reduced-motion", "contrast", "color-scheme"])
        require_keys(dict(FULL, reducedMotion=None), ["contrast", "color-scheme"])
        with self.assertRaisesRegex(ValueError, "does not provide required keys: reduced-motion$"):
            require_keys(dict(FULL, reducedMotion=None), ["reduced-motion", "contrast"])
        with self.assertRaisesRegex(ValueError, "required keys: contrast, color-scheme$"):
            require_keys({name: None for name in FULL}, ["contrast", "color-scheme"])

    def test_the_desktop_backend_changes_and_resets_the_gnome_settings(self):
        with mock.patch.object(check_quickshell_clock_shell, "gsettings") as gsettings:
            settings = DesktopBackend({"GSETTINGS_BACKEND": "dconf"})
            for key in ("reduced-motion", "contrast", "color-scheme"):
                settings.change(key)
                settings.reset(key)
        environment = {"GSETTINGS_BACKEND": "dconf"}
        self.assertEqual(gsettings.call_args_list, [
            mock.call(environment, "set", "org.gnome.desktop.a11y.interface", "reduced-motion", "reduce"),
            mock.call(environment, "reset", "org.gnome.desktop.a11y.interface", "reduced-motion"),
            mock.call(environment, "set", "org.gnome.desktop.a11y.interface", "high-contrast", "true"),
            mock.call(environment, "reset", "org.gnome.desktop.a11y.interface", "high-contrast"),
            mock.call(environment, "set", "org.gnome.desktop.interface", "color-scheme", "prefer-dark"),
            mock.call(environment, "reset", "org.gnome.desktop.interface", "color-scheme")])

    def test_the_test_backend_sets_the_preferred_value_and_resets_to_no_preference(self):
        backend = check_quickshell_clock_shell.portal_settings_backend
        with mock.patch.object(backend, "set_value") as set_value:
            settings = PortalTestBackend({"DBUS_SESSION_BUS_ADDRESS": "unix:path=/bus"})
            for key in ("reduced-motion", "contrast", "color-scheme"):
                settings.change(key)
                settings.reset(key)
        environment = {"DBUS_SESSION_BUS_ADDRESS": "unix:path=/bus"}
        self.assertEqual(set_value.call_args_list, [
            mock.call(environment, key, value) for key in ("reduced-motion", "contrast", "color-scheme")
            for value in (1, 0)])


DATE = "Monday, 19 October 2026"


def node(role, name="", actions=()):
    return {"role": role, "name": name, "actions": list(actions)}


def closed_tree():
    return [node("frame"), node("frame"), node("push button", "Show clock", ["Press", "SetFocus"])]


def open_tree():
    return [node("frame"), node("frame"), node("frame"), node("push button", "Show clock", ["Press"]),
            node("label", "02:06:55", ["SetFocus"]), node("label", DATE, ["SetFocus"]),
            node("push button", "Close", ["Press", "SetFocus"])]


class ClockShellAccessibilityTests(unittest.TestCase):
    def test_the_native_tree_is_exactly_the_frames_commands_time_and_date(self):
        self.assertIsNone(tree_mismatch(closed_tree(), ["Show clock"]))
        self.assertIsNone(tree_mismatch(open_tree(), ["Show clock", "Close"], DATE))
        self.assertIsNone(tree_mismatch(list(reversed(open_tree())), ["Close", "Show clock"], DATE))

    def test_any_other_role_name_or_count_is_a_mismatch(self):
        closed = [
            (closed_tree() + [node("frame")], "roles and names"),
            (closed_tree()[1:], "roles and names"),
            (closed_tree() + [node("push button", "Show clock", ["Press"])], "roles and names"),
            (closed_tree() + [node("push button", "Close", ["Press"])], "roles and names"),
            (closed_tree() + [node("panel")], "roles and names"),
            (closed_tree() + [node("label", "Show clock")], "static text"),
            (closed_tree() + [node("label", "")], "static text"),
        ]
        for nodes, message in closed:
            with self.subTest(nodes=nodes):
                self.assertRegex(tree_mismatch(nodes, ["Show clock"]), "^" + message)
        opened = [
            (open_tree()[:4] + open_tree()[5:], "static text"),
            (open_tree()[:5] + open_tree()[6:], "static text"),
            ([item if item["name"] != "02:06:55" else node("label", "2:06") for item in open_tree()], "static text"),
            (open_tree() + [node("label", "Close")], "static text"),
            ([item if item["name"] != DATE else node("label", "Tuesday") for item in open_tree()], "static text"),
            (open_tree()[1:], "roles and names"),
        ]
        for nodes, message in opened:
            with self.subTest(nodes=nodes):
                self.assertRegex(tree_mismatch(nodes, ["Show clock", "Close"], DATE), "^" + message)

    def test_only_buttons_carry_actions_and_every_button_can_be_pressed(self):
        nodes = open_tree()
        nodes[4] = dict(nodes[4], actions=[])
        self.assertIsNone(tree_mismatch(nodes, ["Show clock", "Close"], DATE))
        for index, actions in ((0, ["SetFocus"]), (0, ["Press"]), (4, ["SetFocus", "Press"]), (5, ["Press"])):
            nodes = open_tree()
            nodes[index] = dict(nodes[index], actions=actions)
            with self.subTest(role=nodes[index]["role"], actions=actions):
                self.assertRegex(tree_mismatch(nodes, ["Show clock", "Close"], DATE),
                                 "^actions on nodes other than Buttons beyond static-text SetFocus")
        nodes = open_tree()
        nodes[-1] = dict(nodes[-1], actions=["SetFocus"])
        self.assertRegex(tree_mismatch(nodes, ["Show clock", "Close"], DATE), "^Buttons without a Press action")


if __name__ == "__main__":
    unittest.main()
