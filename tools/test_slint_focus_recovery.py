import unittest

from check_slint_focus_recovery import check_observations


OBSERVATIONS = """resina-focus-recovery excluded false
resina-focus-recovery disabled-discoverable true
resina-focus-recovery enabled-retained true
resina-focus-recovery disabled-retained true
resina-focus-recovery unsafe-old-focus true
resina-focus-recovery unsafe-successor-focus true
resina-focus-recovery transferred-old-focus false
resina-focus-recovery transferred-successor-focus true
resina-focus-recovery excluded-after-transfer false
"""


class SlintFocusRecoveryTests(unittest.TestCase):
    def test_complete_observations(self):
        self.assertEqual(check_observations(OBSERVATIONS), 9)

    def test_incomplete_duplicate_unknown_and_malformed_observations_fail(self):
        for output in (
            "", "\n".join(OBSERVATIONS.splitlines()[1:]),
            OBSERVATIONS + OBSERVATIONS.splitlines()[0],
            OBSERVATIONS + "resina-focus-recovery unknown true",
            OBSERVATIONS.replace("excluded false", "excluded 0"),
            OBSERVATIONS.replace("excluded false", "excluded false extra"),
        ):
            with self.subTest(output=output), self.assertRaises(ValueError):
                check_observations(output)

    def test_every_incorrect_native_observation_fails(self):
        for line in OBSERVATIONS.splitlines():
            opposite = "false" if line.endswith("true") else "true"
            replacement = line.rsplit(" ", 1)[0] + " " + opposite
            with self.subTest(line=line), self.assertRaises(ValueError):
                check_observations(OBSERVATIONS.replace(line, replacement))


if __name__ == "__main__":
    unittest.main()
