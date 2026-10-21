import copy
import json
from pathlib import Path
import tempfile
import unittest

from check_quickshell_command_motion_budget import (AppendedMotion, FLOOR_INTERVAL, GAP_BOUND, TICK,
                                                     continuations, describe_tail, evaluate, measure, summary,
                                                     tail, verify_scale)
from test_quickshell_command_motion import FIXTURE, HELD, REST, frame

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


class FakeMotion:
    def __init__(self, spanning_every):
        self.log, self.spanning_every, self.clock = [], spanning_every, 0

    def frames(self):
        return list(self.log)

    def publish(self, pressed, count=6):
        for _ in range(count):
            self.clock += .02
            spans = self.spanning_every and len(self.log) % self.spanning_every == 0
            self.log.append(timed(self.clock, span=TICK + .01 if spans else .01, delay=.001,
                                  hold=HELD["hold"] if pressed else None))


class FakeSession:
    def __init__(self, motion):
        self.motion = motion
        self.input = self
        self.cycles = 0

    def move(self, *position):
        pass

    def reset(self, *arguments):
        pass

    def settle(self, *arguments):
        pass

    def key(self, key, pressed):
        self.cycles += not pressed
        self.motion.publish(pressed)

    def wait(self, condition, description, timeout):
        assert condition(), description


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
        self.assertEqual(result["gap"], {"frames": 19, "bound": GAP_BOUND, "evaluated": False, "observed": 49,
                                         "longestSpan": .019, "unexercised": False})
        self.assertTrue(evaluate(measured(.02, delay=.001))["gap"]["evaluated"])

    def test_a_host_whose_predecessors_never_spanned_a_tick_leaves_the_gap_unexercised(self):
        fast = TICK - 1e-4
        result = evaluate(measured(.02, count=200, delay=.012, span=fast))
        self.assertEqual((result["gap"]["evaluated"], result["gap"]["unexercised"], result["gap"]["frames"]),
                         (False, True, 0))
        self.assertEqual(result["gap"]["longestSpan"], fast)
        self.assertFalse(evaluate(measured(.02, count=200, delay=.012, span=TICK))["gap"]["unexercised"])

    def test_some_frames_that_spanned_a_tick_are_never_unexercised(self):
        fast = TICK - 1e-4
        for spanned in (1, 19):
            with self.subTest(spanned=spanned):
                gap = evaluate(measured(.02, count=200, delay=.001, span=fast)
                               + measured(.04, count=spanned, delay=.001, span=TICK + .01))["gap"]
                self.assertEqual((gap["evaluated"], gap["unexercised"], gap["frames"]), (False, False, spanned))

    def test_unexercised_requires_enough_observed_predecessors_to_show_it(self):
        gap = evaluate(measured(.02, count=19, delay=.012, span=.01) + measured(.03, count=30))["gap"]
        self.assertEqual((gap["observed"], gap["unexercised"]), (19, False))
        self.assertTrue(evaluate(measured(.02, count=20, delay=.012, span=.01))["gap"]["unexercised"])

    def test_a_slow_host_evaluates_a_delayed_publication(self):
        gap = evaluate(measured(.04, count=300, delay=.012, span=.028))["gap"]
        self.assertEqual((gap["evaluated"], gap["met"], gap["unexercised"]), (True, False, False))

    def test_a_borderline_host_evaluates_exactly_at_the_minimum(self):
        fast = measured(.02, count=400, delay=.012, span=.012)
        gap = evaluate(fast + measured(.04, count=20, delay=.001, span=TICK))["gap"]
        self.assertEqual((gap["frames"], gap["evaluated"], gap["met"]), (20, True, True))
        gap = evaluate(fast + measured(.04, count=19, delay=.001, span=TICK))["gap"]
        self.assertEqual((gap["frames"], gap["evaluated"], gap["unexercised"]), (19, False, False))

    def test_presses_continue_until_enough_frames_spanned_a_tick(self):
        motion = FakeMotion(spanning_every=20)
        session = FakeSession(motion)
        frames, presses = measure(session, motion, 3, 100, 6)
        qualifying = evaluate(continuations(frames))["gap"]["frames"]
        self.assertEqual((qualifying, session.cycles, presses), (20, 34, 34))

    def test_presses_stop_at_the_minimum_when_enough_frames_already_spanned_a_tick(self):
        motion = FakeMotion(spanning_every=2)
        session = FakeSession(motion)
        measure(session, motion, 5, 100, 6)
        self.assertEqual(session.cycles, 5)

    def test_presses_stop_at_the_cap_when_a_host_never_spans_a_tick(self):
        motion = FakeMotion(spanning_every=0)
        session = FakeSession(motion)
        frames, presses = measure(session, motion, 3, 8, 6)
        self.assertEqual((session.cycles, presses), (8, 8))
        self.assertTrue(evaluate(continuations(frames))["gap"]["unexercised"])

    def test_motion_frames_are_parsed_once_from_the_complete_lines_of_a_growing_log(self):
        def line(event):
            return "RESINA_LIFECYCLE " + json.dumps(event) + "\n"

        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "shell.log"
            session = type("Session", (), {"log": log})()
            motion = AppendedMotion(session)
            log.write_text("", encoding="utf-8")
            self.assertEqual(motion.frames(), [])
            first, second = timed(.1), timed(.2)
            with log.open("a", encoding="utf-8") as output:
                output.write(line(first) + line({"kind": "started", "generation": "g"}))
                output.write(line(second)[:30])
            self.assertEqual([item["motion"]["time"] for item in motion.frames()], [.1])
            with log.open("a", encoding="utf-8") as output:
                output.write(line(second)[30:] + "unrelated output\n")
            self.assertEqual([item["motion"]["time"] for item in motion.frames()], [.1, .2])
            self.assertEqual(len(motion.frames()), 2)

    def test_summary_uses_nearest_rank_percentiles(self):
        self.assertEqual(summary(list(range(10, 0, -1))), {"median": 5.5, "p90": 9, "max": 10})
        self.assertEqual(summary([3]), {"median": 3, "p90": 3, "max": 3})

    def test_floor_is_the_median_interval_at_thirty_hertz(self):
        self.assertTrue(evaluate(measured(FLOOR_INTERVAL))["floor"]["met"])
        self.assertFalse(evaluate(measured(FLOOR_INTERVAL + 1e-4))["floor"]["met"])
        self.assertTrue(evaluate(measured(.02, count=11) + measured(.05, count=10))["floor"]["met"])
        self.assertFalse(evaluate(measured(.02, count=10) + measured(.05, count=11))["floor"]["met"])

    def test_tail_reports_high_percentiles_shares_and_the_longest_slow_run(self):
        intervals = [.016] * 90 + [.034, .05, .016, .034, .034, .034, .042, .016, .016, .060]
        result = tail(intervals)
        self.assertEqual((result["p95"], result["p99"], result["max"]), (.034, .05, .060))
        self.assertEqual([(bound["interval"], bound["share"]) for bound in result["above"]],
                         [(FLOOR_INTERVAL, .07), (1 / 24, .03)])
        self.assertEqual(result["longestRunAbove"], {"interval": FLOOR_INTERVAL, "frames": 4})
        self.assertEqual(tail([.016] * 20)["longestRunAbove"]["frames"], 0)
        # An interval at a bound is not above it.
        self.assertEqual([bound["share"] for bound in tail([FLOOR_INTERVAL, 1 / 24])["above"]], [.5, 0])

    def test_a_median_floor_that_is_met_still_publishes_its_slow_tail(self):
        result = evaluate(measured(.02, count=11) + measured(.05, count=10))
        self.assertTrue(result["floor"]["met"])
        self.assertEqual(result["tail"]["max"], .05)
        self.assertEqual([bound["share"] for bound in result["tail"]["above"]], [10 / 21, 10 / 21])
        self.assertEqual(result["tail"]["longestRunAbove"]["frames"], 10)
        self.assertEqual(describe_tail(2, result["tail"]),
                         "TAIL scale 2: published interval p95 50.0 ms, p99 50.0 ms, max 50.0 ms; 47.6% above "
                         "33.3 ms, 47.6% above 41.7 ms; longest run of 10 consecutive intervals above 33.3 ms "
                         "(reported, not enforced)")

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
