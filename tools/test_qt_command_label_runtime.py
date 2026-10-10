import copy
import json
import unittest

from check_qt_command_label_runtime import ROOT, verify_layout


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


if __name__ == "__main__":
    unittest.main()
