"""Verify sampled Command motion and preference fallback with compositor-delivered input in released Quickshell."""
import argparse
import copy
import json
import os
from pathlib import Path
import subprocess
import sys

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


def verify_clock(sequence):
    pairs = [(a, b) for a, b in zip(sequence, sequence[1:])
             if a["motion"]["channels"] == b["motion"]["channels"]
             and a["motion"]["motion"]["target"] == b["motion"]["motion"]["target"]]
    if not any(b["motion"]["time"] > a["motion"]["time"] for a, b in pairs):
        raise ValueError("native sampled clock did not advance")
    for a, b in pairs:
        if abs((b["motion"]["time"] - a["motion"]["time"]) - (b["time"] - a["time"])) > .25:
            raise ValueError("sample clock does not track elapsed native time")


class Motion:
    def __init__(self, session):
        self.session = session

    def frames(self):
        return frames(self.session.events())

    def wait_frames(self, start, predicate, description, timeout=10):
        return self.session.wait(lambda: [frame for frame in self.frames()[start:] if predicate(frame)],
                                 description, timeout)


def spring_cases(session, motion):
    results = []
    session.reset("motion", True)
    start = len(motion.frames())
    before = len(effects(session.events()))
    session.input.key(KEY_SPACE, True)
    pressing = motion.wait_frames(start, lambda frame: moving(frame) and frame["state"]["hold"],
                                  "unsettled spring frames while pressed")
    motion.session.wait(lambda: len([frame for frame in motion.frames()[start:] if moving(frame)]) >= 3,
                        "multiple native pressed frames", timeout=10)
    release = len(motion.frames())
    session.input.key(KEY_SPACE, False)
    expect_effects(session, before, 1, "sampled Command activation")
    retargeted = motion.wait_frames(release, lambda frame: not frame["state"]["hold"],
                                    "retargeted release frames")
    if not any(any(abs(frame["motion"]["channels"][name]["initial"]["velocity"]) > 0 for name in CHANNELS)
               for frame in retargeted):
        raise ValueError("release retarget discarded retained velocity")
    session.wait(lambda: motion.frames() and settled(motion.frames()[-1]), "both channels settled", timeout=30)
    sequence = motion.frames()[start:]
    if not any(frame["motion"]["motion"]["bodyMix"]["settled"] != frame["motion"]["motion"]["depthScale"]["settled"]
               for frame in sequence):
        raise ValueError("no frame showed one channel settled while the other remained scheduled")
    verify_clock(sequence)
    count = len(motion.frames())
    session.quiet(.5)
    if len(motion.frames()) != count:
        raise ValueError("native frame scheduling continued after both channels settled")
    if not pressing:
        raise ValueError("no pressed spring frame was published")
    results.append("pressed and released spring frames follow the independent equations and settle")

    start = len(motion.frames())
    observed = session.current()
    session.point(observed)
    motion.wait_frames(start, lambda frame: frame["hovered"] and moving(frame), "hover spring frames")
    session.input.move(*OUTSIDE)
    motion.wait_frames(start, lambda frame: not frame["hovered"] and moving(frame), "hover exit spring frames")
    session.wait(lambda: settled(motion.frames()[-1]), "hover motion settled", timeout=30)
    results.append("compositor hover enters and leaves through spring frames")
    return results


def interruption_cases(session, motion):
    results = []
    session.reset("motion", True)
    start = len(motion.frames())
    session.input.key(KEY_SPACE, True)
    motion.wait_frames(start, moving, "spring frames before the preference change")
    change = len(motion.frames())
    session.call("configure", "reduced")
    reduced = motion.wait_frames(change, lambda frame: frame["configuration"] == "reduced",
                                 "frames after reduced-motion preference")
    if any(frame["motion"]["motion"]["policy"] != "reducedMotion" or not settled(frame) for frame in reduced):
        raise ValueError("reduced-motion preference retained sampled motion")
    session.input.key(KEY_SPACE, False)
    session.quiet()
    results.append("reduced-motion preference during a press switches to immediate settled frames")

    session.reset("motion", True)
    start = len(motion.frames())
    session.input.key(KEY_SPACE, True)
    motion.wait_frames(start, moving, "spring frames before the layout change")
    origin = session.current()["target"]["x"]
    before = len(effects(session.events()))
    moved = len(motion.frames())
    session.call("setCommandX", "90")
    session.settle(lambda current: current["state"]["hold"] is None, "held gesture cancelled by the layout change")
    session.input.key(KEY_SPACE, False)
    expect_effects(session, before, 0, "Space released after the layout change")
    session.settle(lambda current: current["ready"] and current["target"]
                   and abs(current["target"]["x"] - origin - 50) < 1e-6, "target moved with the layout")
    motion.wait_frames(moved, lambda frame: True, "frames after the layout change")
    results.append("layout change during motion cancels the gesture and republishes guarded frames")
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
        results = []
        for cases in (spring_cases, interruption_cases, policy_cases):
            results += cases(session, motion)
        requests = authored_requests()
        recorded = motion.frames()
        for frame in recorded:
            verify_frame(frame, requests)
        (args.output_dir / "report.json").write_text(json.dumps({"cases": results, "frames": len(recorded)}, indent=2),
                                                     encoding="utf-8")
        for result in results:
            print("PASS " + result)
        print(f"PASS {len(recorded)} native frames match independent equations and complete snapshots")
    except (OSError, ValueError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"Quickshell Command motion check failed: {error}", file=sys.stderr)
        return 1
    finally:
        if session:
            session.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
