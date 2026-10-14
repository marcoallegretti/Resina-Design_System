import copy
from contextlib import redirect_stderr
import io
import json
import math
import subprocess
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch
from PIL import Image

from check_quickshell_command_updates import (
    authored_cases, main, read_reports, verify_capture, verify_case,
    verify_stable_targets, verify_typography, verify_version,
    verify_authored_paint,
)


class CommandUpdateEvidenceTests(unittest.TestCase):
    def test_authored_depth_reaches_light_and_extrusion_bounds(self):
        from check_quickshell_command_updates import verify_authored_geometry
        request = authored_cases()[0]["request"]

        def snapshot(scale):
            offset = math.sqrt(2) * scale
            return {"paint": {"response": {"depthScale": scale}, "paint": {"body": {
                "materialFamily": "elastomer", "lighting": {
                    "direction": {"x": -math.sqrt(0.5), "y": -math.sqrt(0.5)},
                    "sideOffset": {"x": offset, "y": offset}},
                "geometry": {
                    "front": {"bounds": {"x": 0, "y": 0, "width": 160, "height": 80}},
                    "silhouette": {"bounds": {"x": 0, "y": 0, "width": 160 + offset, "height": 80 + offset}},
                    "edgeInterior": {"offset": {"x": 1, "y": 1}, "contour": {"bounds": {
                        "x": 0, "y": 0, "width": 158 + offset, "height": 78 + offset}}}}}}}}

        rest, disabled = snapshot(1), snapshot(0.7)
        for direction in ("ltr", "rtl"):
            request["content"]["environment"]["layoutDirection"] = direction
            verify_authored_geometry(request, rest)
            verify_authored_geometry(request, disabled)
        for change in ("geometry", "lighting", "both", "origin", "edge-placement", "edge-bounds"):
            changed = copy.deepcopy(disabled)
            body = changed["paint"]["paint"]["body"]
            if change in ("geometry", "both"):
                body["geometry"] = copy.deepcopy(rest["paint"]["paint"]["body"]["geometry"])
            if change in ("lighting", "both"):
                body["lighting"] = copy.deepcopy(rest["paint"]["paint"]["body"]["lighting"])
            if change == "origin":
                body["geometry"]["silhouette"]["bounds"]["x"] = 1
            elif change == "edge-placement":
                body["geometry"]["edgeInterior"]["offset"]["x"] = 0
            elif change == "edge-bounds":
                body["geometry"]["edgeInterior"]["contour"]["bounds"]["width"] += 1
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, "(geometry|lighting) differs from the authored depth response"):
                verify_authored_geometry(request, changed)

    def test_native_timeout_retains_partial_streams_and_failure(self):
        for stdout, stderr in ((b"partial observation\xff\n", b"native error\n"),
                               ("partial observation\n", "native error\n"), (None, None)):
            with self.subTest(stdout=stdout), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                for name in ("host", "font"):
                    (root / name).touch()
                (root / "module").mkdir()
                argv = ["check", "--host", str(root / "host"), "--font", str(root / "font"),
                        "--alternate-font", str(root / "font"), "--module-path", str(root / "module"),
                        "--output-dir", str(root / "output")]
                timeout = subprocess.TimeoutExpired(["host"], 350, output=stdout, stderr=stderr)
                with patch("sys.argv", argv), redirect_stderr(io.StringIO()), \
                        patch("check_quickshell_command_updates.subprocess.run", side_effect=[
                            SimpleNamespace(returncode=0, stdout="Quickshell 0.3.2 / Qt Quick 6.11.2\n", stderr=""),
                            timeout]) as run:
                    self.assertEqual(main(), 1)
                self.assertEqual(run.call_args.kwargs["timeout"], 10 * len(authored_cases()))
                log = (root / "output/0/run.log").read_text(encoding="utf-8")
                if stdout is None:
                    self.assertEqual(log, "")
                else:
                    self.assertIn("partial observation", log)
                    self.assertIn("native error", log)

    def test_callbacks_retain_independently_checked_layout_and_target(self):
        case = next(case for case in authored_cases() if case["name"] == "authored-layout-ltr")
        content = case["request"]["content"]
        style = case["typography"]
        label = {"schemaVersion": "0.1.0", "text": content["text"], "typography": style,
                 "layoutDirection": "ltr", "size": {"width": 200, "height": 100},
                 "labelBounds": {"x": 40, "y": 51, "width": 136, "height": 8}}
        state = {"schemaVersion": "0.1.0", "enabled": True, "focused": False, "hold": None}
        snapshot = {"label": label, "hitRegion": {"bounds": {"x": 0, "y": 0, "width": 220, "height": 120}},
                    "paint": {"geometry": {"front": {"bounds": {"width": 200, "height": 100}}}}}
        measurement = {"text": content["text"], "family": content["fontFamily"]["name"],
                       "pixelSize": 16, "weight": 700, "spacing": 0, "lineHeight": 1.48}
        report = {**self.rejected(), "name": case["name"], "ready": True, "error": "", "ignored": False,
                  "snapshot": json.dumps(snapshot), "target": json.dumps(snapshot["hitRegion"]),
                  "measurements": [{"request": {**measurement, "maximumWidth": maximum},
                                    "result": {"kind": "extent", "width": 100, "height": 8}}
                                   for maximum in (None, 136)],
                  "publications": [{"configuration": json.dumps(case["request"]),
                                    "snapshot": copy.deepcopy(snapshot), "state": state, "hovered": False}]}
        with patch("check_quickshell_command_updates.verify_snapshot") as validate_snapshot, \
                patch("check_quickshell_command_updates.verify_authored_paint"):
            verify_case(case, report, 1)
            report["publications"][0]["hovered"] = True
            report["publications"][0]["snapshot"]["paint"]["phase"] = "hover"
            verify_case(case, report, 1)
            self.assertEqual(validate_snapshot.call_args.args[2]["states"], ["hover"])
            invalid_final = copy.deepcopy(report)
            invalid_layout = json.loads(invalid_final["snapshot"])
            invalid_layout["label"]["labelBounds"]["x"] = 0
            invalid_final["snapshot"] = json.dumps(invalid_layout)
            with self.assertRaisesRegex(ValueError, "complete Rust layout equations"):
                verify_case(case, invalid_final, 1)
            for change in ("offset", "size", "target"):
                changed = copy.deepcopy(report)
                callback = changed["publications"][0]["snapshot"]
                if change == "offset":
                    callback["label"]["labelBounds"]["x"] = 0
                elif change == "size":
                    callback["label"]["size"]["width"] += 1
                    callback["paint"]["geometry"]["front"]["bounds"]["width"] += 1
                else:
                    callback["hitRegion"]["bounds"]["width"] += 1
                with self.subTest(change=change), self.assertRaisesRegex(ValueError, "callback.*layout|callback.*target"):
                    verify_case(case, changed, 1)

    def test_old_theme_paint_cannot_be_relabelled_as_the_new_source(self):
        case = authored_cases()[0]
        request = case["request"]
        snapshot = {"paint": {"phase": "rest", "response": {"bodyMix": 0, "depthScale": 1},
            "paint": {"body": {"materialFamily": "elastomer", "materialRole": "control.interactive",
                "colorRole": "surface.high", "foregroundRole": "content.primary",
                "form": {"schemaVersion": "0.1.0", "shape": "rounded", "elevation": "raised"},
                "pigment": {"body": {"colorSpace": "srgb", "alpha": 1,
                    "components": [252/255, 251/255, 248/255]}},
                "foreground": {"colorSpace": "srgb", "alpha": 1, "components": [24/255, 41/255, 45/255]},
                "edge": {"color": {"colorSpace": "srgb", "alpha": 1, "components": [109/255, 129/255, 128/255]}}}}}}
        with patch("check_quickshell_command_updates.verify_authored_geometry"):
            verify_authored_paint(request, snapshot)
            request["content"]["themeSource"] = authored_cases()[12]["request"]["content"]["themeSource"]
            with self.assertRaisesRegex(ValueError, "authored"):
                verify_authored_paint(request, snapshot)

    def test_capture_rejects_missing_shifted_or_incomplete_line_ink(self):
        snapshot = {"label": {"labelBounds": {"x": 10, "y": 10, "width": 50, "height": 20},
                              "size": {"width": 80, "height": 60},
                              "typography": {"fontSize": {"value": 10}, "lineHeight": 1}},
                    "hitRegion": {"bounds": {"x": 0, "y": 0}},
                    "paint": {"paint": {"body": {
                        "geometry": {"content": {"offset": {"x": 3, "y": 3}}},
                        "pigment": {"body": {"colorSpace": "srgb", "alpha": 1, "components": [1, 1, 1]}},
                        "foreground": {
                        "colorSpace": "srgb", "alpha": 1, "components": [0, 0, 0]}}}}}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "capture.png"
            for points in ([], [(65, 65)], [(65, 65), (65, 75)], [(5, 5), (5, 15)]):
                image = Image.new("RGBA", (500, 800), "white")
                for point in points:
                    image.putpixel(point, (0, 0, 0, 255))
                image.save(path)
                if points == [(65, 65), (65, 75)]:
                    verify_capture(path, snapshot, 1)
                else:
                    with self.assertRaises(ValueError):
                        verify_capture(path, snapshot, 1)
            Image.new("RGBA", (499, 800), "white").save(path)
            with self.assertRaises(ValueError):
                verify_capture(path, snapshot, 1)
            image = Image.new("RGBA", (500, 800), "white")
            for point in ((65, 65), (65, 75)):
                image.putpixel(point, (0, 0, 0, 255))
            image.putpixel((90, 56), (255, 0, 0, 255))
            image.save(path)
            with self.assertRaisesRegex(ValueError, "body pigment"):
                verify_capture(path, snapshot, 1)

    def test_host_execution_failure_is_distinct_from_unsupported_version(self):
        with self.assertRaisesRegex(ValueError, "verification failed:.*timed out"):
            verify_version(SimpleNamespace(returncode=1, stdout="", stderr="traceback\ntimed out after 10 seconds"))
        with self.assertRaisesRegex(ValueError, "requires released Quickshell"):
            verify_version(SimpleNamespace(returncode=0, stdout="Quickshell 0.3.1 / Qt Quick 6.11.2", stderr=""))
        verify_version(SimpleNamespace(returncode=0, stdout="Quickshell 0.3.2 / Qt Quick 6.11.2\n", stderr=""))

    def rejected(self):
        return {"name": "invalid", "ready": False, "error": "invalid configuration",
                "snapshot": "", "target": "", "state": {
                    "schemaVersion": "0.1.0", "enabled": True, "focused": False, "hold": None},
                "hovered": False, "ignored": True, "deviceScale": 1, "renderer": "software",
                "publications": [], "measurements": []}

    def test_complete_rejected_replacement_is_reported(self):
        report = self.rejected()
        cases = [{"name": "invalid", "reject": True}]
        log = "RESINA_COMMAND_CASE " + json.dumps(report) + "\nRESINA_COMMAND_DONE 1\n"
        self.assertEqual(read_reports(log, cases), [report])
        verify_case(cases[0], report, 1)
        with self.assertRaisesRegex(ValueError, "different failure cause"):
            verify_case({**cases[0], "expectedError": "matching native translation"}, report, 1)

    def test_missing_duplicate_reordered_or_failed_native_cases_are_rejected(self):
        report = json.dumps(self.rejected())
        line = "RESINA_COMMAND_CASE " + report + "\n"
        valid = line + "RESINA_COMMAND_DONE 1\n"
        for log in ("", line, "RESINA_COMMAND_DONE 1\n", line + valid,
                    valid + "RESINA_COMMAND_DONE 1\n", valid.replace("DONE 1", "DONE 0"),
                    valid.replace('"invalid"', '"other"'), valid + "RESINA_COMMAND_FAIL failed\n",
                    valid.replace('"ready": false', '"ready": false, "ready": false')):
            with self.subTest(log=log), self.assertRaises(ValueError):
                read_reports(log, [{"name": "invalid"}])

    def test_font_diagnostics_are_retained_without_claiming_pixel_failure(self):
        report = self.rejected()
        log = "render glyph failed err=62\nRESINA_COMMAND_CASE " + json.dumps(report) + "\nRESINA_COMMAND_DONE 1\n"
        self.assertEqual(read_reports(log, [{"name": "invalid"}]), [report])

    def test_failed_replacement_cannot_keep_any_published_channel(self):
        for field, value in (("error", ""), ("ready", True), ("snapshot", "{}"),
                             ("target", "{}"), ("ignored", False), ("publications", [{}]), ("measurements", [{}]),
                             ("deviceScale", 1.25), ("deviceScale", True), ("renderer", "unsupported"), ("ready", 0)):
            report = copy.deepcopy(self.rejected())
            report[field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                verify_case({"name": "invalid", "reject": True}, report, 1)

    def test_authored_typography_detects_stale_size_weight_spacing_and_line_height(self):
        cases = authored_cases()
        for case in (cases[0], next(case for case in cases if case["name"] == "authored-layout-rtl")):
            scale = case["request"]["content"]["environment"]["textScale"]
            style = copy.deepcopy(case.get("typography", {"familyRole": "sans",
                "fontSize": {"value": 14 * scale, "unit": "px"}, "fontWeight": 500,
                "lineHeight": 1.34, "letterSpacing": {"value": 0.15 * scale, "unit": "px"}}))
            verify_typography(case, {"typography": style})
            for field in style:
                stale = copy.deepcopy(style)
                stale[field] = None
                with self.subTest(case=case["name"], field=field), self.assertRaises(ValueError):
                    verify_typography(case, {"typography": stale})

    def test_requested_platform_applies_to_host_verification(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ("host", "font"):
                (root / name).touch()
            (root / "module").mkdir()
            argv = ["check", "--host", str(root / "host"), "--font", str(root / "font"),
                    "--alternate-font", str(root / "font"),
                    "--module-path", str(root / "module"), "--output-dir", str(root / "output")]
            with patch("sys.argv", argv), patch.dict("os.environ", {"QT_QPA_PLATFORM": "wayland"}), \
                    patch("check_quickshell_command_updates.subprocess.run", return_value=SimpleNamespace(
                        returncode=1, stdout="", stderr="unavailable host")) as run, redirect_stderr(io.StringIO()):
                self.assertEqual(main(), 1)
            self.assertEqual(run.call_args.kwargs["env"]["QT_QPA_PLATFORM"], "offscreen")
            self.assertFalse((root / "output").exists())

    def test_availability_and_recovery_cannot_move_the_same_authored_target(self):
        cases = [{"request": {"content": "same"}}, {"request": {"content": "same"}, "enabled": False}]
        reports = [{"target": '{"x":0}'}, {"target": '{"x":0}'}]
        verify_stable_targets(cases, reports)
        reports[1]["target"] = '{"x":1}'
        with self.assertRaisesRegex(ValueError, "same authored target"):
            verify_stable_targets(cases, reports)


if __name__ == "__main__":
    unittest.main()
