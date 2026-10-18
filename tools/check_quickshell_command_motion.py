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
from check_quickshell_command_lifecycle import OUTSIDE, Session, authored_requests, expect_effects, effects
from check_quickshell_command_updates import verify_version
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


def spring_cases(session, motion):
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
    results.append("pressed and released spring frames continue across the retarget, track native time and settle")

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
    for name, change, expectation in (
            ("reduced-motion preference", lambda: session.call("configure", "reduced"),
             lambda frame: frame["configuration"] == "reduced"),
            ("availability withdrawal", lambda: session.call("availability", "false"),
             lambda frame: not frame["state"]["enabled"])):
        session.reset("motion", True)
        start = len(motion.frames())
        before = len(effects(session.events()))
        session.input.key(KEY_SPACE, True)
        motion.wait_frames(start, lambda frame: moving(frame) and frame["state"]["hold"], f"press before {name}")
        index = len(motion.frames())
        change()
        after = motion.wait_frames(index, expectation, f"frames after {name}")
        if any(frame["state"]["hold"] for frame in after):
            raise ValueError(f"{name} during a press retained the held gesture")
        if name.startswith("reduced") and any(frame["motion"]["motion"]["policy"] != "reducedMotion"
                                              or not settled(frame) for frame in after):
            raise ValueError("reduced-motion preference retained sampled motion")
        session.input.key(KEY_SPACE, False)
        expect_effects(session, before, 0, f"Space released after {name}")
        results.append(f"{name} during a press cancels the gesture and republishes guarded frames")

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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--module-path", type=Path, required=True)
    parser.add_argument("--font", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    session = None
    try:
        host, module, font = (path.resolve(strict=True) for path in (args.host, args.module_path, args.font))
        if not os.environ.get("WAYLAND_DISPLAY") or not os.environ.get("XDG_RUNTIME_DIR"):
            raise ValueError("requires a live Wayland compositor session with layer-shell and virtual input")
        environment = dict(os.environ, RESINA_QUICKSHELL_HOST=str(host), RESINA_QT_RUNTIME_MODULE=str(module),
                           QT_QPA_PLATFORM="offscreen")
        verify_version(subprocess.run([sys.executable, str(ROOT / "tools/run_qt_availability_probe.py"), "--version"],
                                      env=environment, capture_output=True, text=True, timeout=15))
        args.output_dir.mkdir(parents=True, exist_ok=False)
        session = Session(host, module, font, args.output_dir)
        motion = Motion(session)
        requests = authored_requests()
        results = spring_cases(session, motion)
        for cases in (interruption_cases, policy_cases):
            results += cases(session, motion)
        recorded = motion.frames()
        for frame in recorded:
            verify_dynamics(frame, requests["channels"])
            verify_frame(frame, requests)
        (args.output_dir / "report.json").write_text(json.dumps({"cases": results, "frames": len(recorded)}, indent=2),
                                                     encoding="utf-8")
        for result in results:
            print("PASS " + result)
        print(f"PASS {len(recorded)} native frames keep the authored dynamics and match independent equations "
              "and complete snapshots")
    except (OSError, ValueError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"Quickshell Command motion check failed: {error}", file=sys.stderr)
        return 1
    finally:
        if session:
            session.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
