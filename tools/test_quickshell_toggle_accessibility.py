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


if __name__ == "__main__":
    unittest.main()
