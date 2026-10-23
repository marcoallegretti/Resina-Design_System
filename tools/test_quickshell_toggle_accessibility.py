from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check_quickshell_toggle_accessibility as accessibility
from test_quickshell_command_accessibility import FakeTree


class ToggleAccessibilityEvidenceTests(unittest.TestCase):
    def names(self, tree, names):
        def text(address, node, field):
            return names[node[1]]
        return patch.object(accessibility, "call", tree), patch.object(accessibility, "text", text)

    def test_the_named_node_is_found_anywhere_in_the_native_tree(self):
        tree = FakeTree({"/app": (75, ["/frame"]), "/frame": (23, ["/toggle", "/other"]),
                         "/toggle": (7, []), "/other": (43, [])})
        call, text = self.names(tree, {"/frame": "", "/toggle": accessibility.NAME, "/other": "Focus target"})
        with call, text:
            self.assertEqual(accessibility.named("unix:test", (":1.0", "/app"), accessibility.NAME),
                             [(":1.0", "/toggle")])

    def test_decorative_toggle_anatomy_must_not_reach_the_native_tree(self):
        tree = FakeTree({"/app": (75, ["/toggle"]), "/toggle": (7, ["/thumb"]), "/thumb": (29, [])})
        call, text = self.names(tree, {"/toggle": accessibility.NAME, "/thumb": ""})
        with call, text, self.assertRaisesRegex(ValueError, "decorative"):
            accessibility.named("unix:test", (":1.0", "/app"), accessibility.NAME)

    def test_cyclic_native_trees_are_rejected(self):
        tree = FakeTree({"/app": (75, ["/frame"]), "/frame": (23, ["/app"])})
        call, text = self.names(tree, {"/frame": ""})
        with call, text, self.assertRaisesRegex(ValueError, "cyclic"):
            accessibility.named("unix:test", (":1.0", "/app"), accessibility.NAME)

    def test_a_withdrawn_toggle_is_reported_as_a_toggle(self):
        def signal(path, member, kind, *value):
            return "\n".join((
                "signal time=1.0 sender=:1.1 -> destination=(null destination) serial=4 "
                f"path={path}; interface=org.a11y.atspi.Event.Object; member={member}",
                f'   string "{kind}"', "   int32 0", "   int32 0", *value, "   array [", "   ]"))
        removed = signal("/frame", "ChildrenChanged", "remove", "   variant       struct {",
                         '         string ":1.1"', '         object path "/node"', "      }")
        unnamed = signal("/node", "PropertyChange", "accessible-name", '   variant       string ""')
        with tempfile.TemporaryDirectory() as directory:
            events = accessibility.NativeEvents.__new__(accessibility.NativeEvents)
            events.log = Path(directory) / "events.log"
            for log, message in ((removed, r"update withdrew the native Toggle from the tree \(remove\)"),
                                 (unnamed, "update cleared the native Toggle name")):
                with self.subTest(message=message):
                    events.log.write_text(log, encoding="utf-8")
                    with self.assertRaisesRegex(ValueError, message):
                        events.retained(0, (":1.1", "/node"), "update", "Toggle")


if __name__ == "__main__":
    unittest.main()
