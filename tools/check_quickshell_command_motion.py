"""Verify sampled Command motion and preference fallback with compositor-delivered input in released Quickshell."""
import argparse
import copy
import json
import os
from pathlib import Path
import subprocess
import sys

from check_command_motion_backend import equation_sample
from check_qt_command_motion_runtime import motion_mismatch
from check_qt_command_snapshot_runtime import ROOT, verify_snapshot
from check_quickshell_command_lifecycle import OUTSIDE, Session, authored_requests, expect_effects, effects, focused
from check_quickshell_command_updates import verify_version
from sway_output import Sway, changed_scale
from wayland_virtual_input import KEY_SPACE

CHANNELS = ("bodyMix", "depthScale")


def frames(events):
    return [event for event in events if event["kind"] == "frame"]


def expected_states(frame):
    state, hovered = frame["state"], frame["hovered"]
    hold = state["hold"]
    pressed = bool(hold and (hold["kind"] == "key" or hold["inside"]))
    signals = ["rest"] if not hovered and not pressed else []
    if hovered:
        signals.append("hover")
    if state["focused"]:
        signals.append("focused")
    if pressed:
        signals.append("pressed")
    if not state["enabled"]:
        if signals == ["rest"]:
            signals = []
        signals.append("disabled")
    return {"schemaVersion": "0.1.0", "states": signals or ["rest"]}


def verify_frame(frame, requests):
    source = copy.deepcopy(requests[frame["configuration"]])
    source.update(activation=frame["state"], hovered=frame["hovered"], focusable=frame["state"]["enabled"])
    metadata = frame["motion"]
    verify_snapshot(frame["snapshot"], source, expected_states(frame), metadata["motion"])
    difference = motion_mismatch(metadata["motion"], source, metadata)
    if difference:
        raise ValueError("native motion differs from independent equations at " + difference)
    if frame["target"] != frame["snapshot"]["hitRegion"]:
        raise ValueError("sampled native target differs from the published snapshot")


def verify_callback_frame(events, position, requests):
    effect = events[position]
    name = f"activation effect {effect['effects']}"
    logged = [event for event in events[:position]
              if event["kind"] == "frame" and event["generation"] == effect["generation"]]
    sequence = logged[-1]["sequence"] if logged else 0
    if effect.get("frame") != sequence:
        raise ValueError(f"{name} reports frame sequence {effect.get('frame')!r}, but frame {sequence} was logged "
                         "last before it")
    snapshot, motion, ready = effect.get("snapshot"), effect.get("motion"), effect.get("ready")
    if snapshot is None and motion is None and ready is False:
        return
    if snapshot is None or motion is None or ready is not True:
        raise ValueError(f"{name} observed an incomplete Command frame: snapshot "
                         f"{'absent' if snapshot is None else 'present'}, motion "
                         f"{'absent' if motion is None else 'present'}, ready {ready!r}")
    if not logged or snapshot != logged[-1]["snapshot"] or motion != logged[-1]["motion"]:
        raise ValueError(f"{name} observed a complete frame other than frame {sequence}, the last frame logged "
                         "before it")
    if logged[-1]["state"] != effect["state"]:
        superseded = " for the superseded pressed request" if logged[-1]["state"]["hold"] else ""
        raise ValueError(f"{name} observed frame {sequence} published{superseded} with state "
                         f"{logged[-1]['state']}, not the committed state {effect['state']}")
    try:
        verify_frame(effect, requests)
    except ValueError as error:
        raise ValueError(f"{name} observed frame {sequence}, which does not present the committed state: "
                         f"{error}") from None


def verify_activation(events, position, start, requests):
    verify_callback_frame(events, position, requests)
    effect = events[position]
    split = len(frames(events[:position]))
    pressed = frames(events)[start:split]
    if not pressed:
        raise ValueError(f"no frame was logged during the press before activation effect {effect['effects']}")
    released = [frame for frame in pressed if not frame["state"]["hold"]]
    if released:
        raise ValueError(f"frame {released[0]['sequence']}, logged before activation effect {effect['effects']}, "
                         "no longer presents the held press")
    return split


def verify_committed_frame(events, position, split):
    effect, recorded = events[position], frames(events)
    if split >= len(recorded):
        raise ValueError(f"no frame was logged after activation effect {effect['effects']}")
    if recorded[split]["state"] != effect["state"]:
        raise ValueError(f"frame {recorded[split]['sequence']}, the first after activation effect "
                         f"{effect['effects']}, presents state {recorded[split]['state']}, not the committed state "
                         f"{effect['state']}")


def verify_superseding_frames(events, marker, expectation, name):
    later = frames(events[marker + 1:])
    if not later:
        raise ValueError(f"no frame was logged after the {name}")
    if not expectation(later[0]):
        raise ValueError(f"frame {later[0]['sequence']}, the first after the {name}, presents the superseded "
                         f"request: configuration {later[0]['configuration']}, state {later[0]['state']}")
    stale = [frame for frame in later if not expectation(frame)]
    if stale:
        raise ValueError(f"frame {stale[0]['sequence']} for the request superseded by the {name} was published "
                         "after it")
    return later


def effect_positions(events):
    return [index for index, event in enumerate(events) if event["kind"] == "effect"]


def verify_scale(observed, expected):
    scale = observed.get("deviceScale") if observed else None
    if type(scale) not in (int, float) or scale != expected:
        raise ValueError(f"the Command's window has device scale {scale!r}, not {expected!r}")


def verify_frame_scales(sequence, initial, change=None, final=None, kind="native frame"):
    scales = [frame.get("deviceScale") for frame in sequence]
    if any(type(scale) not in (int, float) for scale in scales):
        raise ValueError(f"a {kind} does not publish its device scale")
    end = len(scales) if change is None else change
    early = [scale for scale in scales[:end] if scale != initial]
    if early:
        raise ValueError(f"a {kind} was published at device scale {early[0]!r}, not {initial!r}")
    if change is None:
        return
    later = scales[change:]
    if final not in later:
        raise ValueError(f"no {kind} was published at device scale {final!r} after the scale change")
    first = later.index(final)
    if any(scale != initial for scale in later[:first]) or any(scale != final for scale in later[first:]):
        raise ValueError(f"{kind}s after the scale change did not move once from {initial!r} to {final!r}")


def verify_publication_scales(events, initial, change=None, final=None):
    frames = [event for event in events if event["kind"] == "frame"]
    static = [event for event in events if event["kind"] == "published"]
    if not static:
        raise ValueError("no static publication was logged from startup")
    if change is None:
        verify_frame_scales(frames, initial)
        verify_frame_scales(static, initial, kind="static publication")
        return
    earlier = [event["kind"] for event in events[:change]]
    verify_frame_scales(frames, initial, earlier.count("frame"), final)
    split = earlier.count("published")
    verify_frame_scales(static[:split], initial, kind="static publication")
    if static[split:]:
        verify_frame_scales(static[split:], initial, 0, final, "static publication")


def settled(frame):
    return all(frame["motion"]["motion"][name]["settled"] for name in CHANNELS)


def moving(frame):
    return frame["motion"]["motion"]["policy"] == "spring" and not settled(frame)


def epochs(sequence):
    groups = []
    for frame in sequence:
        key = (json.dumps(frame["motion"]["channels"], sort_keys=True),
               json.dumps(frame["motion"]["motion"]["target"], sort_keys=True))
        if groups and groups[-1][0] == key:
            groups[-1][1].append(frame)
        else:
            groups.append((key, [frame]))
    return [group for _, group in groups]


def verify_clock(sequence, minimum=.5):
    spans = [(group[-1]["motion"]["time"] - group[0]["motion"]["time"], group[-1]["time"] - group[0]["time"])
             for group in epochs(sequence)]
    measured = [(sample, native) for sample, native in spans if native >= minimum]
    if not measured:
        raise ValueError("no sampled epoch lasted long enough to measure the native clock")
    for sample, native in measured:
        if abs(sample - native) > max(.05, .1 * native):
            raise ValueError(f"sample clock advanced {sample:.3f} s during {native:.3f} s of native time")


def verify_dynamics(frame, channels):
    if any(frame["motion"]["channels"][name]["dynamics"] != channels[name]["dynamics"] for name in CHANNELS):
        raise ValueError("native motion replaced the authored dynamics")


def verify_retarget(earlier, previous, retargeted, tolerance=(.02, .1)):
    instant = retargeted["motion"].get("retargetTime")
    if type(instant) not in (int, float) or not previous["motion"]["time"] <= instant < float("inf"):
        raise ValueError(f"retarget instant {instant!r} does not follow the previous sample")
    # A frame is sampled only after the frame before it was published and logged, and logged only after its
    # own sample, so log lag can loosen this bound but never tighten it. Date.now() truncates to milliseconds.
    reached = retargeted["time"] - retargeted["motion"]["time"] - earlier["time"] + .002
    if instant - previous["motion"]["time"] > reached:
        raise ValueError(f"retarget instant {instant!r} lies beyond the {reached:.3f} s logged after the earlier frame")
    for name in CHANNELS:
        expected = equation_sample(previous["motion"]["channels"][name],
                                   previous["motion"]["motion"]["target"][name], instant, False)["state"]
        initial = retargeted["motion"]["channels"][name]["initial"]
        if abs(initial["position"] - expected["position"]) > tolerance[0] \
                or abs(initial["velocity"] - expected["velocity"]) > tolerance[1]:
            raise ValueError(f"{name} retarget did not continue from the reached state: {initial} versus {expected}")
    if not any(retargeted["motion"]["channels"][name]["initial"]["velocity"] for name in CHANNELS):
        raise ValueError("release retarget discarded retained velocity")


class Motion:
    def __init__(self, session):
        self.session = session

    def frames(self):
        return frames(self.session.events())

    def wait_frames(self, start, predicate, description, timeout=10):
        return self.session.wait(lambda: [frame for frame in self.frames()[start:] if predicate(frame)],
                                 description, timeout)

    def index(self, start, predicate, description):
        found = self.wait_frames(start, predicate, description)[0]
        return self.frames().index(found, start)

    def settle(self, description):
        self.session.wait(lambda: self.frames() and settled(self.frames()[-1]), description, timeout=30)


def spring_cases(session, motion, requests):
    results = []
    session.input.move(*OUTSIDE)
    session.reset("motion", True)
    session.settle(lambda current: not current["hovered"], "pointer outside the Command")
    motion.settle("motion settled before the press")
    start = len(motion.frames())
    before = len(effects(session.events()))
    session.input.key(KEY_SPACE, True)
    session.wait(lambda: len([frame for frame in motion.frames()[start:]
                              if moving(frame) and frame["state"]["hold"]]) >= 3,
                 "several unsettled pressed spring frames", timeout=10)
    release = len(motion.frames())
    session.input.key(KEY_SPACE, False)
    expect_effects(session, before, 1, "sampled Command activation")
    first = motion.index(release, lambda frame: not frame["state"]["hold"], "retargeted release frame")
    events = session.events()
    position = effect_positions(events)[before]
    split = verify_activation(events, position, start, requests)
    verify_committed_frame(events, position, split)
    if split != first:
        raise ValueError(f"the retargeted release frame is frame {first}, not frame {split}, the first after the "
                         "activation effect")
    recorded = motion.frames()
    verify_retarget(*recorded[first - 2:first + 1])
    motion.settle("both channels settled")
    sequence = motion.frames()[start:]
    if not any(frame["motion"]["motion"]["bodyMix"]["settled"] != frame["motion"]["motion"]["depthScale"]["settled"]
               for frame in sequence):
        raise ValueError("no frame showed one channel settled while the other remained scheduled")
    verify_clock(sequence)
    count = len(motion.frames())
    session.quiet(.5)
    if len(motion.frames()) != count:
        raise ValueError("native frame scheduling continued after both channels settled")
    results.append("pressed and released spring frames continue across the retarget, track native time and settle; "
                   "the activation callback observes a withdrawn frame and the next frame presents committed state")

    motion.settle("motion settled before the held press")
    start = len(motion.frames())
    before = len(effects(session.events()))
    session.input.key(KEY_SPACE, True)
    held = session.settle(lambda current: (current["state"]["hold"] or {}).get("key") == "space" and current["ready"],
                          "space hold with a published frame")
    session.quiet(1.2)
    current = session.current()
    if current["publications"] - held["publications"] < 2:
        raise ValueError("a held press with motion delivered fewer than 2 auto-repeat publications")
    if current["retractions"] != held["retractions"] or not current["ready"]:
        raise ValueError("auto-repeat during a held press with motion retracted or unpublished the Command frame")
    held_frames = [frame for frame in motion.frames()[start:] if frame["state"]["hold"]]
    # The authored test profile is still moving after the compositor's repeat delay, so held frames span repeats.
    if not held_frames or held_frames[-1]["time"] - held_frames[0]["time"] < 1:
        raise ValueError("held spring frames stopped while auto-repeat continued")
    if len(epochs(held_frames)) != 1:
        raise ValueError("auto-repeat restarted the motion clock during a held press")
    session.input.key(KEY_SPACE, False)
    expect_effects(session, before, 1, "held sampled Command activation")
    motion.settle("motion settled after the held press")
    events = session.events()
    position = effect_positions(events)[before]
    verify_committed_frame(events, position, verify_activation(events, position, start, requests))
    results.append("auto-repeat during a held press keeps one clock epoch and publishes frames without retraction")

    start = len(motion.frames())
    session.point(session.current())
    entered = motion.index(start, lambda frame: frame["hovered"] and moving(frame), "hover spring frames")
    session.input.move(*OUTSIDE)
    motion.wait_frames(entered + 1, lambda frame: not frame["hovered"] and moving(frame), "hover exit spring frames")
    motion.settle("hover motion settled")
    results.append("compositor hover enters and then leaves through spring frames")

    session.point(session.current())
    start = len(motion.frames())
    motion.wait_frames(start, lambda frame: frame["hovered"] and moving(frame), "hover spring frames before focus loss")
    change = len(motion.frames())
    session.call("focusOther")
    motion.wait_frames(change, lambda frame: not frame["state"]["focused"] and frame["hovered"],
                       "frames after focus loss during motion")
    session.input.move(*OUTSIDE)
    motion.settle("motion settled after focus loss")
    results.append("focus loss during motion republishes frames without the focus cue")
    return results


def interruption_cases(session, motion):
    results = []
    for name, change, marker, expectation in (
            ("reduced-motion preference", lambda: session.call("configure", "reduced"),
             lambda event: event["kind"] == "configure" and event["configuration"] == "reduced",
             lambda frame: frame["configuration"] == "reduced"),
            ("availability withdrawal", lambda: session.call("availability", "false"),
             lambda event: event["kind"] == "availability" and event["enabled"] is False,
             lambda frame: not frame["state"]["enabled"])):
        session.reset("motion", True)
        start = len(motion.frames())
        before = len(effects(session.events()))
        session.input.key(KEY_SPACE, True)
        motion.wait_frames(start, lambda frame: moving(frame) and frame["state"]["hold"], f"press before {name}")
        logged = len(session.events())
        change()
        motion.wait_frames(start, expectation, f"frames after {name}")
        session.input.key(KEY_SPACE, False)
        expect_effects(session, before, 0, f"Space released after {name}")
        motion.settle(f"motion settled after {name}")
        events = session.events()
        requested = [index for index, event in enumerate(events) if index >= logged and marker(event)]
        if not requested:
            raise ValueError(f"the probe did not log the {name} request")
        after = verify_superseding_frames(events, requested[0], expectation, name)
        if any(frame["state"]["hold"] for frame in after):
            raise ValueError(f"{name} during a press retained the held gesture")
        if name.startswith("reduced") and any(frame["motion"]["motion"]["policy"] != "reducedMotion"
                                              or not settled(frame) for frame in after):
            raise ValueError("reduced-motion preference retained sampled motion")
        results.append(f"{name} during a press cancels the gesture; the first frame after it reflects the change "
                       "and no frame for the earlier request follows")

    session.reset("motion", True)
    start = len(motion.frames())
    before = len(effects(session.events()))
    session.input.key(KEY_SPACE, True)
    motion.wait_frames(start, lambda frame: moving(frame) and frame["state"]["hold"], "press before the layout change")
    origin = session.current()["target"]["x"]
    moved = len(motion.frames())
    session.call("setCommandX", "90")
    session.settle(lambda current: current["state"]["hold"] is None, "held gesture cancelled by the layout change")
    session.input.key(KEY_SPACE, False)
    expect_effects(session, before, 0, "Space released after the layout change")
    motion.wait_frames(moved, lambda frame: frame["center"] and abs(frame["center"]["x"] - origin - 50) < 1e-6,
                       "frames published at the moved position")
    results.append("layout change during a press cancels the gesture and republishes frames at the new position")
    return results


CALLBACK_REACTIONS = (
    ("replace", "replacing the configuration", 1, lambda frame: frame["configuration"] == "replacement",
     "still presents the earlier configuration"),
    ("unavailable", "withdrawing availability", 1, lambda frame: not frame["state"]["enabled"], "is still enabled"),
    ("invoke", "invoking the control", 2, lambda frame: True, ""),
)


def reentrancy_cases(session, motion, requests):
    results = []
    for mode, reaction, count, expectation, problem in CALLBACK_REACTIONS:
        session.input.move(*OUTSIDE)
        session.reset("motion", True)
        session.settle(lambda current: not current["hovered"], "pointer outside the Command")
        motion.settle(f"motion settled before the {mode} callback")
        start = len(motion.frames())
        before = len(effects(session.events()))
        session.call("setReaction", mode)
        session.input.key(KEY_SPACE, True)
        motion.wait_frames(start, lambda frame: moving(frame) and frame["state"]["hold"],
                           f"pressed spring frames before the {mode} callback")
        session.input.key(KEY_SPACE, False)
        expect_effects(session, before, count, f"{mode} from a consumer callback with motion")
        motion.settle(f"motion settled after the {mode} callback")
        if mode != "replace" and session.current()["reactionResult"] is not True:
            raise ValueError(f"the {mode} reaction from a consumer callback was not delivered")
        events = session.events()
        positions = effect_positions(events)[before:before + count]
        verify_activation(events, positions[0], start, requests)
        for position in positions[1:]:
            verify_callback_frame(events, position, requests)
        later = frames(events[positions[-1] + 1:])
        if not later:
            raise ValueError(f"no frame was logged after the {mode} callback")
        for frame in later:
            if frame["state"]["hold"]:
                raise ValueError(f"frame {frame['sequence']} after the {mode} callback keeps the released hold")
            if not expectation(frame):
                raise ValueError(f"frame {frame['sequence']} after the {mode} callback {problem}")
            try:
                verify_frame(frame, requests)
            except ValueError as error:
                raise ValueError(f"frame {frame['sequence']} after the {mode} callback: {error}") from None
        results.append(f"an activation callback {reaction} with motion observes a withdrawn frame, and every "
                       "later frame presents the committed result")
    return results


def policy_cases(session, motion):
    results = []
    for configuration, policy in (("reduced", "reducedMotion"), ("cast", "castImmediate")):
        session.reset(configuration, True)
        start = len(motion.frames())
        session.input.key(KEY_SPACE, True)
        pressed = motion.wait_frames(start, lambda frame: frame["state"]["hold"], f"{policy} pressed frame")
        if any(frame["motion"]["motion"]["policy"] != policy or not settled(frame) for frame in pressed):
            raise ValueError(f"{policy} published sampled motion")
        session.input.key(KEY_SPACE, False)
        session.quiet()
        results.append(f"{policy} publishes immediate settled frames")

    session.reset("frost", True)
    start = len(motion.frames())
    session.input.key(KEY_SPACE, True)
    sample = motion.wait_frames(start, moving, "Frost spring frames with reduced transparency")[0]
    if sample["snapshot"]["paint"]["paint"]["body"]["materialFamily"] != "frost":
        raise ValueError("reduced-transparency Frost lost its bound family")
    session.input.key(KEY_SPACE, False)
    session.quiet()
    results.append("reduced-transparency Frost keeps its family and spring policy")
    return results


def live_scale_case(session, motion, requests, compositor, initial, final):
    session.input.move(*OUTSIDE)
    session.reset("motion", True)
    session.settle(lambda current: not current["hovered"], "pointer outside the Command")
    motion.settle("motion settled before the scale change")
    before = session.current()
    reference = motion.frames()[-1]
    change = len(motion.frames())
    logged = len(session.events())
    compositor.set_scale(final)
    after = session.until(lambda current: current["ready"] and current["target"] and current["deviceScale"] == final,
                          f"ready Command at device scale {final:g}", 20)
    republished = motion.wait_frames(change, lambda frame: frame["deviceScale"] == final,
                                     f"a complete frame at device scale {final:g}")[0]
    for channel in ("hitRegion", "accessibility"):
        if republished["snapshot"][channel] != reference["snapshot"][channel]:
            raise ValueError(f"the {channel} published at device scale {final:g} differs from scale {initial:g}")
    if after["target"] != before["target"] or republished["center"] != reference["center"]:
        raise ValueError(f"the native target moved when the device scale changed: {before['target']} versus "
                         f"{after['target']}")
    session.input.width, session.input.height = after["screen"]["width"], after["screen"]["height"]
    session.call("focusCommand")
    session.settle(lambda current: focused(current) and current["state"]["focused"],
                   f"observed Command focus at device scale {final:g}")
    start = len(motion.frames())
    before_effects = len(effects(session.events()))
    session.input.key(KEY_SPACE, True)
    session.wait(lambda: len([frame for frame in motion.frames()[start:]
                              if moving(frame) and frame["state"]["hold"]]) >= 3,
                 f"several unsettled pressed spring frames at device scale {final:g}", timeout=10)
    session.input.key(KEY_SPACE, False)
    expect_effects(session, before_effects, 1, f"Command activation at device scale {final:g}")
    motion.settle(f"motion settled at device scale {final:g}")
    events = session.events()
    position = effect_positions(events)[before_effects]
    verify_committed_frame(events, position, verify_activation(events, position, start, requests))
    verify_clock(motion.frames()[start:])
    return logged, (f"a live output scale change from {initial:g} to {final:g} republishes a complete frame with the "
                    "same target and accessible node, and Space then publishes spring frames that track native time "
                    "and settle at the new scale; its activation callback observes no superseded frame")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--module-path", type=Path, required=True)
    parser.add_argument("--font", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--scale", type=float, required=True, help="Output scale the compositor presents")
    parser.add_argument("--live-scale", type=float,
                        help="Output scale Sway changes to while the shell runs, after the other cases")
    args = parser.parse_args()
    session = None
    try:
        live = changed_scale(args.scale, args.live_scale)
        host, module, font = (path.resolve(strict=True) for path in (args.host, args.module_path, args.font))
        if not os.environ.get("WAYLAND_DISPLAY") or not os.environ.get("XDG_RUNTIME_DIR"):
            raise ValueError("requires a live Wayland compositor session with layer-shell and virtual input")
        environment = dict(os.environ, RESINA_QUICKSHELL_HOST=str(host), RESINA_QT_RUNTIME_MODULE=str(module),
                           QT_QPA_PLATFORM="offscreen")
        verify_version(subprocess.run([sys.executable, str(ROOT / "tools/run_qt_availability_probe.py"), "--version"],
                                      env=environment, capture_output=True, text=True, timeout=15))
        args.output_dir.mkdir(parents=True, exist_ok=False)
        session = Session(host, module, font, args.output_dir)
        verify_scale(session.current(), args.scale)
        compositor = Sway(os.environ) if live is not None else None
        motion = Motion(session)
        requests = authored_requests()
        results = spring_cases(session, motion, requests)
        results += interruption_cases(session, motion)
        results += reentrancy_cases(session, motion, requests)
        results += policy_cases(session, motion)
        change = None
        if live is not None:
            change, result = live_scale_case(session, motion, requests, compositor, args.scale, live)
            results.append(result)
        recorded = motion.frames()
        verify_publication_scales(session.events(), args.scale, change, live)
        for frame in recorded:
            verify_dynamics(frame, requests["channels"])
            verify_frame(frame, requests)
        (args.output_dir / "report.json").write_text(
            json.dumps({"scale": args.scale, "liveScale": live, "cases": results, "frames": len(recorded)}, indent=2),
            encoding="utf-8")
        for result in results:
            print("PASS " + result)
        scales = f"{args.scale:g}" + (f" then {live:g}" if live is not None else "")
        print(f"PASS {len(recorded)} native frames at device scale {scales} keep the authored dynamics and match "
              "independent equations and complete snapshots")
    except (OSError, ValueError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"Quickshell Command motion check failed: {error}", file=sys.stderr)
        return 1
    finally:
        if session:
            session.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
