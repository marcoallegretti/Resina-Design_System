import unittest

from check_focus_svg_backend import check_svg
from check_schemas import ROOT, load_json


class FocusSvgTests(unittest.TestCase):
    def setUp(self):
        self.source = (ROOT / "conformance/web/focus-baseline.svg").read_text(encoding="utf-8")
        self.expected = load_json(ROOT / "conformance/ir/focus-ir-expected.json")

    def test_baseline_preserves_closed_boundaries_and_actual_color(self):
        check_svg(self.source, self.expected)
        self.expected["indicator"]["color"]["components"] = [0.8, 0.9, 1]
        check_svg(self.source.replace("100% 100% 100%", "80% 90% 100%"), self.expected)

    def test_altered_geometry_pigment_hole_and_focus_semantics_fail(self):
        for before, after in (
            ('viewBox="-4 -4 28 22"', 'viewBox="0 0 28 22"'),
            ('height="22"', 'height="20"'),
            ("100% 100% 100%", "90% 100% 100%"),
            ('fill-rule="evenodd"', 'fill-rule="nonzero"'),
            ("A 4 4 0 0 1", "A 4 4 0 0 0"),
            ("A 4 4 0 0 1", "A 4 4 0 0.0 1"),
            ("A 2 2 0 0 1", "A 3 3 0 0 1"),
            ("L 24 14", "L 24 12"),
            ('focusable="false"', 'focusable="true"'),
            ('<path ', '<path stroke="red" '),
            ("</svg>", "<script/></svg>"),
            ("M -4 0", "M NaN 0"),
            (" Z M -2", " M -2"),
        ):
            with self.subTest(alteration=after), self.assertRaises(ValueError):
                check_svg(self.source.replace(before, after), self.expected)


if __name__ == "__main__":
    unittest.main()
