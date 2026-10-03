import unittest

from check_focus_slint_backend import check_slint
from check_schemas import ROOT, load_json


class FocusSlintTests(unittest.TestCase):
    def test_baseline_and_whitespace(self):
        expected = load_json(ROOT / "conformance/ir/focus-ir-expected.json")
        source = (ROOT / "conformance/slint/focus-baseline.slint").read_text(encoding="utf-8")
        check_slint(source, expected)
        check_slint("\n" + source.replace("\n", "\n    "), expected)

    def test_rejects_geometry_color_winding_scaling_and_extra_behavior(self):
        expected = load_json(ROOT / "conformance/ir/focus-ir-expected.json")
        source = (ROOT / "conformance/slint/focus-baseline.slint").read_text(encoding="utf-8")
        for old, new in (
            ("paint-origin-x: -4px", "paint-origin-x: 0px"),
            ("viewbox-height: 22", "viewbox-height: 20"),
            ("rgb(255, 255, 255)", "rgb(255, 0, 255)"),
            ("nonzero", "evenodd"),
            ("radius-x: 4", "radius-x: 3"),
            ("sweep: false", "sweep: true"),
            ("large-arc: false", "large-arc: true"),
            ("stroke-width: 0px", "stroke-width: 1px"),
            ("accessible-role: none", "accessible-role: button"),
            ("fit: preserve", "fit: fill"),
            ("Close {}", ""),
            ("clip: false", "clip: true"),
            ("Path {", "TouchArea {}\nPath {"),
        ):
            with self.subTest(change=new):
                self.assertIn(old, source)
                with self.assertRaises(ValueError):
                    check_slint(source.replace(old, new, 1), expected)


if __name__ == "__main__":
    unittest.main()
