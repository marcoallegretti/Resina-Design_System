import unittest

from check_quickshell_toggle_motion_budget import (
    EPOCH_GAP, FLOOR_INTERVAL, BudgetSession, evaluate, intervals,
)
from check_schemas import ROOT


def publication(frame, moving=True):
    return {"kind": "publication", "frame": frame, "moving": moving}


class ToggleMotionBudgetTests(unittest.TestCase):
    def test_intervals_are_the_frame_clock_steps_between_moving_publications(self):
        measured = intervals([publication(0), publication(.0167), publication(.05), publication(.0667, False)])
        self.assertEqual(len(measured), 2)
        self.assertAlmostEqual(measured[0], .0167)
        self.assertAlmostEqual(measured[1], .0333)

    def test_a_gap_longer_than_an_epoch_and_an_unmoving_frame_end_the_measured_run(self):
        self.assertEqual(intervals([publication(0), publication(EPOCH_GAP + .001)]), [])
        self.assertEqual(intervals([publication(0), publication(.016, False)]), [])
        self.assertEqual(intervals([publication(0, False), publication(.016)]), [])
        self.assertEqual(len(intervals([publication(0), publication(EPOCH_GAP)])), 1)

    def test_publications_must_report_motion_and_a_forward_clock(self):
        with self.assertRaisesRegex(ValueError, "whether its motion was moving"):
            intervals([publication(0), {"kind": "publication", "frame": .016}])
        with self.assertRaisesRegex(ValueError, "ran backwards"):
            intervals([publication(.5), publication(.4)])
        for bad in (-1, float("nan"), None, "0"):
            with self.subTest(bad=bad), self.assertRaisesRegex(ValueError, "finite non-negative"):
                intervals([publication(bad), publication(.1)])

    def test_the_floor_is_the_median_published_interval(self):
        slow = [FLOOR_INTERVAL * 1.5] * 10 + [FLOOR_INTERVAL / 2] * 9
        fast = [FLOOR_INTERVAL * 1.5] * 9 + [FLOOR_INTERVAL / 2] * 10
        for measured, met in ((fast, True), (slow, False)):
            result = evaluate(measured, minimum=19)
            self.assertIs(result["floor"]["met"], met)
            self.assertEqual(result["frames"], 19)
            self.assertAlmostEqual(result["tail"]["above"][0]["share"], 9 / 19 if met else 10 / 19)
        with self.assertRaisesRegex(ValueError, "only 3 continued spring frames"):
            evaluate([.016] * 3)

    def test_the_session_measures_ordinary_motion_with_the_authored_spring_channels(self):
        requests = BudgetSession.requests(object.__new__(BudgetSession))
        source = requests["standard"]
        self.assertIs(source["content"]["environment"]["accessibilityPreferences"]["reducedMotion"], False)
        self.assertEqual(set(requests["channels"]), {"track", "thumb", "travel"})

    def test_the_probe_reports_each_publication_with_the_frame_clock_and_motion_state(self):
        probe = (ROOT / "conformance/quickshell/toggle-lifecycle-probe.qml").read_text(encoding="utf-8")
        for fragment in ("FrameAnimation", "frame:frames.elapsedTime", "moving:moving(control)",
                         'function setMotion(enabled: bool)', "motionChannels: root.motionChannels"):
            self.assertIn(fragment, probe)


if __name__ == "__main__":
    unittest.main()
