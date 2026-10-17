"""Measure Command spring frame cost in released Quickshell and enforce the motion frame-rate floor."""
import argparse
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import sys

from check_qt_command_snapshot_runtime import ROOT
from check_quickshell_command_lifecycle import OUTSIDE, Session
from check_quickshell_command_motion import Motion, epochs, moving
from check_quickshell_command_updates import verify_version
from wayland_virtual_input import KEY_SPACE

FLOOR_INTERVAL = 1 / 30
TARGET_INTERVAL = .017
TARGET_PREPARATION = .008
MINIMUM_FRAMES = 20


def seconds(value, name):
    if type(value) not in (int, float) or not math.isfinite(value) or value < 0:
        raise ValueError(f"{name} {value!r} is not a finite non-negative number of seconds")
    return value


def verify_scale(screen, expected):
    observed = screen.get("deviceScale") if screen else None
    if type(observed) not in (int, float) or observed != expected:
        raise ValueError(f"the Command is presented at device scale {observed!r}, not {expected!r}")


def continuations(sequence):
    groups = {id(frame): index for index, group in enumerate(epochs(sequence)) for frame in group}
    measured = []
    for previous, frame in zip(sequence, sequence[1:]):
        if not moving(previous) or frame["motion"]["motion"]["policy"] != "spring":
            continue
        before, after = (seconds(item["motion"]["time"], "sample time") for item in (previous, frame))
        if groups[id(previous)] == groups[id(frame)]:
            if frame["motion"]["retargetTime"] != previous["motion"]["retargetTime"] or after <= before:
                raise ValueError(f"sample clock restarted between {before!r} and {after!r} without changing the motion")
            interval = after - before
        elif frame["motion"]["retargetTime"] is None:
            continue
        else:
            instant = seconds(frame["motion"]["retargetTime"], "retarget instant")
            if instant < before:
                raise ValueError(f"retarget instant {instant!r} precedes the previous sample {before!r}")
            interval = instant - before + after
        measured.append({"interval": interval,
                         "preparation": seconds(frame["motion"].get("preparationDuration"), "preparation duration")})
    return measured


def summary(values):
    ordered = sorted(values)
    return {"median": statistics.median(ordered), "p90": ordered[math.ceil(.9 * len(ordered)) - 1],
            "max": ordered[-1]}


def evaluate(measured, minimum=MINIMUM_FRAMES):
    if len(measured) < minimum:
        raise ValueError(f"only {len(measured)} continued spring frames were published; {minimum} are required")
    interval = summary([item["interval"] for item in measured])
    preparation = summary([item["preparation"] for item in measured])
    floor = interval["median"] <= FLOOR_INTERVAL
    return {"frames": len(measured), "interval": interval, "preparation": preparation,
            "floor": {"interval": FLOOR_INTERVAL, "met": floor},
            "target": {"interval": TARGET_INTERVAL, "preparation": TARGET_PREPARATION,
                       "met": interval["median"] <= TARGET_INTERVAL and preparation["median"] <= TARGET_PREPARATION}}


def measure(session, motion, presses, frames_per_phase):
    session.input.move(*OUTSIDE)
    session.reset("motion", True)
    session.settle(lambda current: not current["hovered"], "pointer outside the Command")
    start = len(motion.frames())
    for press in range(presses):
        for pressed in (True, False):
            phase = len(motion.frames())
            session.input.key(KEY_SPACE, pressed)
            session.wait(lambda: len([frame for frame in motion.frames()[phase:]
                                      if moving(frame) and bool(frame["state"]["hold"]) == pressed])
                         >= frames_per_phase,
                         f"{frames_per_phase} {'pressed' if pressed else 'released'} spring frames of press "
                         f"{press + 1}", timeout=60)
    return motion.frames()[start:]


def describe(name, values, unit=1000):
    return f"{name} median {values['median'] * unit:.1f} ms, p90 {values['p90'] * unit:.1f} ms, " \
           f"max {values['max'] * unit:.1f} ms"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--module-path", type=Path, required=True)
    parser.add_argument("--font", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--scale", type=float, required=True, help="Output scale the compositor presents")
    parser.add_argument("--presses", type=int, default=10)
    parser.add_argument("--frames-per-phase", type=int, default=6)
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
        verify_scale(session.current()["screen"], args.scale)
        motion = Motion(session)
        measured = continuations(measure(session, motion, args.presses, args.frames_per_phase))
        (args.output_dir / "report.json").write_text(json.dumps({"scale": args.scale, "measured": measured}),
                                                     encoding="utf-8")
        result = evaluate(measured)
        (args.output_dir / "report.json").write_text(
            json.dumps(dict(result, scale=args.scale, measured=measured), indent=2), encoding="utf-8")
        print(f"MEASURED scale {args.scale:g}: {result['frames']} continued spring frames; "
              + describe("published interval", result["interval"]) + "; "
              + describe("synchronous preparation", result["preparation"]))
        print(("MET" if result["target"]["met"] else "NOT MET") + " target: median preparation at most "
              f"{TARGET_PREPARATION * 1000:g} ms and median interval at most {TARGET_INTERVAL * 1000:g} ms "
              "(reported; not enforced while frames wait for the next animation tick after publication)")
        if not result["floor"]["met"]:
            raise ValueError(f"median published interval {result['interval']['median'] * 1000:.1f} ms exceeds the "
                             f"{FLOOR_INTERVAL * 1000:.1f} ms (30 Hz) motion floor")
        print(f"PASS median published interval meets the {FLOOR_INTERVAL * 1000:.1f} ms (30 Hz) motion floor")
    except (OSError, ValueError, KeyError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"Quickshell Command motion budget check failed: {error}", file=sys.stderr)
        return 1
    finally:
        if session:
            session.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
