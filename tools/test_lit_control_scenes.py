import copy
import unittest

from check_schemas import ROOT, load_json
from lit_control_scenes import KINDS, MANIFEST, Resolution, prepare, viewport


def refuse(request):
    raise AssertionError("an invalid catalog must be rejected before any theme is resolved")


class LitControlScenesTests(unittest.TestCase):
    def manifest(self):
        return copy.deepcopy(load_json(MANIFEST))

    def test_catalog_covers_both_themes_every_part_state_light_and_environment(self):
        manifest = self.manifest()
        scenes = manifest["scenarios"]
        self.assertEqual(len(scenes), 84)
        self.assertEqual(len({scene["name"] for scene in scenes}), 84)
        for theme in manifest["themes"]:
            own = [scene for scene in scenes if scene["theme"] == theme]
            self.assertEqual(len(own), 42)
            commands = [scene for scene in own if scene["component"] == "command"]
            self.assertEqual({scene["variant"] for scene in commands}, {"standard", "primary"})
            for variant in ("standard", "primary"):
                self.assertEqual({tuple(scene["states"]) for scene in commands if scene["variant"] == variant and scene["light"] == "both"
                                  and scene["environment"] == "admitting"},
                                 {("rest",), ("hover",), ("pressed",), ("disabled",), ("rest", "focused")})
            toggles = [scene for scene in own if scene["component"] == "toggle"]
            for part, checked in ((p, c) for p in ("track", "thumb") for c in (False, True)):
                states = {tuple(scene["states"]) for scene in toggles if scene["part"] == part and scene["checked"] is checked
                          and scene["light"] == "both" and scene["environment"] == "admitting"}
                self.assertEqual(states, {("rest",), ("hover",), ("pressed",), ("disabled",)} | ({("rest", "focused")} if part == "track" else set()))
            self.assertEqual({scene["light"] for scene in own}, set(manifest["lights"]))
            self.assertEqual({scene["environment"] for scene in own}, set(manifest["environments"]))
        self.assertEqual({KINDS[scene["component"]] for scene in scenes}, {"commandPaint", "togglePartPaint"})

    def test_invalid_catalogs_fail_before_resolving_any_theme(self):
        for mutate, message in (
                (lambda m: m["scenarios"].append(copy.deepcopy(m["scenarios"][0])), "duplicate"),
                (lambda m: m["scenarios"][0].update(theme="sepia"), "unknown theme"),
                (lambda m: m["scenarios"][0].update(light="strobe"), "unknown light"),
                (lambda m: m["scenarios"][0].update(environment="outdoors"), "unknown environment"),
                (lambda m: m["scenarios"][0].update(part="track"), "variant|part|checked|valid"),
                (lambda m: m["scenarios"].append(dict(next(s for s in m["scenarios"] if s.get("part") == "thumb"),
                                                      name="thumb-ring", states=["rest", "focused"])), "focus ring")):
            manifest = self.manifest()
            mutate(manifest)
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                prepare(manifest, refuse)

    def test_resolution_derives_radii_and_depth_from_authored_assignments(self):
        resolution = Resolution({}, load_json(ROOT / "tokens/foundation.json"), load_json(ROOT / "definitions/tier0-surface-appearance.json"))
        request = {"surface": {"body": {"size": {"width": 64.5, "height": 40.5}, "surface": {"form": {"shape": "capsule", "elevation": "embedded"}}}}}
        self.assertEqual(resolution.radii(request)["topStart"], {"x": 20.25, "y": 20.25})
        self.assertEqual(resolution.depth(request), 0)
        request["surface"]["body"]["surface"]["form"] = {"shape": "rounded", "elevation": "raised"}
        self.assertEqual(resolution.radii(request)["bottomEnd"], {"x": 24, "y": 24})
        self.assertEqual(resolution.depth(request), 2)

    def test_viewport_encloses_the_part_and_its_margin_at_every_scale(self):
        request = {"surface": {"body": {"size": {"width": 64.5, "height": 40.5}}}}
        capture = self.manifest()["capture"]
        view = viewport(request, capture, 1.25, 3)
        self.assertEqual((view["origin"], view["width"], view["height"], view["pixelsPerUnit"], view["samplesPerAxis"]),
                         ({"x": -8, "y": -8}, 101, 71, 1.25, 3))


if __name__ == "__main__":
    unittest.main()
