import copy
import unittest
from unittest import mock

from check_command_motion_backend import equation_sample
from check_quickshell_command_motion import (CHANNELS, effect_positions, epochs, expected_states, frames, moving,
                                             settled, verify_activation, verify_callback_frame, verify_clock,
                                             verify_committed_frame, verify_dynamics, verify_frame_scales,
                                             verify_publication_scales, verify_retarget, verify_scale,
                                             verify_superseding_frames)

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


HELD = {"enabled": True, "focused": True, "hold": {"kind": "key", "key": "space"}}
COMMITTED = {"enabled": True, "focused": True, "hold": None}


def logged_frame(sequence, state, generation="g", configuration="motion"):
    return {"kind": "frame", "generation": generation, "sequence": sequence, "configuration": configuration,
            "snapshot": {"hitRegion": sequence}, "motion": {"sample": sequence}, "state": copy.deepcopy(state)}


def callback(effects, sequence, state=COMMITTED, generation="g", presented=None):
    shown = presented or {"snapshot": None, "motion": None, "ready": False}
    return {"kind": "effect", "generation": generation, "effects": effects, "frame": sequence,
            "state": copy.deepcopy(state), **shown}


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

    def test_the_command_must_be_presented_at_the_requested_device_scale(self):
        verify_scale({"width": 819, "height": 480, "deviceScale": 1.25}, 1.25)
        for screen, expected in ((None, 1.25), ({"width": 819, "height": 480}, 1.25), ({"deviceScale": 2}, 1.25),
                                 ({"deviceScale": True}, 1.0), ({"deviceScale": "1.25"}, 1.25)):
            with self.subTest(screen=screen), self.assertRaisesRegex(ValueError, "device scale"):
                verify_scale(screen, expected)

    @staticmethod
    def scaled(*scales):
        result = []
        for scale in scales:
            item = frame(0, 0)
            if scale is not None:
                item["deviceScale"] = scale
            result.append(item)
        return result

    def test_every_frame_is_published_at_the_requested_device_scale(self):
        verify_frame_scales(self.scaled(1.25, 1.25), 1.25)
        verify_frame_scales([], 2)
        for scales, message in (((1.25, 2), "device scale 2, not 1.25"), ((1.25, None), "does not publish"),
                                ((1.25, True), "does not publish"), ((1.25, "1.25"), "does not publish")):
            with self.subTest(scales=scales), self.assertRaisesRegex(ValueError, message):
                verify_frame_scales(self.scaled(*scales), 1.25)

    def test_a_live_scale_change_moves_frames_once_to_the_new_device_scale(self):
        verify_frame_scales(self.scaled(1.25, 1.25, 2, 2), 1.25, 2, 2)
        verify_frame_scales(self.scaled(2, 2, 1.25), 2, 2, 1.25)
        # A frame prepared before the compositor's change may still be logged after it was requested.
        verify_frame_scales(self.scaled(1.25, 1.25, 1.25, 2), 1.25, 2, 2)
        for scales, message in (((1.25, 2, 2), "device scale 2, not 1.25"),
                                ((1.25, 1.25, 1.25), "no native frame was published at device scale 2"),
                                ((1.25, 1.25, 2, 1.25), "did not move once"),
                                ((1.25, 1.25, 1, 2), "did not move once")):
            with self.subTest(scales=scales), self.assertRaisesRegex(ValueError, message):
                verify_frame_scales(self.scaled(*scales), 1.25, 2, 2)

    @staticmethod
    def logged(*entries):
        return [{"kind": kind, "deviceScale": scale} for kind, scale in entries]

    def test_publications_from_startup_use_the_window_device_scale(self):
        verify_publication_scales(self.logged(("published", 1.25), ("frame", 1.25)), 1.25)
        verify_publication_scales(self.logged(("published", 1.25), ("frame", 1.25), ("published", 1.25),
                                              ("frame", 2), ("published", 2)), 1.25, 3, 2)
        for entries, change, message in (
                ((("published", 2), ("published", 1.25), ("frame", 1.25)), None, "static publication was published "
                                                                                 "at device scale 2"),
                ((("frame", 1.25),), None, "no static publication"),
                ((("published", 1.25), ("frame", 1.25), ("published", 1.25), ("published", 1.25)), 2,
                 "no native frame was published at device scale 2"),
                ((("published", 1.25), ("frame", 1.25), ("frame", 2), ("published", 1.25)), 2,
                 "no static publication was published at device scale 2")):
            with self.subTest(entries=entries), self.assertRaisesRegex(ValueError, message):
                verify_publication_scales(self.logged(*entries), 1.25, change, 2 if change else None)


class CommandCallbackFrameTests(unittest.TestCase):
    def press(self, *after):
        return [logged_frame(1, COMMITTED), {"kind": "started"}, logged_frame(2, HELD), logged_frame(3, HELD),
                callback(1, 3), *after]

    def test_a_callback_may_observe_a_withdrawn_frame(self):
        events = self.press(logged_frame(4, COMMITTED))
        position = effect_positions(events)[0]
        verify_callback_frame(events, position, {})
        self.assertEqual(verify_activation(events, position, 1, {}), 3)
        verify_committed_frame(events, position, 3)

    def test_a_callback_must_report_the_last_logged_frame(self):
        for sequence in (2, 4, None):
            events = self.press()
            events[-1]["frame"] = sequence
            with self.subTest(sequence=sequence), self.assertRaisesRegex(ValueError, "reports frame sequence"):
                verify_callback_frame(events, len(events) - 1, {})
        events = [logged_frame(7, HELD, generation="retired"), callback(1, 0)]
        verify_callback_frame(events, 1, {})

    def test_a_callback_must_not_observe_an_incomplete_frame(self):
        frame = logged_frame(3, HELD)
        for presented in ({"snapshot": frame["snapshot"], "motion": None, "ready": True},
                          {"snapshot": None, "motion": frame["motion"], "ready": False},
                          {"snapshot": None, "motion": None, "ready": True},
                          {"snapshot": frame["snapshot"], "motion": frame["motion"], "ready": False}):
            events = self.press()
            events[-1].update(presented)
            with self.subTest(presented=presented), self.assertRaisesRegex(ValueError, "incomplete Command frame"):
                verify_callback_frame(events, len(events) - 1, {})

    def test_a_callback_must_not_observe_the_superseded_pressed_frame(self):
        stale = logged_frame(3, HELD)
        events = self.press()
        events[-1].update(snapshot=stale["snapshot"], motion=stale["motion"], ready=True)
        with self.assertRaisesRegex(ValueError, "frame 3 published for the superseded pressed request"):
            verify_callback_frame(events, len(events) - 1, {})

    def test_a_complete_callback_frame_must_be_the_logged_frame_for_committed_state(self):
        events = [logged_frame(1, HELD), logged_frame(2, COMMITTED)]
        events.append(callback(1, 2, presented={"snapshot": events[1]["snapshot"], "motion": events[1]["motion"],
                                                "ready": True}))
        with mock.patch("check_quickshell_command_motion.verify_frame") as verify:
            verify_callback_frame(events, 2, {"motion": {}})
            verify.assert_called_once_with(events[2], {"motion": {}})
            verify.side_effect = ValueError("native motion differs")
            with self.assertRaisesRegex(ValueError, "does not present the committed state: native motion differs"):
                verify_callback_frame(events, 2, {})
        events[2]["motion"] = events[0]["motion"]
        with self.assertRaisesRegex(ValueError, "complete frame other than frame 2"):
            verify_callback_frame(events, 2, {})

    def test_frames_before_the_callback_must_hold_the_press(self):
        events = self.press()
        events[3]["state"] = COMMITTED
        with self.assertRaisesRegex(ValueError, "frame 3, logged before activation effect 1, no longer presents"):
            verify_activation(events, len(events) - 1, 1, {})
        with self.assertRaisesRegex(ValueError, "no frame was logged during the press"):
            verify_activation(events, len(events) - 1, 3, {})

    def test_the_first_frame_after_the_callback_presents_committed_state(self):
        events = self.press()
        with self.assertRaisesRegex(ValueError, "no frame was logged after activation effect 1"):
            verify_committed_frame(events, len(events) - 1, 3)
        events = self.press(logged_frame(4, HELD), logged_frame(5, COMMITTED))
        with self.assertRaisesRegex(ValueError, "frame 4, the first after activation effect 1, presents state"):
            verify_committed_frame(events, 4, 3)

    def test_frames_after_a_superseding_request_never_present_the_earlier_one(self):
        unavailable = {"enabled": False, "focused": False, "hold": None}
        change = {"kind": "availability", "enabled": False}

        def expectation(frame):
            return not frame["state"]["enabled"]
        events = [logged_frame(1, HELD), change, logged_frame(2, unavailable), logged_frame(3, unavailable)]
        self.assertEqual([frame["sequence"] for frame in verify_superseding_frames(events, 1, expectation, "change")],
                         [2, 3])
        for later, message in (((), "no frame was logged after the change"),
                               ((HELD, unavailable), "frame 2, the first after the change, presents the superseded"),
                               ((unavailable, HELD), "frame 3 for the request superseded by the change")):
            sequence = [logged_frame(index + 2, state) for index, state in enumerate(later)]
            with self.subTest(later=later), self.assertRaisesRegex(ValueError, message):
                verify_superseding_frames([logged_frame(1, HELD), change, *sequence], 1, expectation, "change")
        with mock.patch("check_quickshell_command_motion.verify_frame",
                        side_effect=[None, ValueError("snapshot differs")]),                 self.assertRaisesRegex(ValueError, "frame 3 after the change does not present the request in "
                                                   "effect: snapshot differs"):
            verify_superseding_frames(events, 1, expectation, "change", {})


if __name__ == "__main__":
    unittest.main()
