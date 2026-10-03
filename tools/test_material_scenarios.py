import copy
import io
import subprocess
import unittest
from unittest.mock import Mock, patch

from check_material_scenarios import check_scene
from check_schemas import ROOT, load_json
from check_surface_paint_backend import baseline, expected_result
from material_scenarios import MANIFEST, asset_source, backend_document, main, prepare


class MaterialSceneTests(unittest.TestCase):
    def setUp(self):
        self.manifest = load_json(MANIFEST)
        self.resolution = load_json(ROOT / "conformance/headless/expected-resolution.json")
        self.backend = Mock(side_effect=lambda _: copy.deepcopy(self.resolution))

    def test_preparation_preserves_all_explicit_inputs_and_isolates_requests(self):
        before = copy.deepcopy(self.manifest)
        bundle = prepare(self.manifest, self.backend)
        self.assertEqual(self.backend.call_count, 2)
        self.assertEqual(len(bundle["scenarios"]), 32)
        self.assertEqual(self.manifest, before)
        self.assertEqual(bundle["capture"], before["capture"])
        for authored, prepared in zip(before["scenarios"], bundle["scenarios"], strict=True):
            request = prepared["request"]
            color = self.resolution["opaqueColorFallbacks"][authored["surroundingColorRole"]]
            if authored["kind"] == "surfacePaint":
                self.assertEqual(request["surroundingColor"], color)
                self.assertEqual(set(request), {"schemaVersion", "body", "surroundingColor"})
                request = request["body"]
            self.assertEqual(request["surface"], authored["surface"])
            self.assertEqual(request["size"], before["size"])
            theme_path = ROOT / before["themes"][authored["theme"]]["source"]
            self.assertEqual(request["theme"]["themeSource"], theme_path.read_text(encoding="utf-8"))
            self.assertEqual(request["theme"]["externalSources"]["foundation.json"], (ROOT / "tokens/foundation.json").read_text(encoding="utf-8"))
            self.assertEqual(request["theme"]["environment"], load_json(ROOT / before["environmentSource"]))
            if authored["kind"] in ("opaqueSurface", "surfacePaint"):
                self.assertEqual(request["adjacentColor"], color)
                self.assertEqual(request["foregroundRole"], authored["foregroundRole"])
                self.assertEqual(request["minimumContentContrast"], 4.5)
                self.assertEqual(request["minimumEdgeContrast"], 3)
                self.assertEqual(request["appearance"], load_json(ROOT / before["appearanceSource"]))
                self.assertEqual("postTreatmentBackdrop" in request, authored["expectedMaterialFamily"] == "frost")
            else:
                self.assertEqual(request["surroundingColor"], color)
                self.assertNotIn("appearance", request)
        bundle["scenarios"][0]["request"]["theme"]["environment"]["locale"] = "changed"
        self.assertNotEqual(bundle["scenarios"][1]["request"]["theme"]["environment"]["locale"], "changed")
        bundle["scenarios"][16]["request"]["body"]["surface"]["states"]["states"].append("selected")
        self.assertEqual(bundle["scenarios"][17]["request"]["body"]["surface"]["states"]["states"], ["rest"])

    def test_invalid_manifest_fails_before_backend_execution(self):
        for mutate in [
            lambda m: m["scenarios"].append(copy.deepcopy(m["scenarios"][0])),
            lambda m: m["scenarios"][0].update(theme="missing"),
            lambda m: m["scenarios"][0]["surface"]["states"].update(states=["focused"]),
            lambda m: m["scenarios"][1]["surface"]["states"].update(states=["rest"]),
            lambda m: m.update(environmentSource="../environment.json"),
            lambda m: m["scenarios"][1].update(foregroundRole="content.primary"),
            lambda m: m["scenarios"][16]["surface"]["states"].update(states=["selected", "focused"]),
            lambda m: m["scenarios"][16].pop("foregroundRole"),
            lambda m: m["scenarios"][16].pop("minimumContentContrast"),
        ]:
            manifest = copy.deepcopy(self.manifest)
            mutate(manifest)
            with self.assertRaises(ValueError):
                prepare(manifest, self.backend)
        self.backend.assert_not_called()

    def test_complete_scenes_cover_each_authored_theme_family_and_preserve_state_sets(self):
        authored = [scene for scene in self.manifest["scenarios"] if scene["kind"] == "surfacePaint"]
        self.assertEqual(
            {(scene["theme"], scene["expectedMaterialFamily"], tuple(scene["surface"]["states"]["states"])) for scene in authored},
            {(theme, family, (state,)) for theme in ("light", "dark") for family in ("cast", "frost", "elastomer", "gel") for state in ("rest", "focused")},
        )
        authored[0]["surface"]["states"]["states"] = ["focused", "rest"]
        bundle = prepare(self.manifest, self.backend)
        self.assertEqual(bundle["scenarios"][16]["request"]["body"]["surface"]["states"]["states"], ["focused", "rest"])

    def test_complete_checker_rejects_channel_mismatch_guards_and_clipped_navigation(self):
        request = baseline()
        request["body"]["surface"]["states"]["states"] = ["focused"]
        scene = {"name": "complete", "kind": "surfacePaint", "expectedMaterialFamily": "cast", "request": request}
        result = expected_result(request)
        capture = {"origin": {"x": -6, "y": -6}, "width": 32, "height": 26, "pixelsPerUnit": 1}
        check_scene(scene, result, capture)
        for mutate in (
            lambda r: r.pop("focus"),
            lambda r: r["focus"]["indicator"]["binding"]["states"].update(states=["rest", "focused"]),
            lambda r: r["focus"]["geometry"]["silhouette"]["bounds"].update(width=21),
            lambda r: r["body"].update(foregroundRole="content.inverse"),
            lambda r: r["body"].update(contentContrastRatio=4.49),
            lambda r: r["body"]["edge"].update(contrastRatio=2.99),
        ):
            altered = copy.deepcopy(result)
            mutate(altered)
            with self.assertRaises(ValueError):
                check_scene(scene, altered, capture)
        with self.assertRaisesRegex(ValueError, "clips"):
            check_scene(scene, result, capture | {"origin": {"x": -3, "y": -6}})
        request["body"]["surface"]["states"]["states"] = ["rest"]
        rest = expected_result(request)
        check_scene(scene, rest, capture)
        rest["focus"] = result["focus"]
        with self.assertRaisesRegex(ValueError, "navigation channel"):
            check_scene(scene, rest, capture)

    def test_family_mismatch_is_not_hidden_by_defaults(self):
        self.manifest["scenarios"][0]["expectedMaterialFamily"] = "gel"
        with self.assertRaisesRegex(ValueError, "material family"):
            prepare(self.manifest, self.backend)

    def test_asset_escape_fails_before_reading(self):
        with self.assertRaisesRegex(ValueError, "escapes repository"):
            asset_source("../outside.json")
        with patch("material_scenarios.ROOT", ROOT / "conformance"):
            with self.assertRaisesRegex(ValueError, "escapes repository"):
                asset_source(str(ROOT / "tokens/foundation.json"))

    def test_backend_status_diagnostics_duplicates_and_schema_are_checked(self):
        for output in [
            subprocess.CompletedProcess([], 1, "", "failed"),
            subprocess.CompletedProcess([], 0, "{}", "warning"),
            subprocess.CompletedProcess([], 0, '{"schemaVersion":"0.4.0","schemaVersion":"0.4.0"}', ""),
            subprocess.CompletedProcess([], 0, "{}", ""),
        ]:
            with patch("material_scenarios.run_backend", return_value=output):
                with self.assertRaises(ValueError):
                    backend_document(["backend", "-"], {}, "schemas/headless-result.schema.json", "test", 30)

    def test_generator_writes_utf8_independently_of_host_text_encoding(self):
        output = io.BytesIO()
        stdout = io.TextIOWrapper(output, encoding="ascii")
        with patch("sys.argv", ["material_scenarios.py", "--", "backend", "-"]), patch("sys.stdout", stdout), patch(
            "material_scenarios.prepare", return_value={"source": "مرحبا"},
        ):
            self.assertEqual(main(), 0)
        self.assertEqual(output.getvalue(), '{\n  "source": "مرحبا"\n}\n'.encode("utf-8"))

    def test_scene_checker_rejects_wrong_semantics_and_clipped_ring(self):
        prepared = prepare(self.manifest, self.backend)
        scene = prepared["scenarios"][1]
        scene["request"]["surface"]["colorRole"] = "surface.base"
        scene["request"]["surface"]["form"]["shape"] = "structural"
        result = load_json(ROOT / "conformance/ir/focus-ir-expected.json")
        capture = {"origin": {"x": -6, "y": -6}, "width": 32, "height": 26, "pixelsPerUnit": 1}
        check_scene(scene, result, capture)
        for change in ["family", "state", "form", "crop"]:
            altered = copy.deepcopy(result)
            view = copy.deepcopy(capture)
            if change == "family":
                altered["indicator"]["binding"]["materialFamily"] = "gel"
            elif change == "state":
                altered["indicator"]["binding"]["states"]["states"] = ["focused"]
            elif change == "form":
                altered["indicator"]["binding"]["form"]["shape"] = "capsule"
            else:
                view["origin"]["x"] = -3
            with self.assertRaises(ValueError):
                check_scene(scene, altered, view)

    def test_surface_checker_preserves_foreground_guards_and_side_capture(self):
        scene = prepare(self.manifest, self.backend)["scenarios"][0]
        result = load_json(ROOT / "conformance/ir/opaque-surface-expected.json")
        scene["request"]["surface"]["colorRole"] = result["colorRole"]
        capture = {"origin": {"x": -1, "y": -1}, "width": 24, "height": 18, "pixelsPerUnit": 1}
        check_scene(scene, result, capture)
        boundary_fit = {**capture, "origin": {"x": -1, "y": -1 - 2e-13}, "height": 15}
        check_scene(scene, result, boundary_fit)
        for change in ("foreground", "content", "edge", "crop"):
            altered, view = copy.deepcopy(result), copy.deepcopy(capture)
            if change == "foreground":
                altered["foregroundRole"] = "content.inverse"
            elif change == "content":
                altered["contentContrastRatio"] = 4.49
            elif change == "edge":
                altered["edge"]["contrastRatio"] = 2.99
            else:
                view["height"] = 14
            with self.assertRaises(ValueError):
                check_scene(scene, altered, view)


if __name__ == "__main__":
    unittest.main()
