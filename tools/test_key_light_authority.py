import copy
import unittest

from check_schemas import ROOT, check_key_light_authority, load_json


class KeyLightAuthorityTests(unittest.TestCase):
    def setUp(self):
        self.authority = load_json(ROOT / "definitions/key-light.json")
        self.shadow = [case["request"]["keyLight"] for case in load_json(ROOT / "conformance/elevation/shadow-vectors.json")]
        self.copies = {"material light": copy.deepcopy(self.authority), "appearance": copy.deepcopy(self.authority)}

    def test_authored_copies_and_shadow_calibration_match_the_authority(self):
        check_key_light_authority(self.authority, self.copies, self.shadow)

    def test_a_differing_copy_is_rejected(self):
        for name in self.copies:
            for member, value in (("x", -0.35), ("y", -0.93)):
                copies = copy.deepcopy(self.copies)
                copies[name]["direction"][member] = value
                with self.subTest(name=name, member=member), self.assertRaisesRegex(ValueError, name):
                    check_key_light_authority(self.authority, copies, self.shadow)
            copies = copy.deepcopy(self.copies)
            copies[name]["schemaVersion"] = "0.2.0"
            with self.subTest(name=name, member="schemaVersion"), self.assertRaisesRegex(ValueError, name):
                check_key_light_authority(self.authority, copies, self.shadow)

    def test_an_authority_the_shadow_vectors_do_not_calibrate_is_rejected(self):
        authority = copy.deepcopy(self.authority)
        authority["direction"] = {"x": -1, "y": -1}
        copies = {name: copy.deepcopy(authority) for name in self.copies}
        shadow = [light for light in self.shadow if light != authority]
        with self.assertRaisesRegex(ValueError, "is not the key direction"):
            check_key_light_authority(authority, copies, shadow)


if __name__ == "__main__":
    unittest.main()
