import copy
import json
import unittest

from check_qt_command_label_runtime import ROOT, resolved_master, verify_layout
import conformance_font


class NativeCommandLabelEvidenceTests(unittest.TestCase):
    def fixture(self):
        label = json.loads((ROOT / "conformance/ir/command-label-ir.json").read_text())
        request = {"text": "Reconnect", "fontFamily": {"name": "Explicit Font"},
                   "minimumSize": {"width": 64, "height": 32},
                   "maximumSize": {"width": 200, "height": 160},
                   "padding": {"start": 12, "end": 8, "top": 6, "bottom": 10}}
        native = {"text": "Reconnect", "family": "Explicit Font", "pixelSize": 40,
                  "weight": 600, "spacing": 0.5, "lineHeight": 1.4}
        calls = [{"request": {**native, "maximumWidth": None},
                  "result": {"kind": "extent", "width": 360, "height": 18}},
                 {"request": {**native, "maximumWidth": 180},
                  "result": {"kind": "extent", "width": 172, "height": 54}}]
        return label, calls, request

    def test_public_layout_arithmetic_case_and_rtl_padding(self):
        label, calls, request = self.fixture()
        verify_layout(label, calls, request)
        label["layoutDirection"] = "rtl"
        label["labelBounds"]["x"] = 8
        verify_layout(label, calls, request)

    def test_incomplete_stale_typography_or_extent_evidence_fails(self):
        for change in ("missing", "extra", "natural", "offer", "height", "text", "font", "scale", "overflow"):
            label, calls, request = self.fixture()
            if change == "missing":
                calls.pop()
            elif change == "extra":
                calls.append(copy.deepcopy(calls[-1]))
            elif change == "natural":
                calls[0]["request"]["maximumWidth"] = 180
            elif change == "offer":
                calls[1]["request"]["maximumWidth"] = 181
            elif change == "height":
                label["labelBounds"]["height"] = 53
            elif change == "text":
                calls[0]["request"]["text"] = "Other command"
            elif change == "font":
                calls[1]["request"]["family"] = "Substituted Font"
            elif change == "scale":
                calls[1]["request"]["pixelSize"] = 80
            elif change == "overflow":
                calls[1]["result"]["width"] = 181
            with self.subTest(change=change), self.assertRaises(ValueError):
                verify_layout(label, calls, request)

    def test_every_call_must_resolve_the_master_of_the_requested_weight(self):
        masters = [{"family": "Inter", "style": master["style"], "weight": master["weight"],
                    "file": master["file"], "sha256": master["sha256"]}
                   for master in conformance_font.MANIFEST["masters"]]
        _, calls, _ = self.fixture()
        for call in calls:
            call["face"] = {"family": "Inter", "style": "SemiBold"}
        self.assertEqual(resolved_master(masters, calls)["file"], "Inter-SemiBold.otf")
        for change in ("regular", "mixed", "weights"):
            changed = copy.deepcopy(calls)
            if change == "regular":
                for call in changed:
                    call["face"]["style"] = "Regular"
            elif change == "mixed":
                changed[1]["face"]["style"] = "Medium"
            else:
                changed[1]["request"]["weight"] = 500
            with self.subTest(change=change), self.assertRaises(ValueError):
                resolved_master(masters, changed)


if __name__ == "__main__":
    unittest.main()
