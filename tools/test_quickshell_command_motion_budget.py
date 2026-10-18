import copy
import unittest

from check_quickshell_command_motion_budget import (FLOOR_INTERVAL, GAP_BOUND, TICK, continuations, evaluate, summary,
                                                     verify_scale)
from test_quickshell_command_motion import FIXTURE, REST, frame

RETARGETED = {"bodyMix": {**REST, "initial": {"position": .4, "velocity": 1}}, "depthScale": copy.deepcopy(REST)}
RELEASED = {"bodyMix": 0, "depthScale": 0}


def timed(sample_time, preparation=.01, delay=.001, span=.02, **changes):
    result = frame(0, sample_time, **changes)
    result["motion"]["preparationDuration"] = preparation
    result["motion"]["publicationDelay"] = delay
    result["motion"]["predecessorSpan"] = None if delay is None else span
    return result


def measured(interval, preparation=.005, count=20, delay=None, span=None):
    if delay is not None and span is None:
        span = interval - delay
    return [{"interval": interval, "preparation": preparation, "publicationDelay": delay,
             "predecessorSpan": span}] * count


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

    def test_publication_delay_is_kept_only_within_a_clock_epoch(self):
        sequence = [timed(.1), timed(.13, delay=.002), timed(.2, delay=None)]
        self.assertEqual([item["publicationDelay"] for item in continuations(sequence)], [.002, None])
        sequence = [timed(.5), timed(.004, channels=RETARGETED, target=RELEASED, retarget=.52, delay=.003)]
        self.assertEqual([item["publicationDelay"] for item in continuations(sequence)], [None])
        for delay in (False, "0.001", -.001, float("nan"), float("inf")):
            with self.subTest(delay=delay), self.assertRaisesRegex(ValueError, "publication delay"):
                continuations([timed(.1), timed(.2, delay=delay)])

    def test_predecessor_span_is_published_with_the_delay(self):
        self.assertEqual([item["predecessorSpan"] for item in continuations([timed(.1), timed(.13, span=.012)])],
                         [.012])
        for span in (False, "0.02", -.001, float("nan"), float("inf")):
            with self.subTest(span=span), self.assertRaisesRegex(ValueError, "predecessor span"):
                continuations([timed(.1), timed(.2, span=span)])
        for delay, span in ((.001, None), (None, .02)):
            unpaired = timed(.2)
            unpaired["motion"]["publicationDelay"] = delay
            unpaired["motion"]["predecessorSpan"] = span
            with self.subTest(delay=delay), self.assertRaisesRegex(ValueError, "published together"):
                continuations([timed(.1), unpaired])

    def test_publication_gap_bounds_the_upper_quartile_after_a_spanned_tick(self):
        spanned = TICK + .02
        self.assertTrue(evaluate(measured(spanned, count=15, delay=GAP_BOUND)
                                 + measured(spanned, count=5, delay=.016))["gap"]["met"])
        self.assertFalse(evaluate(measured(spanned, count=14, delay=GAP_BOUND)
                                  + measured(spanned, count=6, delay=.016))["gap"]["met"])
        self.assertFalse(evaluate(measured(spanned, delay=GAP_BOUND + 1e-4))["gap"]["met"])
        # A predecessor published within one tick may correctly wait for the next tick.
        self.assertTrue(evaluate(measured(spanned, delay=.001)
                                 + measured(TICK, count=40, delay=.016, span=TICK - .001))["gap"]["met"])

    def test_publication_gap_selects_frames_by_the_published_span(self):
        # The interval suggests a spanned tick; the published span shows the predecessor did not span one.
        result = evaluate(measured(.04, delay=.016, span=.01) + measured(.04, delay=.001, span=TICK))
        self.assertEqual((result["gap"]["frames"], result["gap"]["met"]), (20, True))

    def test_publication_gap_requires_enough_frames_that_spanned_a_tick(self):
        result = evaluate(measured(.02, count=19, delay=.001) + measured(.01, count=30, delay=.009)
                          + measured(.03, count=30))
        self.assertEqual(result["gap"], {"frames": 19, "bound": GAP_BOUND, "evaluated": False})
        self.assertTrue(evaluate(measured(.02, delay=.001))["gap"]["evaluated"])

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
