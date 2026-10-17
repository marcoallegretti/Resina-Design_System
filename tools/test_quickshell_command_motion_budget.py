import copy
import unittest

from check_quickshell_command_motion_budget import FLOOR_INTERVAL, continuations, evaluate, summary, verify_scale
from test_quickshell_command_motion import FIXTURE, REST, frame

RETARGETED = {"bodyMix": {**REST, "initial": {"position": .4, "velocity": 1}}, "depthScale": copy.deepcopy(REST)}
RELEASED = {"bodyMix": 0, "depthScale": 0}


def timed(sample_time, preparation=.01, **changes):
    result = frame(0, sample_time, **changes)
    result["motion"]["preparationDuration"] = preparation
    return result


def measured(interval, preparation=.005, count=20):
    return [{"interval": interval, "preparation": preparation}] * count


class CommandMotionBudgetTests(unittest.TestCase):
    def test_intervals_follow_the_sample_clock_within_an_epoch(self):
        sequence = [timed(.1), timed(.13, .004), timed(.2, .006)]
        result = continuations(sequence)
        self.assertEqual([round(item["interval"], 9) for item in result], [.03, .07])
        self.assertEqual([item["preparation"] for item in result], [.004, .006])

    def test_only_frames_continuing_spring_motion_are_measured(self):
        cases = (
            [timed(.1, settled_channels=(True, True)), timed(.2)],
            [timed(.1, policy="reducedMotion"), timed(.2)],
            [timed(.1), timed(.2, policy="reducedMotion", settled_channels=(True, True))],
        )
        for sequence in cases:
            with self.subTest(sequence=sequence):
                self.assertEqual(continuations(sequence), [])
        self.assertEqual(len(continuations([timed(.1), timed(.2, settled_channels=(True, True))])), 1)

    def test_retarget_intervals_join_both_clock_epochs_at_the_runtime_instant(self):
        sequence = [timed(.5), timed(.004, channels=RETARGETED, target=RELEASED, retarget=.52),
                    timed(.03, channels=RETARGETED, target=RELEASED, retarget=.52)]
        self.assertEqual([round(item["interval"], 9) for item in continuations(sequence)], [.024, .026])

    def test_epochs_started_without_a_retarget_instant_are_not_joined(self):
        sequence = [timed(.5), timed(.004, channels=RETARGETED, target=RELEASED)]
        self.assertEqual(continuations(sequence), [])

    def test_inconsistent_runtime_clocks_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "restarted"):
            continuations([timed(.2), timed(.2)])
        with self.assertRaisesRegex(ValueError, "restarted"):
            continuations([timed(.2, retarget=.1), timed(.3, retarget=.4)])
        with self.assertRaisesRegex(ValueError, "precedes the previous sample"):
            continuations([timed(.5), timed(.004, channels=RETARGETED, target=RELEASED, retarget=.49)])
        for instant in (True, "0.52", float("inf"), -1):
            with self.subTest(instant=instant), self.assertRaisesRegex(ValueError, "retarget instant"):
                continuations([timed(.5), timed(.004, channels=RETARGETED, target=RELEASED, retarget=instant)])

    def test_each_measured_frame_requires_a_published_preparation_duration(self):
        for preparation in (None, False, "0.01", -.001, float("nan"), float("inf")):
            with self.subTest(preparation=preparation), self.assertRaisesRegex(ValueError, "preparation duration"):
                continuations([timed(.1), timed(.2, preparation)])
        missing = timed(.2)
        del missing["motion"]["preparationDuration"]
        with self.assertRaisesRegex(ValueError, "preparation duration"):
            continuations([timed(.1), missing])

    def test_summary_uses_nearest_rank_percentiles(self):
        self.assertEqual(summary(list(range(10, 0, -1))), {"median": 5.5, "p90": 9, "max": 10})
        self.assertEqual(summary([3]), {"median": 3, "p90": 3, "max": 3})

    def test_floor_is_the_median_interval_at_thirty_hertz(self):
        self.assertTrue(evaluate(measured(FLOOR_INTERVAL))["floor"]["met"])
        self.assertFalse(evaluate(measured(FLOOR_INTERVAL + 1e-4))["floor"]["met"])
        self.assertTrue(evaluate(measured(.02, count=11) + measured(.05, count=10))["floor"]["met"])
        self.assertFalse(evaluate(measured(.02, count=10) + measured(.05, count=11))["floor"]["met"])

    def test_target_requires_both_preparation_and_interval(self):
        self.assertTrue(evaluate(measured(.017, .008))["target"]["met"])
        self.assertFalse(evaluate(measured(.0171, .008))["target"]["met"])
        self.assertFalse(evaluate(measured(.017, .0081))["target"]["met"])

    def test_measurement_requires_the_requested_device_scale(self):
        verify_scale({"width": 512, "height": 300, "deviceScale": 2}, 2.0)
        for screen, expected in ((None, 1.0), ({"width": 1024, "height": 600}, 1.0), ({"deviceScale": 1}, 2.0),
                                 ({"deviceScale": True}, 1.0), ({"deviceScale": "2"}, 2.0)):
            with self.subTest(screen=screen), self.assertRaisesRegex(ValueError, "device scale"):
                verify_scale(screen, expected)

    def test_too_few_frames_cannot_qualify(self):
        with self.assertRaisesRegex(ValueError, "only 19 continued spring frames"):
            evaluate(measured(.01, count=19))
        self.assertEqual(evaluate(measured(.01, count=20))["frames"], 20)


if __name__ == "__main__":
    unittest.main()
