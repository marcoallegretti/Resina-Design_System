"""Measure Toggle spring frame intervals in released Quickshell and check the motion frame-rate floor."""
import argparse
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time

from check_qt_toggle_control_runtime import ROOT, request_for
from check_qt_toggle_motion_runtime import fixture_channels
from check_quickshell_command_lifecycle import read_events
from check_quickshell_command_motion_budget import FLOOR_INTERVAL, describe, describe_tail, tail
from check_quickshell_command_updates import verify_version
from check_quickshell_toggle_lifecycle import ToggleSession
from wayland_virtual_input import KEY_SPACE
import conformance_font

MINIMUM_FRAMES = 20
MAX_SECONDS = 120
# Frames of one spring epoch are published back to back; a longer pause separates two epochs.
EPOCH_GAP = 0.25


class BudgetSession(ToggleSession):
    def requests(self):
        source = request_for("light")
        source["content"]["environment"]["accessibilityPreferences"]["reducedMotion"] = False
        return {**super().requests(), "standard": source, "channels": fixture_channels()}


class Publications:
    """Publication events parsed once from the shell log, so long measurements do not reread it for every poll."""

    def __init__(self, session):
        self.session, self.offset, self.logged = session, 0, []

    def all(self):
        with self.session.log.open("rb") as log:
            log.seek(self.offset)
            appended = log.read()
        complete = appended[:appended.rfind(b"\n") + 1]
        self.offset += len(complete)
        self.logged += [event for event in read_events(complete.decode("utf-8", errors="replace"))
                        if event["kind"] == "publication"]
        return self.logged


def seconds(value, name):
    if type(value) not in (int, float) or not math.isfinite(value) or value < 0:
        raise ValueError(f"{name} {value!r} is not a finite non-negative number of seconds")
    return value


def intervals(publications):
    """Seconds between consecutive published frames of one spring epoch, from the probe's frame clock."""
    measured = []
    for previous, frame in zip(publications, publications[1:]):
        if type(previous.get("moving")) is not bool or type(frame.get("moving")) is not bool:
            raise ValueError("a publication did not report whether its motion was moving")
        before, after = seconds(previous["frame"], "frame time"), seconds(frame["frame"], "frame time")
        if after < before:
            raise ValueError(f"the frame clock ran backwards from {before!r} to {after!r}")
        if previous["moving"] and frame["moving"] and after - before <= EPOCH_GAP:
            measured.append(after - before)
    return measured


def summary(values):
    ordered = sorted(values)
    return {"median": statistics.median(ordered), "p90": ordered[math.ceil(.9 * len(ordered)) - 1], "max": ordered[-1]}


def evaluate(measured, minimum=MINIMUM_FRAMES):
    if len(measured) < minimum:
        raise ValueError(f"only {len(measured)} continued spring frames were published; {minimum} are required")
    interval = summary(measured)
    return {"frames": len(measured), "interval": interval, "tail": tail(measured),
            "floor": {"interval": FLOOR_INTERVAL, "met": interval["median"] <= FLOOR_INTERVAL}}


def quiet(publications):
    """Wait until no frame has been published for a whole epoch gap."""
    count, since = len(publications.all()), time.monotonic()
    while time.monotonic() - since < EPOCH_GAP:
        time.sleep(0.05)
        if len(publications.all()) != count:
            count, since = len(publications.all()), time.monotonic()


def measure(session, publications, cycles, frames_per_phase, budget):
    session.reset()
    session.call("setMotion", "true")
    session.ready()
    session.call("focusToggle")
    session.settle(lambda current: current["nativeFocus"] and current["state"]["focused"], "focused moving Toggle")
    quiet(publications)
    start = len(publications.all())
    deadline = time.monotonic() + budget
    cycle = 0
    while cycle < cycles and time.monotonic() < deadline:
        for pressed in (True, False):
            phase = len(publications.all())
            session.input.key(KEY_SPACE, pressed)
            session.wait(lambda: sum(frame["moving"] for frame in publications.all()[phase:]) >= frames_per_phase,
                         f"{frames_per_phase} {'pressed' if pressed else 'released'} spring frames of cycle {cycle + 1}",
                         timeout=60)
        quiet(publications)
        cycle += 1
    return publications.all()[start:], cycle


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--module-path", type=Path, required=True)
    parser.add_argument("--font-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--scale", type=float, required=True, help="Output scale the compositor presents")
    parser.add_argument("--cycles", type=int, default=12, help="Press and release cycles to measure")
    parser.add_argument("--max-seconds", type=float, default=MAX_SECONDS,
                        help="Seconds after which measuring stops once a cycle has finished")
    parser.add_argument("--frames-per-phase", type=int, default=6)
    parser.add_argument("--floor", choices=("enforce", "report"), default="enforce",
                        help="Report the 30 Hz floor without failing, for hosts that do not qualify frame timing")
    args = parser.parse_args()
    session = None
    try:
        seconds(args.max_seconds, "--max-seconds")
        host, module = (path.resolve(strict=True) for path in (args.host, args.module_path))
        fonts = conformance_font.qml_sources(args.font_dir)
        if not os.environ.get("WAYLAND_DISPLAY") or not os.environ.get("XDG_RUNTIME_DIR"):
            raise ValueError("requires a live Wayland compositor session with layer-shell and virtual input")
        environment = dict(os.environ, RESINA_QUICKSHELL_HOST=str(host), RESINA_QT_RUNTIME_MODULE=str(module),
                           QT_QPA_PLATFORM="offscreen")
        verify_version(subprocess.run([sys.executable, str(ROOT / "tools/run_qt_availability_probe.py"), "--version"],
                                      env=environment, capture_output=True, text=True, timeout=15))
        args.output_dir.mkdir(parents=True, exist_ok=False)
        session = BudgetSession(host, module, fonts, args.output_dir)
        screen = session.current()["screen"]
        if not screen:
            raise ValueError("the Toggle has no screen")
        publications, cycles = measure(session, Publications(session), args.cycles, args.frames_per_phase,
                                       args.max_seconds)
        measured = intervals(publications)
        result = evaluate(measured)
        (args.output_dir / "report.json").write_text(
            json.dumps(dict(result, scale=args.scale, cycles=cycles, measured=measured), indent=2), encoding="utf-8")
        print(f"MEASURED scale {args.scale:g}: {result['frames']} continued spring frames over {cycles} cycles; "
              + describe("published interval", result["interval"]))
        print(describe_tail(args.scale, result["tail"]))
        floor = (f"median published interval {result['interval']['median'] * 1000:.1f} ms against the "
                 f"{FLOOR_INTERVAL * 1000:.1f} ms (30 Hz) motion floor")
        if result["floor"]["met"]:
            print(f"PASS {floor}")
        elif args.floor == "report":
            print(f"NOT MET {floor} (reported, not enforced on this host)")
        else:
            raise ValueError(f"{floor} is not met")
    except (OSError, ValueError, KeyError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"Quickshell Toggle motion budget check failed: {error}", file=sys.stderr)
        return 1
    finally:
        if session:
            session.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
