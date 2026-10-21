import copy
import unittest
from unittest import mock

from check_command_motion_backend import equation_sample
from check_qt_command_motion_runtime import authored_motion
from check_quickshell_command_lifecycle import authored_requests
from check_quickshell_command_motion import (CHANNELS, boundaries, committed_epoch, effect_positions, epochs,
                                             expected_states, frames, moving, settled, verify_activation,
                                             verify_callback_frame, verify_clock, verify_committed_frame,
                                             verify_dynamics, verify_frame_scales, verify_publication_scales,
                                             verify_reduced_frost, verify_retarget, verify_retargets, verify_scale,
                                             verify_superseding_frames, verify_translucent_frost)

DYNAMICS = {"schemaVersion": "0.1.0", "mass": 1, "stiffness": 4, "damping": 4,
            "positionThreshold": 1e-06, "velocityThreshold": 1e-06}
REST = {"dynamics": DYNAMICS, "initial": {"position": 0, "velocity": 0}}
FIXTURE = {"bodyMix": copy.deepcopy(REST), "depthScale": copy.deepcopy(REST)}
REQUESTS = authored_requests()


def frame(time, sample_time, settled_channels=(False, False), policy="spring", target=None, channels=None,
          hovered=False, focused=True, enabled=True, hold=None, retarget=None, sequence=0, configuration="motion"):
    return {"kind": "frame", "generation": "g", "sequence": sequence, "configuration": configuration, "time": time,
            "hovered": hovered,
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


def target(phase, configuration="motion"):
    return authored_motion(REQUESTS[configuration], phase)[2]


def reached(previous, instant):
    immediate = previous["motion"]["motion"]["policy"] != "spring"
    return {name: {"dynamics": DYNAMICS,
                   "initial": dict(equation_sample(previous["motion"]["channels"][name],
                                                   previous["motion"]["motion"]["target"][name], instant,
                                                   immediate)["state"])} for name in CHANNELS}


def later(previous, sequence, time, sample_time):
    result = copy.deepcopy(previous)
    result.update(sequence=sequence, time=time)
    result["motion"]["time"] = sample_time
    return result


def retargeted(previous, sequence, time, instant, phase, configuration="motion", resumed=None, hovered=False,
               hold=None, enabled=True, sample_time=.001):
    _, policy, goal = authored_motion(REQUESTS[configuration], phase)
    result = frame(time, sample_time, policy=policy, target=goal,
                   channels=reached(previous, instant if resumed is None else resumed), hovered=hovered,
                   enabled=enabled, hold=hold, retarget=None if resumed is not None else instant, sequence=sequence,
                   configuration=configuration)
    result["motion"]["resumeTime"] = resumed
    return result


def committed(state, hovered=False, configuration="motion"):
    return {"kind": "effect", "generation": "g", "configuration": configuration, "state": copy.deepcopy(state),
            "hovered": hovered}


def at_rest(sequence, time, sample_time, configuration="motion"):
    _, policy, goal = authored_motion(REQUESTS[configuration], "rest")
    rest = {name: {"dynamics": DYNAMICS, "initial": {"position": goal[name], "velocity": 0}} for name in CHANNELS}
    return frame(time, sample_time, (True, True), policy=policy, target=goal, channels=rest, sequence=sequence,
                 configuration=configuration)


KEY = {"kind": "key", "key": "space"}


def session_log(settled_press=False):
    log = [at_rest(1, 10, .5), at_rest(2, 10.1, .6)]
    log.append(retargeted(log[-1], 3, 10.2, .65, "pressed", hold=KEY))
    log.append(later(log[-1], 4, 10.4, .15))
    log.append(committed(COMMITTED))
    log.append(retargeted(log[-2], 5, 10.5, .2, "rest"))
    log.append(later(log[-1], 6, 10.6, .08))
    log.append(retargeted(log[-1], 7, 10.7, .1, "hover", hovered=True))
    log.append(later(log[-1], 8, 10.8, 30 if settled_press else .08))
    log.append(retargeted(log[-1], 9, 10.9, 30.05 if settled_press else .1, "pressed", hovered=True, hold=KEY))
    log.append(later(log[-1], 10, 11, .08))
    log.append(retargeted(log[-1], 11, 11.1, .1, "pressed", hold=KEY))
    log.append(later(log[-1], 12, 11.2, .08))
    log.append({"kind": "configure", "generation": "g", "configuration": "reduced"})
    log.append(retargeted(log[-2], 13, 11.4, None, "pressed", "reduced", resumed=.15, hold=KEY))
    return log


class CommandRetargetTests(unittest.TestCase):
    def test_every_logged_boundary_continues_its_predecessor_and_is_counted_by_trigger(self):
        self.assertEqual(verify_retargets(session_log(), REQUESTS),
                         ({"press": 2, "release": 1, "hover enter": 1, "hover exit": 1, "policy change": 1,
                           "reconfiguration": 1},
                          {"press": 1, "release": 1, "hover enter": 1, "hover exit": 1, "policy change": 1,
                           "reconfiguration": 1}))
        self.assertEqual([(previous["sequence"], current["sequence"]) for _, previous, current, _
                          in boundaries(session_log())], [(2, 3), (4, 5), (6, 7), (8, 9), (10, 11), (12, 13)])

    def test_each_trigger_kind_needs_a_checked_boundary(self):
        log = session_log()
        for end, kind in ((7, "hover enter"), (11, "hover exit"), (-2, "policy change")):
            with self.subTest(kind=kind), self.assertRaisesRegex(ValueError, f"no logged clock epoch boundary was "
                                                                             f"triggered by {kind}$"):
                verify_retargets(log[:end], REQUESTS)

    def test_each_trigger_kind_needs_a_moving_hand_over(self):
        log = session_log(settled_press=True)
        self.assertEqual([log[index]["motion"]["channels"][name]["initial"]["velocity"]
                          for index in (2, 9) for name in CHANNELS], [0, 0, 0, 0])
        with self.assertRaisesRegex(ValueError, "no logged clock epoch boundary triggered by press handed over a "
                                                "moving state"):
            verify_retargets(log, REQUESTS)

    def test_a_retarget_continues_its_predecessor_at_the_published_instant_within_spec44_tolerance(self):
        earlier, previous = frame(10, .1, sequence=1), frame(10.3, .3, sequence=2)
        exact = retargeted(previous, 3, 10.6, .35, "rest")
        # Log time lags the runtime clock by unrelated main-thread work.
        verify_retarget(earlier, previous, exact)
        velocity = exact["motion"]["channels"]["bodyMix"]["initial"]["velocity"]
        self.assertGreater(abs(velocity), 1e-3)
        for field, value in (("velocity", velocity / 2), ("velocity", 1e-9),
                             ("position", exact["motion"]["channels"]["bodyMix"]["initial"]["position"] + .015)):
            changed = copy.deepcopy(exact)
            changed["motion"]["channels"]["bodyMix"]["initial"][field] = value
            with self.subTest(field=field, value=value), self.assertRaisesRegex(
                    ValueError, "bodyMix at frame 3 did not continue from the reached state"):
                verify_retarget(earlier, previous, changed)
        for name in CHANNELS:
            jumped = copy.deepcopy(exact)
            jumped["motion"]["channels"][name]["initial"]["position"] += 2e-9
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, f"{name} at frame 3 did not continue"):
                verify_retarget(earlier, previous, jumped)
            within = copy.deepcopy(exact)
            within["motion"]["channels"][name]["initial"]["velocity"] += 5e-10
            verify_retarget(earlier, previous, within)
        late = copy.deepcopy(exact)
        late["motion"]["retargetTime"] = .3501
        with self.assertRaisesRegex(ValueError, "did not continue"):
            verify_retarget(earlier, previous, late)

    def test_a_projected_release_position_is_rejected(self):
        earlier, previous = frame(10, .1, sequence=1, hold=HELD["hold"]), frame(10.3, .3, sequence=2, hold=HELD["hold"])
        projected = retargeted(previous, 3, 10.6, .35, "rest")
        for name in CHANNELS:
            projected["motion"]["channels"][name]["initial"]["position"] = previous["motion"]["motion"]["target"][name]
        with self.assertRaisesRegex(ValueError, "bodyMix at frame 3 did not continue from the reached state"):
            verify_retarget(earlier, previous, projected)

    def test_zero_velocity_is_handed_over_only_by_a_resting_or_immediate_predecessor(self):
        earlier = frame(12.9, 2.9, hovered=True, target=target("hover"), sequence=1)
        previous = frame(13, 3, hovered=True, target=target("hover"), sequence=2)
        exit = retargeted(previous, 3, 13.1, 3.05, "rest")
        verify_retarget(earlier, previous, exit)
        for name in CHANNELS:
            stopped = copy.deepcopy(exit)
            stopped["motion"]["channels"][name]["initial"]["velocity"] = 0
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, f"{name} at frame 3 "):
                verify_retarget(earlier, previous, stopped)
        resting = at_rest(2, 13, 3)
        verify_retarget(earlier, resting, retargeted(resting, 3, 13.1, 3.05, "pressed"))
        for configuration in ("reduced", "cast"):
            immediate = later(retargeted(resting, 2, 13, 3, "hover", configuration), 2, 13, 3)
            with self.subTest(configuration=configuration):
                press = retargeted(immediate, 3, 13.1, 3.05, "pressed", configuration)
                self.assertEqual([press["motion"]["channels"][name]["initial"]["velocity"] for name in CHANNELS],
                                 [0, 0])
                verify_retarget(earlier, immediate, press)

    def test_a_published_instant_is_a_finite_elapsed_time_within_the_logged_window(self):
        earlier, previous = frame(10, .1, sequence=1), frame(10.3, .3, sequence=2)
        for key, kind in (("retargetTime", "retarget"), ("resumeTime", "resume")):
            for instant in (True, -.1, float("inf"), float("nan"), "0.35"):
                invalid = retargeted(previous, 3, 10.36, .35, "rest")
                invalid["motion"].update(retargetTime=None, resumeTime=None)
                invalid["motion"][key] = instant
                with self.subTest(key=key, instant=instant), self.assertRaisesRegex(
                        ValueError, f"{kind} instant .* is not a finite elapsed time"):
                    verify_retarget(earlier, previous, invalid, reconfigured=True)
        # The previous frame logged .19 s after its sample, later than the retarget it precedes.
        lagging = frame(10.5, .3, sequence=2)
        verify_retarget(earlier, lagging, retargeted(lagging, 3, 10.52, .5, "rest", sample_time=.01))
        for instant in (.9, 1.1):
            with self.subTest(instant=instant), self.assertRaisesRegex(ValueError, "lies beyond the 0.592 s"):
                verify_retarget(earlier, previous, retargeted(previous, 3, 10.6, instant, "rest", sample_time=.01))
        verify_retarget(earlier, previous, retargeted(previous, 3, 10.6, .89, "rest", sample_time=.01))
        with self.assertRaisesRegex(ValueError, "resume instant 1.2 of frame 3 lies beyond the 0.501 s"):
            verify_retarget(earlier, previous, retargeted(previous, 3, 10.5, None, "rest", "reduced", resumed=1.2),
                            reconfigured=True)

    def test_motion_restarts_from_the_authored_channels_after_a_rejected_configuration(self):
        rejected = {"kind": "rejected", "generation": "g", "error": "requires a resolved opaque body"}
        restarted = later(at_rest(0, 0, 0), 15, 12, .01)
        restarted["motion"]["channels"] = copy.deepcopy(REQUESTS["channels"])
        counts = verify_retargets([*session_log(), rejected, restarted], REQUESTS)[0]
        self.assertEqual(counts["restart after rejection"], 1)
        for change, message in ((lambda item: item["motion"].update(resumeTime=.02), "continues a clock epoch "
                                                                                     "across a rejected"),
                                (lambda item: item["motion"]["channels"]["bodyMix"]["initial"].update(velocity=.1),
                                 "bodyMix at frame 15 restarts motion after a rejected configuration")):
            changed = copy.deepcopy(restarted)
            change(changed)
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                verify_retargets([*session_log(), rejected, changed], REQUESTS)

    def test_every_boundary_publishes_exactly_one_instant(self):
        earlier, previous = frame(10, .1, sequence=1), frame(10.3, .3, sequence=2)
        unpublished = retargeted(previous, 3, 10.6, .35, "rest")
        unpublished["motion"]["retargetTime"] = None
        with self.assertRaisesRegex(ValueError, "frame 3 begins a clock epoch without a published retarget or resume "
                                                "instant"):
            verify_retarget(earlier, previous, unpublished, reconfigured=True)
        both = retargeted(previous, 3, 10.6, .35, "rest")
        both["motion"]["resumeTime"] = .35
        with self.assertRaisesRegex(ValueError, "publishes both a retarget and a resume instant"):
            verify_retarget(earlier, previous, both, reconfigured=True)

    def test_a_retarget_whose_predecessor_epoch_was_not_logged_fails(self):
        earlier, previous = frame(10, .1, sequence=1), frame(10.3, .3, sequence=2)
        with self.assertRaisesRegex(ValueError, "precedes the sample 0.3 of frame 2: the clock epoch it continues "
                                                "was not logged"):
            verify_retarget(earlier, previous, retargeted(previous, 3, 10.36, .29, "rest"))
        moved = retargeted(previous, 3, 10.36, .35, "rest")
        moved["center"] = {"x": 1, "y": 1}
        with self.assertRaisesRegex(ValueError, "its center changed after frame 2: the clock epoch it continues"):
            verify_retarget(earlier, previous, moved)
        with self.assertRaisesRegex(ValueError, "only one frame was logged before it"):
            verify_retarget(None, previous, retargeted(previous, 3, 10.36, .35, "rest"))
        for resumed in (None, .35):
            with self.subTest(resumed=resumed), self.assertRaisesRegex(
                    ValueError, "frame 3 continues a clock epoch of which no frame was logged"):
                boundaries([retargeted(previous, 3, 10.36, .35, "rest", resumed=resumed)])

    def test_a_reconfiguration_resumes_from_the_state_reached_at_the_published_restart(self):
        earlier = frame(12.9, 2.9, hold=HELD["hold"], target=target("pressed"), sequence=1)
        previous = frame(13, 3, hold=HELD["hold"], target=target("pressed"), sequence=2)
        reduced = retargeted(previous, 3, 13.2, None, "rest", "reduced", resumed=3.05)
        verify_retarget(earlier, previous, reduced, reconfigured=True)
        # Channels sampled when the change arrived, before the clock restarted at the published instant.
        stale = copy.deepcopy(reduced)
        stale["motion"]["resumeTime"] = 3.17
        with self.assertRaisesRegex(ValueError, "bodyMix at frame 3 did not continue from the reached state: .* at "
                                                "3.17"):
            verify_retarget(earlier, previous, stale, reconfigured=True)
        released = retargeted(previous, 3, 13.2, None, "rest", resumed=3.05)
        with self.assertRaisesRegex(ValueError, "frame 3 resumes a clock epoch at 3.05, but no reconfiguration was "
                                                "logged after frame 2"):
            verify_retarget(earlier, previous, released)
        with self.assertRaisesRegex(ValueError, "no reconfiguration was logged"):
            verify_retargets([earlier, previous, released], REQUESTS)
        moved = copy.deepcopy(released)
        moved["center"] = {"x": 1, "y": 1}
        for evidence in ([earlier, previous, moved],
                         [earlier, previous, {"kind": "configure", "generation": "g", "configuration": "motion"},
                          released]):
            with self.subTest(evidence=evidence[2]["kind"]), self.assertRaisesRegex(ValueError, "triggered by"):
                verify_retargets(evidence, REQUESTS)
        dropped = copy.deepcopy(reduced)
        dropped["motion"]["channels"]["depthScale"]["initial"]["velocity"] = 0
        with self.assertRaisesRegex(ValueError, "depthScale at frame 3 did not continue from the reached state"):
            verify_retarget(earlier, previous, dropped, reconfigured=True)

    def test_an_epoch_superseded_before_publication_is_continued_in_its_place(self):
        earlier = frame(10, .1, hold=KEY, target=target("pressed"), sequence=1)
        previous = frame(10.3, .3, hold=KEY, target=target("pressed"), sequence=2)
        # The superseded epoch starts between the grid instants tried first.
        released = retargeted(previous, 0, 0, .34237, "rest")
        withdrawn = retargeted(released, 3, 10.5, .06, "disabled", enabled=False)
        epoch = committed_epoch(committed(COMMITTED), REQUESTS)
        with self.assertRaisesRegex(ValueError, "precedes the sample 0.3 of frame 2"):
            verify_retarget(earlier, previous, withdrawn)
        verify_retarget(earlier, previous, withdrawn, epoch)
        replaced = retargeted(released, 3, 10.5, None, "rest", "replacement", resumed=.1)
        with self.assertRaisesRegex(ValueError, "resumes at 0.1, but it precedes the sample 0.3"):
            verify_retarget(earlier, previous, replaced, reconfigured=True)
        verify_retarget(earlier, previous, replaced, epoch, True)
        # Both clock epochs elapse within the logged window.
        late = retargeted(retargeted(previous, 0, 0, .7, "rest"), 3, 10.5, None, "rest", "replacement", resumed=.4)
        with self.assertRaisesRegex(ValueError, "at 0.4; through the epoch prepared for the committed state from "):
            verify_retarget(earlier, previous, late, epoch, True)
        for field, value in (("position", 1e-6), ("velocity", 1e-6)):
            drifted = copy.deepcopy(withdrawn)
            drifted["motion"]["channels"]["bodyMix"]["initial"][field] += value
            with self.subTest(field=field), self.assertRaisesRegex(
                    ValueError, "^through the epoch prepared for the committed state from 0.342.*, bodyMix at "
                                "frame 3 did not continue .*; directly, the retarget at 0.06 is impossible because it "
                                "precedes the sample 0.3 of frame 2$"):
                verify_retarget(earlier, previous, drifted, epoch)


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
                        side_effect=[None, ValueError("snapshot differs")]), \
                self.assertRaisesRegex(ValueError, "frame 3 after the change does not present the request in "
                                                   "effect: snapshot differs"):
            verify_superseding_frames(events, 1, expectation, "change", {})

    def test_reduced_transparency_frost_publishes_opaque_dimensional_frames(self):
        valid = [frost_frame(1), frost_frame(2)]
        verify_reduced_frost(valid)
        for change, message in ((("materialFamily", "cast"), "frame 2 with reduced transparency lost its bound family"),
                                (("frostRepresentation", "translucentPigmented"),
                                 "publishes Frost representation 'translucentPigmented', not 'opaqueDimensional'"),
                                (("frostRepresentation", None), "publishes Frost representation None"),
                                (("alpha", .62), "publishes body alpha 0.62, not 1"),
                                (("contentFallbackApplied", True), "through the content fallback, not the preference")):
            sequence = [frost_frame(1), frost_frame(2, *change)]
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, message):
                verify_reduced_frost(sequence)
        with self.assertRaisesRegex(ValueError, "no Frost frame with reduced transparency was logged"):
            verify_reduced_frost([])

    def test_frost_without_reduced_transparency_is_withdrawn_by_the_opaque_profile(self):
        rejected = {"ready": False, "target": None,
                    "error": "command surface paint body: opaque surface IR requires a resolved opaque body"}
        verify_translucent_frost(rejected, [{"kind": "configure", "configuration": "translucentFrost"},
                                            {"kind": "rejected", "error": rejected["error"]}])
        rejection = [{"kind": "rejected", "error": rejected["error"]}]
        for change, message in (({"ready": True}, "was not withdrawn by the opaque body rejection: ready True"),
                                ({"target": {"x": 1, "y": 2}}, r"target \{'x': 1, 'y': 2\}"),
                                ({"error": ""}, "error ''"),
                                ({"error": "command label: missing font"}, "error 'command label: missing font'")):
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, message):
                verify_translucent_frost({**rejected, **change}, rejection)
        unrelated = "was not rejected for its translucent body"
        for events in ([], [{"kind": "rejected", "error": "command label: missing font"}]):
            with self.subTest(events=events), self.assertRaisesRegex(ValueError, unrelated):
                verify_translucent_frost(rejected, events)
        translucent = frost_frame(3, "frostRepresentation", "translucentPigmented")
        translucent["configuration"] = "translucentFrost"
        for event, message in ((translucent, "was published: frame event .*translucentPigmented"),
                               ({"kind": "published", "deviceScale": 1}, "was published: published event")):
            with self.subTest(event=event["kind"]), self.assertRaisesRegex(ValueError, message):
                verify_translucent_frost(rejected, [event])


def frost_frame(sequence, field=None, value=None):
    body = {"materialFamily": "frost", "frostRepresentation": "opaqueDimensional",
            "face": {"pigment": {"body": {"colorSpace": "srgb", "components": [.94, .94, .92], "alpha": 1}}},
            "contentFallbackApplied": False}
    if field == "alpha":
        body["face"]["pigment"]["body"]["alpha"] = value
    elif field == "frostRepresentation" and value is None:
        del body[field]
    elif field:
        body[field] = value
    return {"kind": "frame", "sequence": sequence, "configuration": "frost",
            "snapshot": {"paint": {"paint": {"body": body}}}}


if __name__ == "__main__":
    unittest.main()
