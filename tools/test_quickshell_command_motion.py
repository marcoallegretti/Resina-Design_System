import unittest

from check_quickshell_command_motion import expected_states, frames, moving, settled, verify_clock


def frame(time, sample_time, settled_channels=(False, False), policy="spring", target=None, channels=None,
          hovered=False, focused=True, enabled=True, hold=None):
    return {"kind": "frame", "time": time, "hovered": hovered,
            "state": {"enabled": enabled, "focused": focused, "hold": hold},
            "motion": {"time": sample_time, "channels": channels or {"bodyMix": 1},
                       "motion": {"policy": policy, "target": target or {"bodyMix": 1},
                                  "bodyMix": {"settled": settled_channels[0]},
                                  "depthScale": {"settled": settled_channels[1]}}}}


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

    def test_sample_clock_must_advance_with_native_time(self):
        verify_clock([frame(10.0, 0.0), frame(10.1, 0.1), frame(10.3, 0.3)])
        with self.assertRaisesRegex(ValueError, "did not advance"):
            verify_clock([frame(10.0, 0.5), frame(10.1, 0.5)])
        with self.assertRaisesRegex(ValueError, "does not track"):
            verify_clock([frame(10.0, 0.0), frame(10.1, 0.6)])

    def test_clock_pairs_exclude_retargeted_frames(self):
        retargeted = frame(10.1, 2.0, channels={"bodyMix": 2}, target={"bodyMix": 0})
        verify_clock([frame(10.0, 0.0), retargeted, frame(10.2, 2.1, channels={"bodyMix": 2}, target={"bodyMix": 0})])


if __name__ == "__main__":
    unittest.main()
