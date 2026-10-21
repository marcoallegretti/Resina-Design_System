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

from check_schemas import ROOT, load_json
from command_label_arithmetic import layout as label_layout
from command_layout_arithmetic import resolve as resolve_command_layout
from lit_body_arithmetic import PHYSICAL, Body, lit_body_source
from test_lit_body_arithmetic import SOURCE, token_value

from check_quickshell_command_updates import (
    authored_cases, capture_body_probe, main, read_reports, verify_capture, verify_case,
    verify_stable_targets, verify_typography, verify_version,
    verify_authored_paint,
)


class CommandUpdateEvidenceTests(unittest.TestCase):
    def test_native_focus_is_the_conjunction_of_raw_native_values(self):
        from check_quickshell_command_updates import verify_native_focus
        observed = dict(activeFocus=True, windowActive=True, visible=True, enabled=True, windowVisible=True)
        verify_native_focus({"nativeFocused": True, "nativeFocus": observed})
        for field in observed:
            changed = {**observed, field: False}
            with self.subTest(field=field):
                verify_native_focus({"nativeFocused": False, "nativeFocus": changed})
                with self.assertRaisesRegex(ValueError, "does not match callback-time native values"):
                    verify_native_focus({"nativeFocused": True, "nativeFocus": changed})
        for changed in ({**observed, "activeFocus": 1}, {**observed, "extra": True},
                        {key: value for key, value in observed.items() if key != "windowActive"}):
            with self.subTest(observation=changed), self.assertRaisesRegex(ValueError, "fields or types differ"):
                verify_native_focus({"nativeFocused": True, "nativeFocus": changed})

    def test_native_focus_matrix_covers_fallback_failure_and_focused_recovery(self):
        cases = authored_cases()
        for theme_name in ("light", "dark"):
            baseline = next(case for case in cases if case["name"] == theme_name + "-latin-1-focused")
            for suffix in ("-focus-fallback", "-focus-contrast", "-focus-recovery"):
                name = theme_name + suffix
                with self.subTest(name=name):
                    matches = [case for case in cases if case["name"] == name]
                    self.assertEqual(len(matches), 1, "native focus guard case is missing")
                    case = matches[0]
                    self.assertIs(case["focused"], True)
                    if suffix == "-focus-recovery":
                        self.assertEqual(case["request"], baseline["request"])
                        continue
                    theme = json.loads(case["request"]["content"]["themeSource"])
                    path = theme["colorAssignments"]["roles"]["surface.base"]
                    self.assertEqual(theme["opaqueColorAssignments"]["roles"]["focus"], path)
                    if suffix == "-focus-contrast":
                        self.assertIs(case["reject"], True)
                        self.assertEqual(theme["opaqueColorAssignments"]["roles"]["outline.strong"], path)
                        self.assertEqual(case["expectedError"], "focus indicator contrast is insufficient")
                    else:
                        self.assertNotIn("reject", case)
                        original = json.loads(baseline["request"]["content"]["themeSource"])
                        self.assertEqual(theme["opaqueColorAssignments"]["roles"].get("outline.strong"),
                                         original["opaqueColorAssignments"]["roles"].get("outline.strong"))

    def test_focus_matrix_cases_force_their_named_contrast_outcome_in_order(self):
        from check_quickshell_command_updates import contrast, inline_color
        cases = authored_cases()
        names = [case["name"] for case in cases]

        def ratios(name):
            case = cases[names.index(name)]
            theme = json.loads(case["request"]["content"]["themeSource"])
            surrounding = case["request"]["body"]["surroundingColor"]["components"]
            return [contrast(inline_color(theme, role, "opaqueColorAssignments")["components"], surrounding)
                    for role in ("focus", "outline.strong")]

        for theme_name in ("light", "dark"):
            with self.subTest(theme=theme_name):
                focus, outline = ratios(theme_name + "-focus-fallback")
                self.assertLess(focus, 3)
                self.assertGreaterEqual(outline, 3)
                self.assertTrue(all(ratio < 3 for ratio in ratios(theme_name + "-focus-contrast")))
                self.assertGreaterEqual(ratios(theme_name + "-focus-recovery")[0], 3)
                fallback = names.index(theme_name + "-focus-fallback")
                self.assertEqual(names[fallback + 1:fallback + 3],
                                 [theme_name + "-focus-contrast", theme_name + "-focus-recovery"])

    def test_focus_guard_checks_authored_pigment_and_fallback_independently(self):
        from check_quickshell_command_updates import verify_authored_focus
        theme = json.loads(authored_cases()[0]["request"]["content"]["themeSource"])
        palette = theme["tokenResolver"]["sets"]["theme"]["sources"][0].setdefault("palette", {})
        black = {"colorSpace": "srgb", "components": [0, 0, 0], "alpha": 1}
        white = {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1}
        palette.update(testSource={"$type": "color", "$value": black},
                       testFocus={"$type": "color", "$value": black}, testOutline={"$type": "color", "$value": black})
        theme["colorAssignments"]["roles"].update({"focus": "palette.testSource", "outline.strong": "palette.testSource"})
        theme["opaqueColorAssignments"]["roles"].pop("focus", None)
        theme["opaqueColorAssignments"]["roles"].pop("outline.strong", None)
        indicator = {"colorRole": "focus", "color": black, "fallbackApplied": False, "contrastRatio": 21}
        verify_authored_focus(theme, white, indicator)
        theme["opaqueColorAssignments"]["roles"].update({"focus": "palette.testFocus", "outline.strong": "palette.testOutline"})
        verify_authored_focus(theme, white, indicator)
        for field, value in (("color", white), ("contrastRatio", 3), ("contrastRatio", 21 + 1e-11),
                             ("colorRole", "outline.strong"), ("fallbackApplied", True)):
            changed = {**indicator, field: value}
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "authored pigment or contrast"):
                verify_authored_focus(theme, white, changed)
        palette["testFocus"]["$value"] = white
        fallback = {**indicator, "colorRole": "outline.strong", "fallbackApplied": True}
        verify_authored_focus(theme, white, fallback)
        with self.assertRaisesRegex(ValueError, "authored pigment or contrast"):
            verify_authored_focus(theme, white, indicator)
        palette["testOutline"]["$value"] = white
        with self.assertRaisesRegex(ValueError, "both fail"):
            verify_authored_focus(theme, white, fallback)
        verify_authored_focus(theme, black, {**indicator, "color": white})

    def test_both_published_variants_have_focused_cases_in_both_themes(self):
        cases = authored_cases()
        for name in ("light-latin-1", "dark-latin-1", "light-primary", "dark-primary"):
            with self.subTest(name=name):
                baseline = next(case for case in cases if case["name"] == name)
                focused = [case for case in cases if case["name"] == name + "-focused"]
                self.assertEqual(len(focused), 1, "native focused Command case is missing")
                self.assertIs(focused[0]["focused"], True)
                self.assertEqual(focused[0]["request"], baseline["request"])

    def test_static_authored_projection_matches_public_hold_free_vectors(self):
        from check_quickshell_command_updates import ROOT, static_authored_states
        cases = json.loads((ROOT / "conformance/interaction/command-states-cases.json").read_text())
        for case in cases:
            request = case["request"]
            if "expected" not in case or request["activation"]["hold"] is not None:
                continue
            with self.subTest(name=case["name"]):
                self.assertEqual(static_authored_states(request["activation"], request["hovered"]), case["expected"])
        for hold in ({"kind": "key", "key": "space"}, {"kind": "pointer", "id": "primary", "inside": True}):
            with self.subTest(hold=hold), self.assertRaisesRegex(ValueError, "unexpected hold"):
                static_authored_states({"enabled": True, "focused": True, "hold": hold}, False)

    def test_primary_cases_retain_the_published_anatomy_in_both_themes(self):
        from check_quickshell_command_updates import ROOT
        definition = json.loads((ROOT / "definitions/components/command.json").read_text())["variants"]["primary"]["body"]
        cases = authored_cases()
        for theme in ("light", "dark"):
            with self.subTest(theme=theme):
                matches = [case for case in cases if case["name"] == theme + "-primary"]
                self.assertEqual(len(matches), 1, "published primary Command lacks a native authored case")
                body = matches[0]["request"]["body"]
                self.assertEqual(body["surface"]["materialRole"], definition["materialRole"])
                self.assertEqual(body["surface"]["colorRole"], definition["colorRole"])
                self.assertEqual(body["surface"]["form"], {
                    "schemaVersion": "0.1.0", "shape": definition["shape"], "elevation": definition["elevation"]})
                self.assertEqual(body["foregroundRole"], definition["contentRole"])

    def test_authored_depth_reaches_light_and_extrusion_bounds(self):
        from check_quickshell_command_updates import verify_authored_geometry
        request = authored_cases()[0]["request"]

        def snapshot(scale):
            offset = math.sqrt(2) * scale
            return {"paint": {"response": {"depthScale": scale}, "paint": {"body": {
                "materialFamily": "elastomer", "lighting": {
                    "direction": {"x": -math.sqrt(0.5), "y": -math.sqrt(0.5)},
                    "sideOffset": {"x": offset, "y": offset}},
                "face": {"surface": {"outline": {"size": {"width": 160, "height": 80}}}},
                "geometry": {
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
                argv = ["check", "--host", str(root / "host"), "--font-dir", str(root / "fonts"),
                        "--alternate-font", str(root / "font"), "--module-path", str(root / "module"),
                        "--output-dir", str(root / "output")]
                timeout = subprocess.TimeoutExpired(["host"], 350, output=stdout, stderr=stderr)
                with patch("sys.argv", argv), redirect_stderr(io.StringIO()), \
                        patch("conformance_font.qml_sources", return_value='["file:///fonts/Inter-Regular.otf"]'), \
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
                                    "snapshot": copy.deepcopy(snapshot), "state": state, "hovered": False,
                                    "nativeFocused": False, "nativeFocus": self.rejected()["nativeFocus"]}]}
        with patch("check_quickshell_command_updates.verify_snapshot") as validate_snapshot, \
                patch("check_quickshell_command_updates.verify_authored_paint"):
            verify_case(case, report, 1)
            report["publications"][0]["hovered"] = True
            report["publications"][0]["snapshot"]["paint"]["phase"] = "hover"
            verify_case(case, report, 1)
            self.assertEqual(validate_snapshot.call_args.args[2]["states"], ["hover"])
            focused = {field: True for field in report["nativeFocus"]}
            for member, reported, error in (
                    ("final", True, "native focus observation and committed state disagree"),
                    ("callback", True, "consumer callback disagrees with native focus observation"),
                    ("final", False, "does not match callback-time native values"),
                    ("callback", False, "does not match callback-time native values")):
                changed = copy.deepcopy(report)
                observation = changed if member == "final" else changed["publications"][0]
                observation.update(nativeFocus=dict(focused), nativeFocused=reported)
                with self.subTest(member=member, reported=reported), self.assertRaisesRegex(ValueError, error):
                    verify_case(case, changed, 1)
            changed = copy.deepcopy(report)
            changed["publications"][0]["nativeFocused"] = 1
            with self.assertRaisesRegex(ValueError, "publication native observation is not Boolean"):
                verify_case(case, changed, 1)
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

    def test_paint_cannot_be_relabelled_as_another_theme_or_variant(self):
        case = authored_cases()[0]
        request = case["request"]
        snapshot = {"paint": {"phase": "rest", "response": {"bodyMix": 0, "depthScale": 1},
            "paint": {"body": {"materialFamily": "elastomer", "materialRole": "control.interactive",
                "colorRole": "surface.high", "foregroundRole": "content.primary",
                "form": {"schemaVersion": "0.1.0", "shape": "rounded", "elevation": "raised"},
                "face": {
                    "pigment": {"body": {"colorSpace": "srgb", "alpha": 1,
                        "components": [252/255, 251/255, 248/255]}},
                    "foreground": {"colorSpace": "srgb", "alpha": 1, "components": [24/255, 41/255, 45/255]},
                    "edge": {"color": {"colorSpace": "srgb", "alpha": 1, "components": [109/255, 129/255, 128/255]}},
                    "surface": {"profile": request["body"]["relief"]["commandBody"]}}}}}}
        with patch("check_quickshell_command_updates.verify_authored_geometry"), \
                patch("check_quickshell_command_updates.body_invariants", return_value=None):
            verify_authored_paint(request, snapshot)
            request["content"]["themeSource"] = authored_cases()[12]["request"]["content"]["themeSource"]
            with self.assertRaisesRegex(ValueError, "authored"):
                verify_authored_paint(request, snapshot)
            for primary in (case for case in authored_cases() if case["name"].endswith("-primary")):
                with self.subTest(variant=primary["name"]):
                    relabelled = copy.deepcopy(snapshot)
                    body = relabelled["paint"]["paint"]["body"]
                    body.update(primary["request"]["body"]["surface"])
                    body["foregroundRole"] = primary["request"]["body"]["foregroundRole"]
                    with self.assertRaisesRegex(ValueError, "authored"):
                        verify_authored_paint(primary["request"], relabelled)

    def test_capture_rejects_missing_shifted_or_incomplete_line_ink(self):
        snapshot = {"label": {"labelBounds": {"x": 10, "y": 10, "width": 50, "height": 20},
                              "size": {"width": 80, "height": 60},
                              "typography": {"fontSize": {"value": 10}, "lineHeight": 1}},
                    "hitRegion": {"bounds": {"x": 0, "y": 0}},
                    "paint": {"paint": {"body": {"geometry": {}, "face": {
                        "edgeWidth": 1,
                        "surface": {"outline": {"size": {"width": 80, "height": 60}, "corners": {
                            name: {"x": 0, "y": 0} for name in ("topLeft", "topRight", "bottomRight", "bottomLeft")}},
                            "profile": {"kind": "convex", "shoulderWidth": 2}},
                        "pigment": {"body": {"colorSpace": "srgb", "alpha": 1, "components": [1, 1, 1]}},
                        "foreground": {
                        "colorSpace": "srgb", "alpha": 1, "components": [0, 0, 0]}}}}}}
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
            crowded = copy.deepcopy(snapshot)
            crowded["label"]["labelBounds"]["y"] = 2.5
            with self.assertRaisesRegex(ValueError, "no whole body pixel"):
                verify_capture(path, crowded, 1)
            snapshot["paint"]["paint"]["body"]["geometry"]["silhouette"] = {
                "bounds": {"x": 0, "y": 0, "width": 80, "height": 60}}
            snapshot["paint"]["paint"]["focus"] = {"indicator": {
                "color": {"colorSpace": "srgb", "alpha": 1, "components": [1, 0, 0]},
                "gap": 2, "strokeWidth": 2}}
            points = [(90, 47), (90, 113), (47, 80), (133, 80)]
            surrounding = {"colorSpace": "srgb", "alpha": 1, "components": [1, 1, 1]}
            for fault in ("none", "missing", "shifted", "color", "alpha", "background"):
                with self.subTest(focus=fault):
                    image = Image.new("RGBA", (500, 800), "white")
                    for point in ((65, 65), (65, 75)):
                        image.putpixel(point, (0, 0, 0, 255))
                    for x, y in points[1:] if fault == "missing" else points:
                        image.putpixel((x + (5 if fault == "shifted" else 0), y),
                                       (0, 0, 255, 255) if fault == "color" else
                                       (255, 0, 0, 128 if fault == "alpha" else 255))
                    image.save(path)
                    if fault == "background":
                        image.putpixel((0, 0), (0, 0, 0, 255))
                        image.save(path)
                    if fault == "none":
                        verify_capture(path, snapshot, 1, surrounding)
                        with self.assertRaisesRegex(ValueError, "explicit opaque"):
                            verify_capture(path, snapshot, 1)
                    else:
                        with self.assertRaisesRegex(ValueError, "focus indicator|focus surroundings"):
                            verify_capture(path, snapshot, 1, surrounding)

    def test_capture_probe_finds_a_whole_uniform_body_pixel_above_every_authored_label(self):
        foundation = load_json(ROOT / "tokens/foundation.json")
        cases = load_json(ROOT / "conformance/layout/command-layout-cases.json")
        relief = load_json(ROOT / "definitions/control-relief.json")["commandBody"]
        shape = load_json(ROOT / "definitions/tier0-shapes.json")["profiles"]["rounded"]["radius"]
        radius = token_value([foundation], shape)["value"]
        edge = load_json(ROOT / "definitions/tier0-surface-appearance.json")["bands"]["elastomer"]["edgeWidth"]
        theme = load_json(ROOT / "tokens/themes/light.json")
        trees = [foundation] + theme["tokenResolver"]["sets"]["theme"]["sources"]
        typography = theme["typographyAssignments"]["roles"]["label"]
        font_size = token_value(trees, typography["fontSize"])["value"]
        line_height = token_value(trees, typography["lineHeight"])
        checked = failures = 0
        for pointer in ("fine", "coarse"):
            layout_request = next(case for case in cases if case["name"] == f"compact text 1 {pointer} ltr")["request"]
            for density in ("compact", "standard", "comfortable", "immersive"):
                for text_scale in (1, 1.0006, 1.5, 2, 3):
                    for lines in (1, 2, 3):
                        for direction in ("ltr", "rtl"):
                            request = copy.deepcopy(layout_request)
                            request["environment"].update(densityPreference=density, textScale=text_scale,
                                                          layoutDirection=direction)
                            policy = resolve_command_layout(request)
                            size = font_size * text_scale
                            measured = {"width": size * 4, "height": lines * size * line_height}
                            resolved = label_layout({
                                "padding": policy["padding"], "minimumSize": policy["minimumSize"],
                                "maximumSize": policy["maximumSize"], "natural": measured, "fitted": measured,
                                "direction": direction})
                            body = Body(lit_body_source(SOURCE, resolved["size"], radius, relief, edge, direction))
                            visual = {"x": min(0, body.offset[0]), "y": min(0, body.offset[1]),
                                      "width": body.width + abs(body.offset[0]),
                                      "height": body.height + abs(body.offset[1])}
                            target = {}
                            for axis, extent, minimum in (("x", "width", policy["targetMinimum"]["width"]),
                                                          ("y", "height", policy["targetMinimum"]["height"])):
                                target[axis] = visual[axis] - (max(visual[extent], minimum) - visual[extent]) / 2
                            snapshot = {
                                "label": {"labelBounds": resolved["labelBounds"], "size": resolved["size"]},
                                "hitRegion": {"bounds": target},
                                "paint": {"paint": {"body": {"face": {
                                    "edgeWidth": edge,
                                    "surface": {"profile": relief, "outline": {
                                        "size": resolved["size"],
                                        "corners": {name: {"x": value, "y": value}
                                                    for name, value in zip(PHYSICAL, body.radii)}}}}}}}}
                            for scale in (1, 1.25, 2):
                                with self.subTest(pointer=pointer, density=density, text_scale=text_scale,
                                                  lines=lines, direction=direction, scale=scale):
                                    checked += 1
                                    try:
                                        column, row = capture_body_probe(snapshot, scale)
                                    except ValueError:
                                        failures += 1
                                        raise
                                    top = 50 - target["y"] + resolved["labelBounds"]["y"]
                                    self.assertLessEqual((row + 1) / scale, top + 1e-9)
                                    self.assertGreater(row / scale, 50 - target["y"])
                                    self.assertTrue(0 <= column < 500 * scale)
        self.assertEqual((checked, failures), (2 * 4 * 5 * 3 * 2 * 3, 0))

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
                "hovered": False, "nativeFocused": False, "ignored": True, "deviceScale": 1, "renderer": "software",
                "publications": [], "measurements": [],
                "nativeFocus": dict(activeFocus=False, windowActive=True, visible=True,
                                    enabled=True, windowVisible=True)}

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
            argv = ["check", "--host", str(root / "host"), "--font-dir", str(root / "fonts"),
                    "--alternate-font", str(root / "font"),
                    "--module-path", str(root / "module"), "--output-dir", str(root / "output")]
            with patch("sys.argv", argv), patch.dict("os.environ", {"QT_QPA_PLATFORM": "wayland"}), \
                    patch("conformance_font.qml_sources", return_value='["file:///fonts/Inter-Regular.otf"]'), \
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
