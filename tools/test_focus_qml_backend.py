import unittest

from check_focus_qml_backend import check_qml
from check_schemas import ROOT, load_json


class FocusQmlTests(unittest.TestCase):
    def test_baseline_and_whitespace(self):
        expected = load_json(ROOT / "conformance/ir/focus-ir-expected.json")
        source = (ROOT / "conformance/quickshell/focus-baseline.qml").read_text(encoding="utf-8")
        check_qml(source, expected)
        check_qml("\n\n" + source.replace("\n", "\n    "), expected)
        with self.assertRaises(ValueError):
            check_qml(source.replace("\n", " "), expected)

    def test_rejects_geometry_pigment_semantics_and_extra_behavior(self):
        expected = load_json(ROOT / "conformance/ir/focus-ir-expected.json")
        source = (ROOT / "conformance/quickshell/focus-baseline.qml").read_text(encoding="utf-8")
        for old, new in (
            ("paintOriginX: -4", "paintOriginX: 0"),
            ("implicitHeight: 22", "implicitHeight: 20"),
            ("Qt.rgba(1, 1, 1, 1)", "Qt.rgba(1, 1, 1, 0.5)"),
            ("OddEvenFill", "WindingFill"),
            ("radiusX: 4", "radiusX: 3"),
            ("Clockwise", "Counterclockwise"),
            ("useLargeArc: false", "useLargeArc: true"),
            ("strokeWidth: 0", "strokeWidth: 1"),
            ("Accessible.ignored: true", "Accessible.ignored: false"),
            ("focus: false", "focus: true"),
            ("fillMode: Shape.NoResize", "fillMode: Shape.Stretch"),
            ("implicitWidth: 28", "implicitWidth: NaN"),
            ("import QtQuick", 'import QtQuick\nComponent.onCompleted: console.log("injected")'),
        ):
            with self.subTest(change=new):
                self.assertIn(old, source)
                with self.assertRaises(ValueError):
                    check_qml(source.replace(old, new, 1), expected)


if __name__ == "__main__":
    unittest.main()
