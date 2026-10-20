import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
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


class FakeExposedTree:
    """Nodes are (role name, name, interfaces, actions, state bits, children)."""

    def __init__(self, nodes):
        self.nodes = nodes

    def __call__(self, destination, path, method, *arguments, address=None):
        role, name, interfaces, names, bits, kids = self.nodes[path]
        if method.endswith("GetRoleName"):
            return REPLY + f'string "{role}"'
        if method.endswith("GetRole"):
            return REPLY + f"uint32 {BUTTON_ROLE if role == 'push button' else 23}"
        if method.endswith("GetChildren"):
            return children(*kids)
        if method.endswith("GetInterfaces"):
            return REPLY + "array [\n" + "".join(f'      string "{item}"\n' for item in interfaces) + "   ]"
        if method.endswith("GetActions"):
            return actions(*names)
        if method.endswith("GetState"):
            return REPLY + f"array [\n      uint32 {bits & 0xffffffff}\n      uint32 {bits >> 32}\n   ]"
        if method.endswith("Properties.Get") and arguments[-1] == "string:Name":
            return REPLY + f'variant       string "{name}"'
        raise AssertionError(method)


PLAIN = ("org.a11y.atspi.Accessible",)
ACTIONABLE = PLAIN + (accessibility.ACTION,)


def withdrawn_tree(**extra):
    nodes = {"/app": ("application", "quickshell", PLAIN, (), 0, ["/frame", *("/" + path for path in extra)]),
             "/frame": ("frame", "", ACTIONABLE, (), 0, [])}
    nodes.update({"/" + path: node for path, node in extra.items()})
    return FakeExposedTree(nodes)


class ImmediateSession:
    def wait(self, predicate, description, timeout):
        if not predicate():
            raise ValueError(description + " was not reached")


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

    def test_the_exposed_tree_reports_every_node_with_its_role_name_and_actions(self):
        tree = withdrawn_tree(target=("panel", "Focus target", ACTIONABLE, ("SetFocus",), 0, []))
        with patch.object(accessibility, "call", tree):
            self.assertEqual(accessibility.exposed("unix:test", (":1.0", "/app")), [
                {"node": (":1.0", "/frame"), "role": "frame", "name": "", "actions": []},
                {"node": (":1.0", "/target"), "role": "panel", "name": "Focus target", "actions": ["SetFocus"]}])
        tree = FakeExposedTree({"/app": ("application", "", PLAIN, (), 0, ["/frame"]),
                                "/frame": ("frame", "", PLAIN, (), 0, ["/app"])})
        with patch.object(accessibility, "call", tree), self.assertRaisesRegex(ValueError, "cyclic"):
            accessibility.exposed("unix:test", (":1.0", "/app"))
        self.assertEqual(accessibility.listed_actions(actions()), [])
        for reply in ("array [ junk ]", "boolean true"):
            with self.subTest(reply=reply):
                with self.assertRaises(ValueError):
                    accessibility.strings(REPLY + reply)
                with self.assertRaises(ValueError):
                    accessibility.listed_actions(REPLY + reply)

    def test_a_withdrawn_tree_exposes_no_named_or_actionable_node(self):
        def node(role, name, names=()):
            return {"node": (":1.0", "/" + role), "role": role, "name": name, "actions": list(names)}
        frame, target = node("frame", ""), node("panel", "Focus target")
        retained = [("panel", "Focus target", ())]
        self.assertEqual(accessibility.leaked([frame]), [])
        self.assertEqual(accessibility.leaked([frame, target], retained), [])
        self.assertEqual(accessibility.leaked([frame, node("label", "Reconnect")]), [("label", "Reconnect", ())])
        self.assertEqual(accessibility.leaked([node("filler", "", ["Press"])]), [("filler", "", ("Press",))])
        self.assertEqual(accessibility.leaked([target, target], retained), [("panel", "Focus target", ())])
        self.assertEqual(accessibility.leaked([frame, node("panel", "Focus target", ["SetFocus"])], [])
                         , [("panel", "Focus target", ("SetFocus",))])
        with self.assertRaisesRegex(ValueError, "lost its retained nodes"):
            accessibility.leaked([frame], retained)
        with self.assertRaisesRegex(ValueError, "lost its retained nodes"):
            accessibility.leaked([frame, node("panel", "Focus target", ["SetFocus"])], retained)

    def test_withdrawal_waits_for_a_tree_without_named_or_actionable_nodes(self):
        tree = accessibility.Tree(ImmediateSession(), "unix:test")
        tree.application = lambda: (":1.0", "/app")
        with patch.object(accessibility, "call", withdrawn_tree()):
            tree.absent("withdrawn")
        target = ("panel", "Focus target", PLAIN, (), 0, [])
        with patch.object(accessibility, "call", withdrawn_tree(target=target)):
            tree.absent("withdrawn", (("panel", "Focus target", ()),))
        for extra, message in (({"label": ("label", "Reconnect", PLAIN, (), 0, [])},
                                r"\[\('label', 'Reconnect', \(\)\)\]"),
                               ({"filler": ("filler", "", ACTIONABLE, ("Press",), 0, [])},
                                r"\[\('filler', '', \('Press',\)\)\]"),
                               ({"button": ("push button", "", PLAIN, (), 0, [])}, r"\[\]$"),
                               ({"target": target}, r"\[\('panel', 'Focus target', \(\)\)\]")):
            with self.subTest(extra=extra), patch.object(accessibility, "call", withdrawn_tree(**extra)), \
                    self.assertRaisesRegex(ValueError, "^withdrawn was not reached; named or actionable nodes: "
                                           + message):
                tree.absent("withdrawn")

    def test_feedback_must_not_be_exposed_as_native_state(self):
        available = (1 << 8) | (1 << 11) | (1 << 12) | (1 << 24) | (1 << 2)
        tree = accessibility.Tree(None, "unix:test")
        with patch.object(accessibility, "call", FakeExposedTree({"/button": ("push button", "Reconnect", PLAIN, (),
                                                                               available, [])})):
            tree.unexposed((":1.0", "/button"), "hover")
        for bit, name in ((20, "pressed"), (4, "checked"), (23, "selected"), (41, "checkable")):
            states = FakeExposedTree({"/button": ("push button", "Reconnect", PLAIN, (), available | 1 << bit, [])})
            with self.subTest(name=name), patch.object(accessibility, "call", states), \
                    self.assertRaisesRegex(ValueError, f"^the native Command exposes {name} during a Space hold$"):
                tree.unexposed((":1.0", "/button"), "a Space hold")

    def test_feedback_must_not_emit_toggle_or_press_state_changes(self):
        def changed(path, kind):
            return "\n".join((
                "signal time=1.0 sender=:1.1 -> destination=(null destination) serial=4 "
                f"path={path}; interface=org.a11y.atspi.Event.Object; member=StateChanged",
                f'   string "{kind}"', "   int32 1", "   int32 0", "   variant       int32 0"))
        with tempfile.TemporaryDirectory() as directory:
            events = accessibility.NativeEvents.__new__(accessibility.NativeEvents)
            events.log = Path(directory) / "events.log"
            events.log.write_text("\n".join((changed("/node", "focused"), changed("/other", "checked"),
                                              changed("/node", "armed"))), encoding="utf-8")
            events.unexposed(0, (":1.1", "/node"), "hover")
            for kind in ("checked", "pressed", "selected", "checkable"):
                events.log.write_text(changed("/node", kind) + "\n" + changed("/node", "focused"), encoding="utf-8")
                with self.subTest(kind=kind), \
                        self.assertRaisesRegex(ValueError, f"^hover emitted native StateChanged events for {kind}$"):
                    events.unexposed(0, (":1.1", "/node"), "hover")
                events.unexposed(1, (":1.1", "/node"), "hover")

    def test_every_native_press_must_reach_the_consumer_after_committed_activation(self):
        effect = {"state": {"enabled": True, "hold": None}, "ready": False}
        self.assertEqual(accessibility.committed([effect], "native Press after replacement"), [effect])
        for broken in ({"state": {"enabled": False, "hold": None}, "ready": True},
                       {"state": {"enabled": True, "hold": {"kind": "pointer", "inside": True}}, "ready": True}):
            with self.subTest(broken=broken), self.assertRaisesRegex(
                    ValueError, "^native Press after replacement reached the consumer before committed activation"):
                accessibility.committed([effect, broken], "native Press after replacement")

    def test_every_native_press_leaves_a_ready_available_command_without_a_hold(self):
        effect = {"kind": "effect", "state": {"enabled": True, "hold": None}, "ready": False}
        settled = {"ready": True, "state": {"enabled": True, "hold": None}}

        class Session:
            def __init__(self, observed):
                self.log, self.observed = [], observed

            def events(self):
                return self.log

            def wait(self, predicate, description, timeout):
                if not predicate():
                    raise ValueError(description)

            def quiet(self):
                pass

            def until(self, predicate, description, timeout):
                if not predicate(self.observed):
                    raise ValueError(description + " was not reached")

        class Pressing:
            def __init__(self, session):
                self.session = session

            def press(self, node):
                self.session.log.append(effect)
        session = Session(settled)
        accessibility.press_once(session, Pressing(session), None, "native Press after replacement")
        for observed in (dict(settled, ready=False), {"ready": True, "state": {"enabled": False, "hold": None}},
                         {"ready": True, "state": {"enabled": True, "hold": {"kind": "key", "key": "space"}}}):
            session = Session(observed)
            with self.subTest(observed=observed), self.assertRaisesRegex(
                    ValueError, "^ready, available Command without a hold after native Press after replacement"):
                accessibility.press_once(session, Pressing(session), None, "native Press after replacement")

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

    def test_focus_presentation_follows_the_verified_orca_release(self):
        self.assertEqual(accessibility.verified_release("46.1"), "46.1")
        self.assertEqual(accessibility.verified_release("Orca version 50.3, AT-SPI2 version: 2.60.7"), "50.3")
        self.assertEqual(accessibility.FOCUS_PRESENTATIONS["46.1"]("Reconnect", "Connect using saved settings."),
                         (("Reconnect push button.", "Connect using saved settings."),
                          "quickshell application frame Reconnect push button"))
        self.assertEqual(accessibility.FOCUS_PRESENTATIONS["50.3"]("Reconnect", "Connect using saved settings."),
                         (("Reconnect", "button.", "Connect using saved settings."),
                          "quickshell application frame Reconnect button Connect using saved settings."))
        speech = ["'Screen reader on.' {}", "'Reconnect push button.' {'established': False}",
                  "'Connect using saved settings.' {'established': False}"]
        self.assertTrue(accessibility.announced(speech, "Reconnect push button.", "Connect using saved settings."))
        for version in ("Orca version 50.4, AT-SPI2 version: 2.60.7", "50.3.1", "46", ""):
            with self.assertRaisesRegex(ValueError, "no verified focus presentation .*verified releases: 46.1, 50.3"):
                accessibility.verified_release(version)

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

    def test_orca_start_reads_the_announcement_before_the_exit_status(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "orca.debug"
            missing = Path(directory) / "missing.debug"
            log.write_text("10:00:00 - SPEECH OUTPUT: 'Screen reader on.' {}", encoding="utf-8")
            later = time.monotonic() + 60
            self.assertTrue(accessibility.started(FakeProcess(), log, later))
            self.assertTrue(accessibility.started(FakeProcess(0), log, 0))
            self.assertFalse(accessibility.started(FakeProcess(), missing, later))
            with self.assertRaisesRegex(ValueError, "Orca exited with return code -9 before its announcement "
                                                    "'Screen reader on.' was read"):
                accessibility.started(FakeProcess(-9), missing, later)
            with self.assertRaises(accessibility.StartExpired):
                accessibility.started(FakeProcess(), missing, 0)

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

    @unittest.skipUnless(os.name == "posix", "the screen reader runs on a pseudo-terminal")
    def test_only_a_screen_reader_that_never_announced_itself_is_sent_sigabrt(self):
        def start(version, failure, process):
            def wait(predicate, description, timeout):
                if failure:
                    raise ValueError(failure)
                predicate()
            reply = subprocess.CompletedProcess([], 0, version + "\n", "")
            with tempfile.TemporaryDirectory() as directory, \
                    patch.object(accessibility.subprocess, "run", return_value=reply), \
                    patch.object(accessibility.subprocess, "Popen", return_value=process) as launched, \
                    patch.object(accessibility, "ORCA_START_TIMEOUT", 0):
                try:
                    accessibility.ScreenReader(wait, Path(directory))
                finally:
                    self.launched = launched.called

        unverified = FakeProcess()
        with self.assertRaisesRegex(ValueError, "no verified focus presentation for 50.4"):
            start("50.4", None, unverified)
        self.assertFalse(self.launched)
        shell = FakeProcess(exits_on=(signal.SIGTERM,))
        with self.assertRaisesRegex(ValueError, "^Orca start was not reached: the shell exited$"):
            start("46.1", "Orca start was not reached: the shell exited", shell)
        self.assertEqual(shell.signals, [signal.SIGTERM])
        silent = FakeProcess(exits_on=(signal.SIGABRT,))
        with self.assertRaisesRegex(ValueError, "Orca did not announce 'Screen reader on.' within 0 s; Orca was "
                                                "still running and was sent SIGABRT"):
            start("46.1", None, silent)
        self.assertEqual(silent.signals, [signal.SIGABRT])
        exited = FakeProcess(-9)
        with self.assertRaisesRegex(ValueError, "return code -9"):
            start("46.1", None, exited)
        self.assertEqual(exited.signals, [])


if __name__ == "__main__":
    unittest.main()
