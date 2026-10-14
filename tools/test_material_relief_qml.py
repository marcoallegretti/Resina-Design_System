import io
from types import SimpleNamespace
import unittest

from PIL import Image, PngImagePlugin

from check_material_relief_qml_backend import component_png


class MaterialReliefQmlTests(unittest.TestCase):
    def setUp(self):
        expected = Image.new("RGBA", (5, 4), (60, 120, 180, 255))
        information = PngImagePlugin.PngInfo()
        information.add(b"sRGB", b"\x00")
        stream = io.BytesIO()
        expected.save(stream, format="PNG", pnginfo=information)
        self.png = stream.getvalue()
        self.expected = [(60, 120, 180, 255)] * 20
        encoded = "".join(f"%{byte:02X}" for byte in self.png)
        self.source = f'''import QtQuick
Item {{
    readonly property real paintOriginX: 12.8
    readonly property real paintOriginY: 16.8
    readonly property real preparedDeviceScale: 1.25
    implicitWidth: 4
    implicitHeight: 3.2
    focus: false
    activeFocusOnTab: false
    Accessible.ignored: true
    readonly property bool paintReady: paint.status === Image.Ready
    readonly property string paintError: paint.status === Image.Error ? "error" : ""
    Image {{
        Accessible.ignored: true
        width: 5
        height: 4
        transform: Scale {{ xScale: 0.8; yScale: 0.8 }}
        smooth: false
        mipmap: false
        asynchronous: false
        source: "data:image/png,{encoded}"
    }}
}}
'''

    def check(self, text, expected=None):
        result = SimpleNamespace(returncode=0, stderr=b"", stdout=text.encode("utf-8"))
        return component_png(result, 5, 4, (13.3, 17.1), 1.25, self.expected if expected is None else expected)

    def test_complete_profile_and_png_pass(self):
        png, worst = self.check(self.source)
        self.assertEqual(png, self.png)
        self.assertEqual(worst, 0)

    def test_placement_transform_extent_filtering_and_semantics_damage_fail(self):
        for old, new in (("paintOriginX: 12.8", "paintOriginX: 13.3"),
                         ("preparedDeviceScale: 1.25", "preparedDeviceScale: 1"),
                         ("implicitWidth: 4", "implicitWidth: 5"),
                         ("width: 5", "width: 4"),
                         ("yScale: 0.8", "yScale: 1"),
                         ("smooth: false", "smooth: true"),
                         ("focus: false", "focus: true"),
                         ("Accessible.ignored: true", "Accessible.ignored: false"),
                         ("paint.status === Image.Ready", "true"),
                         ("%89", "%8G")):
            with self.subTest(old=old), self.assertRaises(ValueError):
                self.check(self.source.replace(old, new))
        duplicate = self.source.replace("preparedDeviceScale: 1.25", "preparedDeviceScale: 1.25\n    readonly property real preparedDeviceScale: 1.25")
        with self.assertRaises(ValueError):
            self.check(duplicate)

    def test_independent_pixel_and_alpha_damage_fail(self):
        for color in ((90, 120, 180, 255), (60, 120, 180, 0)):
            damaged = self.expected.copy()
            damaged[4] = color
            with self.subTest(color=color), self.assertRaises(ValueError):
                self.check(self.source, damaged)


if __name__ == "__main__":
    unittest.main()
