import copy
import unittest

from check_command_motion_backend import equation_sample
from check_quickshell_command_motion import (CHANNELS, epochs, expected_states, frames, moving, settled, verify_clock,
                                             verify_dynamics, verify_retarget)

DYNAMICS = {"schemaVersion": "0.1.0", "mass": 1, "stiffness": 4, "damping": 4,
            "positionThreshold": 1e-06, "velocityThreshold": 1e-06}
REST = {"dynamics": DYNAMICS, "initial": {"position": 0, "velocity": 0}}
FIXTURE = {"bodyMix": copy.deepcopy(REST), "depthScale": copy.deepcopy(REST)}


def frame(time, sample_time, settled_channels=(False, False), policy="spring", target=None, channels=None,
          hovered=False, focused=True, enabled=True, hold=None, retarget=None):
    return {"kind": "frame", "time": time, "hovered": hovered,
            "state": {"enabled": enabled, "focused": focused, "hold": hold},
            "motion": {"time": sample_time, "retargetTime": retarget, "channels": copy.deepcopy(channels or FIXTURE),
                       "motion": {"policy": policy, "target": target or {"bodyMix": 1, "depthScale": .5},
                                  "bodyMix": {"settled": settled_channels[0]},
                                  "depthScale": {"settled": settled_channels[1]}}}}


def continued(previous, elapsed):
    return {name: {"dynamics": DYNAMICS,
                   "initial": dict(equation_sample(FIXTURE[name], previous["motion"]["motion"]["target"][name],
                                                   elapsed, False)["state"])} for name in CHANNELS}


class CommandMotionEvidenceTests(unittest.TestCase):
    def test_frames_are_selected_from_lifecycle_events(self):
        events = [{"kind": "started"}, frame(1, 0), {"kind": "effect"}, frame(2, 1)]
        self.assertEqual([item["time"] for item in frames(events)], [1, 2])

    def test_expected_states_follow_the_command_projection(self):
        cases = (
            ({}, ["rest", "focused"]),
            ({"hovered": True}, ["hover", "focused"]),
            ({"hold": {"kind": "key", "key": "space"}}, ["focused", "pressed"]),
            ({"hold": {"kind": "pointer", "id": "p", "inside": False}, "focused": False}, ["rest"]),
            ({"enabled": False, "focused": False}, ["disabled"]),
            ({"enabled": False, "focused": False, "hovered": True}, ["hover", "disabled"]),
        )
        for changes, states in cases:
            with self.subTest(changes=changes):
                self.assertEqual(expected_states(frame(0, 0, **changes))["states"], states)

    def test_spring_motion_requires_an_unsettled_channel(self):
        self.assertTrue(moving(frame(0, 0, (True, False))))
        self.assertFalse(moving(frame(0, 0, (True, True))))
        self.assertFalse(moving(frame(0, 0, (False, False), policy="reducedMotion")))
        self.assertTrue(settled(frame(0, 0, (True, True), policy="castImmediate")))

    def test_epochs_split_on_retarget(self):
        retargeted = {"bodyMix": {**REST, "initial": {"position": .4, "velocity": 1}}, "depthScale": REST}
        sequence = [frame(0, 0), frame(.1, .1), frame(.2, 0, channels=retargeted, target={"bodyMix": 0, "depthScale": 0})]
        self.assertEqual([len(group) for group in epochs(sequence)], [2, 1])

    def test_sample_clock_must_track_native_time_across_each_epoch(self):
        verify_clock([frame(10 + step * .016, step * .016) for step in range(60)])
        for rate in (2, .5):
            with self.subTest(rate=rate), self.assertRaisesRegex(ValueError, "sample clock advanced"):
                verify_clock([frame(10 + step * .016, step * .016 * rate) for step in range(60)])
        with self.assertRaisesRegex(ValueError, "long enough"):
            verify_clock([frame(10 + step * .016, step * .016) for step in range(10)])

    def test_dynamics_must_be_authored(self):
        verify_dynamics(frame(0, 0), FIXTURE)
        changed = copy.deepcopy(FIXTURE)
        changed["depthScale"]["dynamics"]["stiffness"] = 8
        with self.assertRaisesRegex(ValueError, "authored dynamics"):
            verify_dynamics(frame(0, 0, channels=changed), FIXTURE)

    def test_retarget_must_continue_from_the_reached_state(self):
        earlier, previous = frame(10, .1), frame(10.3, .3)
        released = {"bodyMix": 0, "depthScale": 0}
        channels = continued(previous, .35)
        # Log time lags the runtime clock by unrelated main-thread work.
        verify_retarget(earlier, previous, frame(10.6, .01, channels=channels, target=released, retarget=.35))
        for name in CHANNELS:
            jumped = copy.deepcopy(channels)
            jumped[name]["initial"]["position"] += .04
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "did not continue"):
                verify_retarget(earlier, previous, frame(10.36, .01, channels=jumped, target=released, retarget=.35))
        stopped = copy.deepcopy(channels)
        for name in stopped:
            stopped[name]["initial"]["velocity"] = 0
        with self.assertRaisesRegex(ValueError, "did not continue"):
            verify_retarget(earlier, previous, frame(10.36, .01, channels=stopped, target=released, retarget=.35))
        with self.assertRaisesRegex(ValueError, "did not continue"):
            verify_retarget(earlier, previous, frame(10.36, .01, channels=channels, target=released, retarget=.39))

    def test_retarget_instant_must_be_published_after_the_previous_sample(self):
        earlier, previous = frame(10, .1), frame(10.3, .3)
        for instant in (None, True, .29, float("inf"), float("nan"), "0.35"):
            with self.subTest(instant=instant), self.assertRaisesRegex(ValueError, "retarget instant"):
                verify_retarget(earlier, previous, frame(10.36, .01, target={"bodyMix": 0, "depthScale": 0},
                                                         retarget=instant))

    def test_retarget_instant_is_bounded_by_the_earlier_log(self):
        earlier, released = frame(10, .1), {"bodyMix": 0, "depthScale": 0}
        # The previous frame logged .19 s after its sample, later than the retarget it precedes.
        lagging = frame(10.5, .3)
        verify_retarget(earlier, lagging, frame(10.52, .01, channels=continued(lagging, .5), target=released,
                                                retarget=.5))
        previous = frame(10.3, .3)
        for instant in (.9, 1.1):
            with self.subTest(instant=instant), self.assertRaisesRegex(ValueError, "beyond"):
                verify_retarget(earlier, previous, frame(10.6, .01, channels=continued(previous, instant),
                                                         target=released, retarget=instant))
        verify_retarget(earlier, previous, frame(10.6, .01, channels=continued(previous, .89), target=released,
                                                 retarget=.89))

if __name__ == "__main__":
    unittest.main()
