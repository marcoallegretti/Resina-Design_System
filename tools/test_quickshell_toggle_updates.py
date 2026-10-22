import copy
from contextlib import redirect_stderr
import io
import json
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch
from PIL import Image

from check_quickshell_command_updates import ROOT, read_reports
from check_quickshell_toggle_updates import (
    authored_cases, contrast, inline_color, main, static_authored_states,
    verify_capture, verify_case, verify_part_paint, verify_snapshot,
)


FOCUSED = dict(activeFocus=True, windowActive=True, visible=True, enabled=True, windowVisible=True)


class ToggleUpdateEvidenceTests(unittest.TestCase):
    def test_focus_matrix_cases_force_their_named_contrast_outcome_in_order(self):
        cases = authored_cases()
        names = [case["name"] for case in cases]

        def ratios(name):
            case = cases[names.index(name)]
            theme = json.loads(case["request"]["content"]["themeSource"])
            surrounding = case["request"]["background"]["components"]
            return [contrast(inline_color(theme, role, "opaqueColorAssignments")["components"], surrounding)
                    for role in ("focus", "outline.strong")]

        for theme_name in ("light", "dark"):
            with self.subTest(theme=theme_name):
                baseline = cases[names.index(theme_name + "-ltr-off-focused")]
                for suffix in ("-focus-fallback", "-focus-contrast", "-focus-recovery"):
                    self.assertEqual(names.count(theme_name + suffix), 1, "native focus guard case is missing")
                    self.assertIs(cases[names.index(theme_name + suffix)]["focused"], True)
                focus, outline = ratios(theme_name + "-focus-fallback")
                self.assertLess(focus, 3)
                self.assertGreaterEqual(outline, 3)
                self.assertTrue(all(ratio < 3 for ratio in ratios(theme_name + "-focus-contrast")))
                rejected = cases[names.index(theme_name + "-focus-contrast")]
                self.assertIs(rejected["reject"], True)
                self.assertEqual(rejected["expectedError"], "focus indicator contrast is insufficient")
                self.assertGreaterEqual(ratios(theme_name + "-focus-recovery")[0], 3)
                self.assertEqual(cases[names.index(theme_name + "-focus-recovery")]["request"], baseline["request"])
                fallback = names.index(theme_name + "-focus-fallback")
                self.assertEqual(names[fallback + 1:fallback + 3],
                                 [theme_name + "-focus-contrast", theme_name + "-focus-recovery"])

    def test_focus_contrast_case_keeps_edge_readability_on_the_authored_strong_outline(self):
        cases = authored_cases()
        for theme_name in ("light", "dark"):
            with self.subTest(theme=theme_name):
                baseline = json.loads(next(case for case in cases if case["name"] == theme_name + "-ltr-off-focused")
                                      ["request"]["content"]["themeSource"])
                theme = json.loads(next(case for case in cases if case["name"] == theme_name + "-focus-contrast")
                                   ["request"]["content"]["themeSource"])
                self.assertEqual(inline_color(theme, "outline", "opaqueColorAssignments"),
                                 inline_color(baseline, "outline.strong", "opaqueColorAssignments"))

    def test_focused_cases_retain_their_baseline_requests_and_selection(self):
        cases = authored_cases()
        for name in ("light-ltr-off", "light-ltr-on", "dark-ltr-off", "dark-rtl-on"):
            with self.subTest(name=name):
                baseline = next(case for case in cases if case["name"] == name)
                focused = [case for case in cases if case["name"] == name + "-focused"]
                self.assertEqual(len(focused), 1, "native focused Toggle case is missing")
                self.assertIs(focused[0]["focused"], True)
                self.assertEqual(focused[0]["request"], baseline["request"])
                self.assertEqual(focused[0]["checked"], baseline["checked"])
        self.assertEqual({case.get("checked", False) for case in cases if case.get("focused")}, {False, True})

    def test_static_authored_projection_matches_public_hold_free_vectors(self):
        vectors = json.loads((ROOT / "conformance/interaction/toggle-states-cases.json").read_text())
        checked = 0
        for vector in vectors:
            request = vector["request"]
            if "expected" not in vector or request["activation"]["hold"] is not None:
                continue
            checked += 1
            with self.subTest(name=vector["name"]):
                self.assertEqual(static_authored_states(request["activation"], request["hovered"], request["checked"]),
                                 vector["expected"])
        self.assertGreater(checked, 0)
        with self.assertRaisesRegex(ValueError, "unexpected hold"):
            static_authored_states({"enabled": True, "focused": True, "hold": {"kind": "key", "key": "space"}},
                                   False, False)

    def report(self, case, focused):
        state = {"schemaVersion": "0.1.0", "enabled": True, "focused": focused, "hold": None}
        native = FOCUSED if focused else {**FOCUSED, "activeFocus": False}
        snapshot = {"label": {"text": case["request"]["content"]["text"]},
                    "hitRegion": {"bounds": {"x": -20, "y": -20, "width": 380, "height": 300}}}
        return {**self.rejected(), "name": case["name"], "ready": True, "error": "", "ignored": False,
                "checked": case.get("checked", False), "state": state, "nativeFocused": focused,
                "nativeFocus": dict(native), "snapshot": json.dumps(snapshot),
                "target": json.dumps(snapshot["hitRegion"]), "measurements": [],
                "publications": [{"configuration": json.dumps(case["request"]), "snapshot": copy.deepcopy(snapshot),
                                  "state": dict(state), "checked": case.get("checked", False), "hovered": False,
                                  "nativeFocused": focused, "nativeFocus": dict(native)}]}

    def verify_unpainted(self, case, report):
        with patch("check_quickshell_toggle_updates.verify_snapshot") as snapshot, \
                patch("check_quickshell_toggle_updates.verify_layout"), \
                patch("check_quickshell_toggle_updates.verify_typography"):
            verify_case(case, report, 1)
        return snapshot

    def test_native_focus_without_committed_focus_is_rejected_at_final_and_callback(self):
        case = next(case for case in authored_cases() if case["name"] == "light-ltr-off-focused")
        self.verify_unpainted(case, self.report(case, True))
        unfocused = {"schemaVersion": "0.1.0", "enabled": True, "focused": False, "hold": None}
        for member, reported, error in (
                ("final", True, "native focus observation and committed state disagree"),
                ("callback", True, "consumer callback disagrees with native focus observation"),
                ("final", False, "does not match callback-time native values"),
                ("callback", False, "does not match callback-time native values")):
            report = self.report(case, True)
            observation = report if member == "final" else report["publications"][0]
            observation.update(state=dict(unfocused), nativeFocus=dict(FOCUSED), nativeFocused=reported)
            with self.subTest(member=member, reported=reported), self.assertRaisesRegex(ValueError, error):
                self.verify_unpainted(case, report)

    def test_each_callback_snapshot_is_checked_against_its_own_observed_state(self):
        case = next(case for case in authored_cases() if case["name"] == "dark-rtl-on-focused")
        report = self.report(case, True)
        report["publications"].insert(0, copy.deepcopy(report["publications"][0]))
        report["publications"][0].update(state={"schemaVersion": "0.1.0", "enabled": True, "focused": False,
                                                "hold": None},
                                         nativeFocus={**FOCUSED, "windowActive": False}, nativeFocused=False)
        snapshot = self.verify_unpainted(case, report)
        requests = [call.args[1] for call in snapshot.call_args_list]
        self.assertEqual([request["activation"]["focused"] for request in requests], [True, False, True])
        self.assertTrue(all(request["checked"] is True for request in requests))
        for change, error in ((("checked", False), "different checked owner"),
                              (("checked", 1), "not Boolean"),
                              (("configuration", json.dumps(authored_cases()[0]["request"])), "mixed authored")):
            changed = copy.deepcopy(report)
            changed["publications"][1][change[0]] = change[1]
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, error):
                self.verify_unpainted(case, changed)
        changed = copy.deepcopy(report)
        changed["checked"] = False
        with self.assertRaisesRegex(ValueError, "checked owner differs"):
            self.verify_unpainted(case, changed)

    def test_snapshot_semantics_follow_the_current_focus_and_selection(self):
        request = copy.deepcopy(next(case for case in authored_cases() if case["name"] == "light-ltr-on")["request"])
        activation = {"schemaVersion": "0.1.0", "enabled": True, "focused": True, "hold": None}
        request.update(activation=activation, hovered=False, checked=True, focusable=True)
        theme = json.loads(request["content"]["themeSource"])
        foreground = inline_color(theme, request["labelForegroundRole"])
        schema = json.loads((ROOT / "schemas/toggle-accessibility-ir.schema.json").read_text())
        accessibility = {"schemaVersion": "0.1.0", "role": schema["properties"]["role"]["const"],
                         "name": request["content"]["text"],
                         "description": request["description"], "value": None,
                         "state": {"enabled": True, "focused": True, "checked": True},
                         "actions": [{"kind": "invoke", "available": True}], "focusable": True, "relationships": []}
        snapshot = {"label": {"text": request["content"]["text"], "layoutDirection": "ltr"},
                    "labelOrigin": request["labelOrigin"], "labelForeground": foreground,
                    "labelBackground": request["background"],
                    "labelContrastRatio": contrast(foreground["components"], request["background"]["components"]),
                    "layout": {"checked": True}, "track": {}, "thumb": {}, "hitRegion": {},
                    "accessibility": accessibility, "thumbContrastRatio": request["minimumThumbContrast"]}
        with patch("check_quickshell_toggle_updates.validator_for"), \
                patch("check_quickshell_toggle_updates.verify_part_paint") as paint:
            verify_snapshot(snapshot, request)
            self.assertEqual([call.args[1] for call in paint.call_args_list], ["track", "thumb"])
            self.assertEqual(paint.call_args.args[3]["states"], ["rest", "focused", "checked"])
            for path, value in ((("state", "focused"), False), (("state", "checked"), False),
                                (("actions",), []), (("name",), "Other")):
                changed = copy.deepcopy(snapshot)
                target = changed["accessibility"]
                for member in path[:-1]:
                    target = target[member]
                target[path[-1]] = value
                with self.subTest(path=path), self.assertRaisesRegex(ValueError, "semantic dimensions"):
                    verify_snapshot(changed, request)
            for member, value in (("layout", {"checked": False}), ("labelForeground", request["background"]),
                                  ("thumbContrastRatio", request["minimumThumbContrast"] - 0.01)):
                with self.subTest(member=member), self.assertRaises(ValueError):
                    verify_snapshot({**snapshot, member: value}, request)

    def capture_snapshot(self):
        color = lambda *components: {"colorSpace": "srgb", "components": list(components), "alpha": 1}
        thumb = {"x": 40, "y": 28, "width": 24, "height": 24}
        return {"hitRegion": {"bounds": {"x": -20, "y": -20, "width": 380, "height": 300}},
                "layout": {"checked": False, "thumbBounds": thumb, "offThumbBounds": thumb,
                           "onThumbBounds": {**thumb, "x": 64}},
                "track": {"paint": {"body": {"face": {"pigment": {"body": color(0.2, 0.4, 0.6)}},
                                             "geometry": {"silhouette": {"bounds": {
                                                 "x": 0, "y": 0, "width": 128, "height": 80}}}},
                                    "focus": {"indicator": {"color": color(0, 0, 1), "gap": 2, "strokeWidth": 2}}}},
                "thumb": {"paint": {"body": {"face": {"pigment": {"body": color(0.8, 0.2, 0.2)}}}}},
                "labelOrigin": {"x": 160, "y": 0}, "labelForeground": color(0, 0, 0),
                "label": {"labelBounds": {"x": 8, "y": 8, "width": 100, "height": 14 * 1.34},
                          "typography": {"fontSize": {"value": 14}, "lineHeight": 1.34}}}

    def draw(self, snapshot, omit=None):
        image = Image.new("RGBA", (500, 800), (255, 255, 255, 255))
        to_pixel = lambda color: tuple(round(channel * 255) for channel in color["components"]) + (255,)
        origin = 70
        thumb = snapshot["layout"]["thumbBounds"]
        uncovered = snapshot["layout"]["onThumbBounds"]
        if omit != "thumb":
            image.putpixel((origin + 52, origin + 40), to_pixel(snapshot["thumb"]["paint"]["body"]["face"]["pigment"]["body"]))
        if omit != "track":
            image.putpixel((origin + 76, origin + 40), to_pixel(snapshot["track"]["paint"]["body"]["face"]["pigment"]["body"]))
        if omit != "label":
            image.putpixel((origin + 160 + 20, origin + 16), to_pixel(snapshot["labelForeground"]))
        if omit != "focus":
            ring = to_pixel(snapshot["track"]["paint"]["focus"]["indicator"]["color"])
            for x, y in ((64, -3), (64, 83), (-3, 40), (131, 40)):
                image.putpixel((origin + x, origin + y), ring)
        self.assertEqual((thumb["x"] + 12, uncovered["x"] + 12), (52, 76))
        return image

    def test_capture_requires_current_part_pigments_label_ink_and_focus_ring(self):
        snapshot = self.capture_snapshot()
        background = {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "capture.png"
            self.draw(snapshot).save(path)
            verify_capture(path, snapshot, 1, background)
            for omitted, error in (("thumb", "thumb pigment"), ("track", "track pigment"),
                                   ("label", "no label ink"), ("focus", "focus indicator is absent")):
                self.draw(snapshot, omitted).save(path)
                with self.subTest(omitted=omitted), self.assertRaisesRegex(ValueError, error):
                    verify_capture(path, snapshot, 1, background)
            self.draw(snapshot).save(path)
            unfocused = copy.deepcopy(snapshot)
            del unfocused["track"]["paint"]["focus"]
            verify_capture(path, unfocused, 1, background)
            with self.assertRaisesRegex(ValueError, "surroundings"):
                verify_capture(path, snapshot, 1, {**background, "components": [0, 0, 0]})

    def test_destruction_cases_unload_a_focused_toggle_with_a_pending_or_published_frame(self):
        cases = {case["name"]: case for case in authored_cases()}
        for name, mode in (("unmount-pending", "pending"), ("unmount-from-callback", "callback")):
            case = cases[name]
            with self.subTest(name=name):
                self.assertEqual((case["unmount"], case["focused"]), (mode, True))
                report = self.report(case, True)
                unloaded = {"published": mode == "callback", "prepared": True, "nativeFocused": True}
                report["unmounts"] = [unloaded]
                self.verify_unpainted(case, report)
                for unmounts in ([], [unloaded, unloaded], [{**unloaded, "published": mode != "callback"}],
                                 [{**unloaded, "prepared": False}], [{**unloaded, "nativeFocused": False}]):
                    with self.subTest(unmounts=unmounts), self.assertRaisesRegex(ValueError, "not unloaded"):
                        self.verify_unpainted(case, {**report, "unmounts": unmounts})
        other = cases["light-ltr-off-focused"]
        with self.assertRaisesRegex(ValueError, "not unloaded"):
            self.verify_unpainted(other, {**self.report(other, True), "unmounts": [unloaded]})

    def test_callbacks_carry_the_focus_changed_during_preparation(self):
        cases = {case["name"]: case for case in authored_cases()}
        for name, focused in (("focus-during-preparation", True), ("blur-during-preparation", False)):
            case = cases[name]
            with self.subTest(name=name):
                self.assertEqual(case["focusDuringPreparation"], focused)
                self.assertEqual(case.get("focused", False), focused)
                report = self.report(case, focused)
                self.verify_unpainted(case, report)
                stale = self.report(case, not focused)["publications"][0]
                report["publications"].insert(0, stale)
                with self.assertRaisesRegex(ValueError, "focus changed during preparation"):
                    self.verify_unpainted(case, report)

    def part_request(self, focused):
        request = copy.deepcopy(next(case for case in authored_cases() if case["name"] == "light-ltr-on")["request"])
        request.update(activation={"schemaVersion": "0.1.0", "enabled": True, "focused": focused, "hold": None},
                       hovered=False, checked=True, focusable=True)
        return request

    def part(self, request, name, states):
        theme = json.loads(request["content"]["themeSource"])
        config = request[name]
        surface = config["surface"]
        group, role = surface["materialRole"].split(".")
        body = {"states": states, "materialFamily": theme["materialAssignments"][group][role],
                "colorRole": config["checkedColorRole"], "materialRole": surface["materialRole"],
                "form": surface["form"], "foregroundRole": config["foregroundRole"],
                "face": {"pigment": {"body": inline_color(theme, config["checkedColorRole"])},
                         "surface": {"outline": {"size": request["layout"][name + "Size"]},
                                     "profile": config["relief"]["toggleTrack" if name == "track" else "toggleThumb"]}}}
        paint = {"body": body}
        if name == "track" and request["activation"]["focused"]:
            paint["focus"] = {"indicator": {}}
        return {"part": name, "checked": True, "phase": "rest", "response": {"bodyMix": 0, "depthScale": 1},
                "paint": paint}

    def test_part_paint_requires_the_selected_pigment_and_track_only_focus(self):
        with patch("check_quickshell_toggle_updates.surface_paint_channel_mismatch", return_value=None), \
                patch("check_quickshell_toggle_updates.body_invariants", return_value=None), \
                patch("check_quickshell_toggle_updates.verify_authored_focus") as focus:
            for focused in (False, True):
                request = self.part_request(focused)
                states = static_authored_states(request["activation"], False, True)
                theme = json.loads(request["content"]["themeSource"])
                for name in ("track", "thumb"):
                    with self.subTest(focused=focused, name=name):
                        part = self.part(request, name, states)
                        verify_part_paint(request, name, part, states)
                        stale = copy.deepcopy(part)
                        stale["paint"]["body"]["face"]["pigment"]["body"] = inline_color(
                            theme, request[name]["surface"]["colorRole"])
                        with self.assertRaisesRegex(ValueError, "paint differs"):
                            verify_part_paint(request, name, stale, states)
                        changed = copy.deepcopy(part)
                        if "focus" in changed["paint"]:
                            del changed["paint"]["focus"]
                        else:
                            changed["paint"]["focus"] = {"indicator": {}}
                        with self.assertRaisesRegex(ValueError, "track-only navigation"):
                            verify_part_paint(request, name, changed, states)
            self.assertEqual(focus.call_count, 1)
            self.assertEqual(focus.call_args.args[1], request["background"])

    def test_host_qml_runtime_errors_fail_the_run(self):
        log = "RESINA_TOGGLE_CASE " + json.dumps(self.rejected()) + "\nRESINA_TOGGLE_DONE 1\n"
        for error in ("TypeError: Property 'synchronize' of object is not a function",
                      "ReferenceError: root is not defined",
                      "QQmlVMEMetaObject: Internal error - attempted to evaluate a function in an invalid context"):
            with self.subTest(error=error), self.assertRaisesRegex(ValueError, "QML runtime error"):
                read_reports("WARN: " + error + "\n" + log, [{"name": "invalid"}], "toggle")

    def test_shared_runner_uses_the_toggle_probe_and_markers(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ("host", "font"):
                (root / name).touch()
            (root / "module").mkdir()
            argv = ["check", "--host", str(root / "host"), "--font-dir", str(root / "fonts"),
                    "--module-path", str(root / "module"), "--output-dir", str(root / "output"), "--scales", "1"]
            with patch("sys.argv", argv), redirect_stderr(io.StringIO()) as stderr, \
                    patch("conformance_font.qml_sources", return_value='["file:///fonts/Inter-Regular.otf"]') as sources, \
                    patch("check_quickshell_command_updates.subprocess.run", side_effect=[
                        SimpleNamespace(returncode=0, stdout="Quickshell 0.3.2 / Qt Quick 6.11.2\n", stderr=""),
                        SimpleNamespace(returncode=0, stdout="RESINA_TOGGLE_DONE 0\n", stderr="")]) as run:
                self.assertEqual(main(), 1)
            environment = run.call_args.kwargs["env"]
            output = root / "output/0"
            self.assertEqual((output / "config.qml").read_bytes(),
                             (ROOT / "conformance/quickshell/toggle-updates-probe.qml").read_bytes())
            self.assertEqual(environment["RESINA_TOGGLE_CASES"], str((output / "cases.json").resolve()))
            self.assertEqual(environment["RESINA_TOGGLE_FONTS"], '["file:///fonts/Inter-Regular.otf"]')
            sources.assert_called_once_with(root / "fonts")
            self.assertNotIn("RESINA_COMMAND_CASES", environment)
            self.assertEqual(run.call_args.kwargs["timeout"], 10 * len(authored_cases()))
            self.assertIn("FAIL Quickshell Toggle updates: native toggle omitted", stderr.getvalue())

    def rejected(self):
        return {"name": "invalid", "ready": False, "error": "invalid configuration",
                "snapshot": "", "target": "", "state": {
                    "schemaVersion": "0.1.0", "enabled": True, "focused": False, "hold": None},
                "checked": False, "hovered": False, "nativeFocused": False, "ignored": True, "deviceScale": 1,
                "renderer": "software", "publications": [], "unmounts": [], "measurements": [],
                "nativeFocus": {**FOCUSED, "activeFocus": False}}

    def test_complete_rejected_replacement_is_reported_with_toggle_markers(self):
        report = self.rejected()
        cases = [{"name": "invalid", "reject": True}]
        log = "RESINA_TOGGLE_CASE " + json.dumps(report) + "\nRESINA_TOGGLE_DONE 1\n"
        self.assertEqual(read_reports(log, cases, "toggle"), [report])
        verify_case(cases[0], report, 1)
        for changed in (log.replace("TOGGLE", "COMMAND"), log + "RESINA_TOGGLE_FAIL failed\n"):
            with self.subTest(log=changed), self.assertRaises(ValueError):
                read_reports(changed, cases, "toggle")
        with self.assertRaisesRegex(ValueError, "different failure cause"):
            verify_case({**cases[0], "expectedError": "matching native translation"}, report, 1)

    def test_failed_replacement_cannot_keep_any_published_channel(self):
        for field, value in (("error", ""), ("ready", True), ("snapshot", "{}"), ("target", "{}"),
                             ("ignored", False), ("publications", [{}]), ("measurements", [{}]),
                             ("checked", True), ("checked", 0), ("deviceScale", 1.25), ("renderer", "unsupported")):
            report = copy.deepcopy(self.rejected())
            report[field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                verify_case({"name": "invalid", "reject": True}, report, 1)


if __name__ == "__main__":
    unittest.main()
