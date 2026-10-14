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
        id: paint
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
                         ("id: paint", "id: unrelated"),
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

    def test_comment_decoys_cannot_satisfy_native_profile(self):
        for old, wrong in (("smooth: false", "smooth: true"),
                           ("mipmap: false", "mipmap: true"),
                           ("asynchronous: false", "asynchronous: true"),
                           ("focus: false", "focus: true"),
                           ("activeFocusOnTab: false", "activeFocusOnTab: true"),
                           ("Accessible.ignored: true", "Accessible.ignored: false"),
                           ("paint.status === Image.Ready", "true"),
                           ('paint.status === Image.Error ? "error" : ""', '""')):
            for comment in (f" // {old}", f" /* {old} */"):
                with self.subTest(old=old, comment=comment), self.assertRaises(ValueError):
                    self.check(self.source.replace(old, wrong + comment))

    def test_comments_are_ignored_without_corrupting_string_values(self):
        source = self.source.replace('"error"', '"error // not a comment"')
        source = source.replace("smooth: false", "smooth: false /* smooth: true */")
        source = "/*\nAccessible.ignored: false\nfocus: true\n*/\n" + source
        self.assertEqual(self.check(source), (self.png, 0))
        with self.assertRaises(ValueError):
            self.check(self.source + "/* unterminated")

    def test_properties_must_belong_to_the_painted_nodes(self):
        extra_image = self.source.replace("        smooth: false", "        opacity: 1; smooth: true")
        extra_image = extra_image.rsplit("}", 1)[0] + "    Image {\n        smooth: false\n    }\n}\n"
        misplaced_focus = self.source.replace("    focus: false", "    opacity: 1; focus: true", 1)
        misplaced_focus = misplaced_focus.replace("        width: 5", "        focus: false\n        width: 5")
        for source in (extra_image, misplaced_focus):
            with self.subTest(source=source), self.assertRaises(ValueError):
                self.check(source)


if __name__ == "__main__":
    unittest.main()
