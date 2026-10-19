from pathlib import Path
import signal
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import check_quickshell_command_accessibility as accessibility
from check_slint_native_availability import BUTTON_ROLE

REPLY = "method return time=1.0 sender=:1.0 -> destination=:1.1 serial=3 reply_serial=2\n   "


def children(*paths):
    return REPLY + "array [\n" + "".join(
        f'      struct {{\n         string ":1.0"\n         object path "{path}"\n      }}\n' for path in paths) + "   ]"


def actions(*names):
    return REPLY + "array [\n" + "".join(
        f'      struct {{\n         string "{name}"\n         string ""\n         string ""\n      }}\n'
        for name in names) + "   ]"


class FakeTree:
    def __init__(self, nodes):
        self.nodes = nodes

    def __call__(self, destination, path, method, *arguments, address=None):
        role, kids = self.nodes[path]
        if method.endswith("GetRole"):
            return REPLY + f"uint32 {role}"
        if method.endswith("GetChildren"):
            return children(*kids)
        raise AssertionError(method)


def action_bus(names, reply):
    def call(destination, path, method, *arguments, address=None):
        return actions(*names) if method.endswith("GetActions") else REPLY + reply
    return call


class FakeProcess:
    def __init__(self, returncode=None, exits_on=()):
        self.returncode, self.exits_on, self.signals = returncode, exits_on, []

    def poll(self):
        return self.returncode

    def send_signal(self, number):
        self.signals.append(number)
        if number in self.exits_on:
            self.returncode = -number

    def terminate(self):
        self.send_signal(signal.SIGTERM)

    def kill(self):
        self.signals.append("kill")
        self.returncode = -9

    def wait(self, timeout=None):
        if self.returncode is None:
            raise subprocess.TimeoutExpired("orca", timeout)
        return self.returncode


class CommandAccessibilityEvidenceTests(unittest.TestCase):
    def test_buttons_are_found_anywhere_in_the_native_tree(self):
        tree = FakeTree({"/app": (75, ["/frame"]), "/frame": (23, ["/button", "/other"]),
                         "/button": (BUTTON_ROLE, []), "/other": (29, [])})
        with patch.object(accessibility, "call", tree):
            self.assertEqual(accessibility.buttons("unix:test", (":1.0", "/app")), [(":1.0", "/button")])

    def test_decorative_command_anatomy_must_not_reach_the_native_tree(self):
        tree = FakeTree({"/app": (75, ["/button"]), "/button": (BUTTON_ROLE, ["/glyph"]), "/glyph": (29, [])})
        with patch.object(accessibility, "call", tree), self.assertRaisesRegex(ValueError, "decorative"):
            accessibility.buttons("unix:test", (":1.0", "/app"))

    def test_cyclic_native_trees_are_rejected(self):
        tree = FakeTree({"/app": (75, ["/frame"]), "/frame": (23, ["/app"])})
        with patch.object(accessibility, "call", tree), self.assertRaisesRegex(ValueError, "cyclic"):
            accessibility.buttons("unix:test", (":1.0", "/app"))
        tree = FakeTree({f"/{index}": (23, [f"/{index + 1}"]) for index in range(40)})
        with patch.object(accessibility, "call", tree), self.assertRaisesRegex(ValueError, "excessive"):
            accessibility.buttons("unix:test", (":1.0", "/0"))

    def test_live_press_requires_one_accepted_press_action(self):
        tree = accessibility.Tree(None, "unix:test")
        with patch.object(accessibility, "call", action_bus(("Press", "SetFocus"), "boolean true")):
            tree.press((":1.0", "/button"))
        for names, reply, message in ((("SetFocus",), "boolean true", "Press action"),
                                      (("Press", "Press"), "boolean true", "Press action"),
                                      (("Press",), "boolean false", "not accepted")):
            with self.subTest(names=names, reply=reply), \
                    patch.object(accessibility, "call", action_bus(names, reply)), \
                    self.assertRaisesRegex(ValueError, message):
                tree.press((":1.0", "/button"))

    def test_retired_press_must_fail_as_an_unknown_object(self):
        tree = accessibility.Tree(None, "unix:test")

        def retired(*arguments, address=None):
            raise ValueError("D-Bus call failed: Error org.freedesktop.DBus.Error.UnknownObject: "
                             "No such object path '/button'")
        with patch.object(accessibility, "call", retired):
            tree.press_retired((":1.0", "/button"))
        for reply, message in (("boolean true", "accepted Press"), ("boolean false", "unknown object")):
            with self.subTest(reply=reply), patch.object(accessibility, "call", action_bus(("Press",), reply)), \
                    self.assertRaisesRegex(ValueError, message):
                tree.press_retired((":1.0", "/button"))

    def test_native_events_are_read_from_monitored_signals(self):
        log = "\n".join((
            "signal time=1.0 sender=:1.1 -> destination=(null destination) serial=4 "
            "path=/org/a11y/atspi/accessible/21; interface=org.a11y.atspi.Event.Object; member=StateChanged",
            '   string "sensitive"', "   int32 0", "   int32 0", "   variant       int32 0",
            "signal time=1.1 sender=:1.1 -> destination=(null destination) serial=5 "
            "path=/org/a11y/atspi/accessible/22; interface=org.a11y.atspi.Event.Focus; member=Focus",
            '   string ""', "   int32 0", "   int32 0"))
        self.assertEqual(accessibility.signals(log), [
            ("/org/a11y/atspi/accessible/21", "Object.StateChanged", "sensitive", 0, None),
            ("/org/a11y/atspi/accessible/22", "Focus.Focus", "", 0, None)])

    def test_updates_must_keep_a_named_node_in_the_tree(self):
        def signal(path, member, kind, *value):
            return "\n".join((
                "signal time=1.0 sender=:1.1 -> destination=(null destination) serial=4 "
                f"path={path}; interface=org.a11y.atspi.Event.Object; member={member}",
                f'   string "{kind}"', "   int32 0", "   int32 0", *value, "   array [", "   ]"))

        def child(path):
            return ("   variant       struct {", '         string ":1.1"', f'         object path "{path}"', "      }")
        name = signal("/node", "PropertyChange", "accessible-name", '   variant       string "Reconnect"')
        unnamed = signal("/node", "PropertyChange", "accessible-name", '   variant       string ""')
        undescribed = signal("/node", "PropertyChange", "accessible-description", '   variant       string ""')
        removed = signal("/frame", "ChildrenChanged", "remove", *child("/node"))
        added = signal("/frame", "ChildrenChanged", "add", *child("/node"))
        other = (signal("/frame", "ChildrenChanged", "add", *child("/other")) + "\n"
                 + signal("/other", "PropertyChange", "accessible-name", '   variant       string ""'))
        with tempfile.TemporaryDirectory() as directory:
            events = accessibility.NativeEvents.__new__(accessibility.NativeEvents)
            events.log = Path(directory) / "events.log"
            events.log.write_text(name + "\n" + other, encoding="utf-8")
            events.retained(0, (":1.1", "/node"), "update")
            for log, message in ((name + "\n" + undescribed, "cleared the native Command description"),
                                 (unnamed + "\n" + name, "cleared the native Command name"),
                                 (removed + "\n" + name, r"withdrew the native Command from the tree \(remove\)"),
                                 (name + "\n" + added, r"withdrew the native Command from the tree \(add\)")):
                with self.subTest(message=message):
                    events.log.write_text(log, encoding="utf-8")
                    with self.assertRaisesRegex(ValueError, message):
                        events.retained(0, (":1.1", "/node"), "update")
            events.log.write_text(removed + "\n" + name, encoding="utf-8")
            events.retained(1, (":1.1", "/node"), "update")

    def test_native_state_decodes_availability_and_focus_bits(self):
        words = (1 << 8) | (1 << 11) | (1 << 12)
        reply = REPLY + f"array [\n      uint32 {words}\n      uint32 0\n   ]"
        with patch.object(accessibility, "call", lambda *arguments, address=None: reply):
            self.assertEqual(accessibility.state("unix:test", (":1.0", "/button")),
                             {"enabled": True, "sensitive": False, "focusable": True, "focused": True})

    def test_focus_presentation_must_speak_name_role_and_description_in_order(self):
        speech = ["'Screen reader on.' {}", "'Reconnect' {'established': False}", "'button.' {}",
                  "'Connect using saved settings.' {}"]
        self.assertTrue(accessibility.announced(speech, "Reconnect", "button.", "Connect using saved settings."))
        self.assertFalse(accessibility.announced(speech[:3], "Reconnect", "button.", "Connect using saved settings."))
        self.assertFalse(accessibility.announced([speech[2], speech[1], speech[3]],
                                                 "Reconnect", "button.", "Connect using saved settings."))
        self.assertFalse(accessibility.announced(["'Reconnect now' {}", "'button.' {}",
                                                  "'Connect using saved settings.' {}"],
                                                 "Reconnect", "button.", "Connect using saved settings."))

    def test_screen_reader_presentation_is_read_from_the_marked_position(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "orca.debug"
            log.write_text("\n".join((
                "10:00:00 - SPEECH OUTPUT: 'Screen reader on.' {}",
                "10:00:01 - SPEECH OUTPUT: 'Reconnect' {'established': False}",
                "10:00:01 - BRAILLE LINE:  'quickshell application frame Reconnect button Connect using saved settings.'",
                "10:00:02 - SPEECH OUTPUT: 'grayed.' {}")), encoding="utf-8")
            speech, braille = accessibility.presentation(log, 1)
            self.assertEqual([item.split(" ", 1)[0] for item in speech], ["'Reconnect'", "'grayed.'"])
            self.assertEqual(braille, ["'quickshell application frame Reconnect button Connect using saved settings.'"])
            self.assertEqual(accessibility.lines(log), 4)
            self.assertEqual(accessibility.presentation(log, 4), ([], []))

    def test_orca_start_reports_the_exit_status_of_a_stopped_screen_reader(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "orca.debug"
            log.write_text("10:00:00 - SPEECH OUTPUT: 'Screen reader on.' {}", encoding="utf-8")
            self.assertTrue(accessibility.started(FakeProcess(), log))
            self.assertFalse(accessibility.started(FakeProcess(), Path(directory) / "missing.debug"))
            with self.assertRaisesRegex(ValueError, "Orca exited with return code -9 before it announced itself"):
                accessibility.started(FakeProcess(-9), log)

    def test_a_screen_reader_that_failed_to_start_dumps_its_stacks_before_it_is_stopped(self):
        aborted = FakeProcess(exits_on=(signal.SIGABRT,))
        accessibility.stop(aborted, abort=True)
        self.assertEqual(aborted.signals, [signal.SIGABRT])
        ignoring = FakeProcess(exits_on=(signal.SIGTERM,))
        accessibility.stop(ignoring, abort=True)
        self.assertEqual(ignoring.signals, [signal.SIGABRT, signal.SIGTERM])
        stopped = FakeProcess(exits_on=(signal.SIGTERM,))
        accessibility.stop(stopped)
        self.assertEqual(stopped.signals, [signal.SIGTERM])
        exited = FakeProcess(0)
        accessibility.stop(exited, abort=True)
        self.assertEqual(exited.signals, [])


if __name__ == "__main__":
    unittest.main()
