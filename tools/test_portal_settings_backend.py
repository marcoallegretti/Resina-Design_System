import tempfile
import unittest
from pathlib import Path
from unittest import mock

import portal_settings_backend
from portal_settings_backend import (APPEARANCE, BUS_NAME, CONTROL, CONTROL_PATH, INVALID, NOT_FOUND, PORTAL_PATH,
                                     PROPERTIES, SETTINGS, UNKNOWN, Settings, matches, portal_directory)

KEYS = ["color-scheme", "contrast", "reduced-motion"]


def control(settings, key, variant):
    return settings.handle(CONTROL_PATH, CONTROL, "Set", "ssv", (APPEARANCE, key, variant))


class NamespaceTests(unittest.TestCase):
    def test_an_empty_filter_or_an_empty_pattern_matches_every_namespace(self):
        self.assertTrue(matches(APPEARANCE, []))
        self.assertTrue(matches(APPEARANCE, ["org.example", ""]))

    def test_patterns_match_exactly_or_by_a_trailing_section_glob(self):
        self.assertTrue(matches(APPEARANCE, [APPEARANCE]))
        self.assertTrue(matches(APPEARANCE, ["org.freedesktop.*"]))
        self.assertFalse(matches(APPEARANCE, ["org.freedesktop.appear"]))
        self.assertFalse(matches(APPEARANCE, ["org.freedesktop.appearance.*"]))
        self.assertFalse(matches(APPEARANCE, ["org.gnome.*", "org.freedesktop"]))


class SettingsTests(unittest.TestCase):
    def test_provided_keys_start_without_a_preference(self):
        settings = Settings(KEYS)
        self.assertEqual(settings.read_all([APPEARANCE]),
                         {APPEARANCE: {"color-scheme": ("u", 0), "contrast": ("u", 0), "reduced-motion": ("u", 0)}})
        self.assertEqual(settings.read_all(["org.gnome.*"]), {})

    def test_only_known_appearance_keys_can_be_provided(self):
        for keys in ([], ["accent-color"], ["reduced-motion", "reduced-transparency"]):
            with self.assertRaisesRegex(ValueError, "non-empty subset"):
                Settings(keys)

    def test_an_unprovided_key_is_not_found_and_cannot_be_set(self):
        settings = Settings(["contrast"])
        self.assertEqual(settings.read_all([]), {APPEARANCE: {"contrast": ("u", 0)}})
        for namespace, key in ((APPEARANCE, "reduced-motion"), ("org.gnome.desktop", "contrast")):
            reply, _ = settings.handle(PORTAL_PATH, SETTINGS, "Read", "ss", (namespace, key))
            self.assertEqual(reply[:2], ("error", NOT_FOUND))
        reply, signals = control(settings, "reduced-motion", ("u", 1))
        self.assertEqual((reply[:2], signals), (("error", NOT_FOUND), []))

    def test_values_outside_the_portal_domain_are_rejected_unchanged(self):
        settings = Settings(KEYS)
        for key, variant in (("reduced-motion", ("u", 2)), ("contrast", ("b", True)), ("color-scheme", ("u", 3)),
                             ("color-scheme", ("s", "dark"))):
            reply, signals = control(settings, key, variant)
            self.assertEqual((reply[:2], signals), (("error", INVALID), []))
        self.assertEqual(settings.read(APPEARANCE, "color-scheme"), ("u", 0))

    def test_a_change_is_readable_and_emitted_once(self):
        settings = Settings(KEYS)
        reply, signals = control(settings, "reduced-motion", ("u", 1))
        self.assertEqual(reply, ("return", "", ()))
        self.assertEqual(signals, [(APPEARANCE, "reduced-motion", ("u", 1))])
        self.assertEqual(settings.handle(PORTAL_PATH, SETTINGS, "Read", "ss", (APPEARANCE, "reduced-motion")),
                         (("return", "v", (("u", 1),)), []))
        reply, signals = settings.handle(PORTAL_PATH, SETTINGS, "ReadAll", "as", ([APPEARANCE],))
        self.assertEqual(reply[:2], ("return", "a{sa{sv}}"))
        self.assertEqual(reply[2][0][APPEARANCE]["reduced-motion"], ("u", 1))
        self.assertEqual(control(settings, "reduced-motion", ("u", 1)), (("return", "", ()), []))
        self.assertEqual(control(settings, "reduced-motion", ("u", 0))[1], [(APPEARANCE, "reduced-motion", ("u", 0))])

    def test_the_backend_reports_its_interface_version(self):
        settings = Settings(KEYS)
        self.assertEqual(settings.handle(PORTAL_PATH, PROPERTIES, "Get", "ss", (SETTINGS, "version")),
                         (("return", "v", (("u", 1),)), []))
        self.assertEqual(settings.handle(PORTAL_PATH, PROPERTIES, "GetAll", "s", (SETTINGS,))[0][2],
                         ({"version": ("u", 1)},))
        self.assertEqual(settings.handle(PORTAL_PATH, PROPERTIES, "GetAll", "s", ("org.example",))[0][2], ({},))
        self.assertEqual(settings.handle(PORTAL_PATH, PROPERTIES, "Get", "ss", (SETTINGS, "other"))[0][:2],
                         ("error", NOT_FOUND))

    def test_calls_the_backend_does_not_serve_are_unknown_methods(self):
        settings = Settings(KEYS)
        for call in ((PORTAL_PATH, SETTINGS, "ReadOne", "ss", (APPEARANCE, "contrast")),
                     (PORTAL_PATH, SETTINGS, "Read", "s", (APPEARANCE,)),
                     (CONTROL_PATH, SETTINGS, "Read", "ss", (APPEARANCE, "contrast")),
                     (PORTAL_PATH, CONTROL, "Set", "ssv", (APPEARANCE, "contrast", ("u", 1))),
                     ("/", "org.freedesktop.DBus.Introspectable", "Introspect", "", ())):
            reply, signals = settings.handle(*call)
            self.assertEqual((reply[:2], signals), (("error", UNKNOWN), []))
        self.assertEqual(settings.read(APPEARANCE, "contrast"), ("u", 0))


class LaunchTests(unittest.TestCase):
    def test_the_portal_directory_selects_only_this_backend_for_settings(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = portal_directory(Path(temporary) / "portals")
            self.assertEqual(sorted(path.name for path in directory.iterdir()), ["portals.conf", "resinatest.portal"])
            self.assertEqual((directory / "resinatest.portal").read_text(encoding="utf-8"),
                             f"[portal]\nDBusName={BUS_NAME}\nInterfaces={SETTINGS};\n")
            self.assertEqual((directory / "portals.conf").read_text(encoding="utf-8"),
                             f"[preferred]\n{SETTINGS}=resinatest\n")
            with self.assertRaises(FileExistsError):
                portal_directory(directory)

    def test_values_are_set_through_the_control_interface(self):
        with mock.patch.object(portal_settings_backend.subprocess, "run") as run:
            portal_settings_backend.set_value({"DBUS_SESSION_BUS_ADDRESS": "unix:path=/bus"}, "reduced-motion", 1)
        run.assert_called_once_with(
            ["gdbus", "call", "--session", "--dest", BUS_NAME, "--object-path", CONTROL_PATH,
             "--method", CONTROL + ".Set", APPEARANCE, "reduced-motion", "<uint32 1>"],
            env={"DBUS_SESSION_BUS_ADDRESS": "unix:path=/bus"}, check=True, capture_output=True, timeout=10)

    def test_a_backend_that_does_not_report_ready_is_a_failure(self):
        process = mock.Mock()
        process.stdout.readline.return_value = f"{BUS_NAME} is already owned\n"
        with mock.patch.object(portal_settings_backend.subprocess, "Popen", return_value=process):
            with self.assertRaisesRegex(ValueError, "did not start: .* is already owned"):
                portal_settings_backend.start({}, KEYS, None)
        process.kill.assert_called_once_with()

    def test_the_frontend_is_ready_only_once_it_owns_the_portal_name(self):
        process = mock.Mock()
        process.poll.return_value = None
        with mock.patch.object(portal_settings_backend.subprocess, "Popen", return_value=process) as popen, \
                mock.patch.object(portal_settings_backend, "owned", side_effect=[False, True]) as owned, \
                mock.patch.object(portal_settings_backend.time, "sleep"):
            self.assertIs(portal_settings_backend.start_frontend("/portal", {"A": "1"}, Path("/portals"), None),
                          process)
        self.assertEqual(popen.call_args.kwargs["env"], {"A": "1", "XDG_DESKTOP_PORTAL_DIR": str(Path("/portals"))})
        self.assertEqual(owned.call_args_list, [mock.call({"A": "1"}, "org.freedesktop.portal.Desktop")] * 2)
        process.kill.assert_not_called()

    def test_a_frontend_that_exits_before_owning_the_portal_name_is_a_failure(self):
        process = mock.Mock(returncode=1)
        process.poll.return_value = 1
        with mock.patch.object(portal_settings_backend.subprocess, "Popen", return_value=process), \
                mock.patch.object(portal_settings_backend, "owned", return_value=False):
            with self.assertRaisesRegex(ValueError, "did not take org.freedesktop.portal.Desktop \\(exit status 1\\)"):
                portal_settings_backend.start_frontend("/portal", {}, Path("/portals"), None)


if __name__ == "__main__":
    unittest.main()
