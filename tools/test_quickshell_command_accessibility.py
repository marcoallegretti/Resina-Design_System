from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check_quickshell_command_accessibility as accessibility
from check_slint_native_availability import BUTTON_ROLE

REPLY = "method return time=1.0 sender=:1.0 -> destination=:1.1 serial=3 reply_serial=2\n   "


def children(*paths):
    return REPLY + "array [\n" + "".join(
        f'      struct {{\n         string ":1.0"\n         object path "{path}"\n      }}\n' for path in paths) + "   ]"


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


if __name__ == "__main__":
    unittest.main()
