import copy
import itertools
import unittest

from capture_material_acceptance import ENVIRONMENTS, environments, motion_scenes
from check_command_motion_backend import equation_sample
from check_schemas import ROOT, load_json
from visual_acceptance import source_environment


class MaterialAcceptanceCaptureTests(unittest.TestCase):
    def test_environment_selection_preserves_caps_and_covers_declared_levels(self):
        base = load_json(ROOT / "conformance/environment/valid-minimal-capabilities.json")
        before = copy.deepcopy(base)
        variants = environments(load_json(ENVIRONMENTS), base)
        self.assertEqual(base, before)
        self.assertEqual(len(variants), 8)
        values = [environment for _, environment in variants]
        self.assertEqual({v["scale"] for v in values}, {1, 1.25, 1.5, 2})
        self.assertEqual({v["textScale"] for v in values}, {1, 1.5, 2})
        self.assertEqual({v["layoutDirection"] for v in values}, {"ltr", "rtl"})
        self.assertEqual({tuple(v["accessibilityPreferences"][key] for key in ("reducedMotion", "reducedTransparency", "highContrast")) for v in values},
                         set(itertools.product((False, True), repeat=3)))
        for variant in values:
            self.assertEqual(variant["rendererCapabilities"], before["rendererCapabilities"])
            self.assertEqual(variant["inputCapabilities"], before["inputCapabilities"])
        document = load_json(ENVIRONMENTS)
        document["variants"].append(copy.deepcopy(document["variants"][0]))
        with self.assertRaisesRegex(ValueError, "duplicate"):
            environments(document, base)

    def test_diagnostic_retarget_retains_raw_position_and_velocity(self):
        scenes = list(motion_scenes())
        self.assertEqual(len(scenes), 8)
        self.assertEqual(len({scene["name"] for scene in scenes}), 8)
        press, release = scenes[1]["request"], scenes[4]["request"]
        self.assertFalse(source_environment(scenes[1])["accessibilityPreferences"]["reducedMotion"])
        self.assertEqual(press["time"], 0.25)
        self.assertEqual(release["time"], 0)
        self.assertEqual(release["surface"]["body"]["surface"]["states"]["states"], ["rest", "focused"])
        target = press["commandAppearance"]["profiles"]["elastomer"]["pressed"]
        for name, value in target.items():
            expected = equation_sample(press["channels"][name], value, 0.25, False)["state"]
            self.assertEqual(release["channels"][name]["initial"], expected)
        self.assertNotEqual(press["channels"], release["channels"])
        self.assertTrue(any(channel["initial"]["velocity"] != 0 for channel in release["channels"].values()))


if __name__ == "__main__":
    unittest.main()
