"""Measure Command spring frame cost in released Quickshell and check the motion frame-rate floor and publication gap."""
import argparse
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time

from check_qt_command_snapshot_runtime import ROOT
from check_quickshell_command_lifecycle import OUTSIDE, Session, read_events
from check_quickshell_command_motion import Motion, epochs, frames, moving, verify_scale
from check_quickshell_command_updates import verify_version
from wayland_virtual_input import KEY_SPACE
import conformance_font

FLOOR_INTERVAL = 1 / 30
TAIL_INTERVALS = (FLOOR_INTERVAL, 1 / 24)
TARGET_INTERVAL = .017
TARGET_PREPARATION = .008
MINIMUM_FRAMES = 20
# The headless Sway output refreshes at 60 Hz. A predecessor that spanned this
# long from refresh start to publication saw at least one animation tick.
TICK = 1 / 60
GAP_PERCENTILE = .75
GAP_BOUND = .004
# A host whose preparation rarely outlasts a tick yields few qualifying frames per press, so the measured phase
# continues past the minimum presses until the gap has enough frames, this many presses have run or the time
# budget is spent. The budget plus the overshoot of one stride must fit the CI step timeout for both scales.
MAX_PRESSES = 600
MAX_SECONDS = 240
# Counting qualifying frames walks every frame, so it runs once per stride of presses.
CHECK_STRIDE = 5


class AppendedMotion(Motion):
    """Motion frames parsed once from the shell log, so long measurements do not reread it for every poll."""

    def __init__(self, session):
        super().__init__(session)
        self.offset = 0
        self.logged = []

    def frames(self):
        with self.session.log.open("rb") as log:
            log.seek(self.offset)
            appended = log.read()
        complete = appended[:appended.rfind(b"\n") + 1]
        self.offset += len(complete)
        self.logged += frames(read_events(complete.decode("utf-8", errors="replace")))
        return self.logged


def seconds(value, name):
    if type(value) not in (int, float) or not math.isfinite(value) or value < 0:
        raise ValueError(f"{name} {value!r} is not a finite non-negative number of seconds")
    return value


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
            delay = frame["motion"].get("publicationDelay")
            delay = None if delay is None else seconds(delay, "publication delay")
            span = frame["motion"].get("predecessorSpan")
            span = None if span is None else seconds(span, "predecessor span")
            if (delay is None) != (span is None):
                raise ValueError("publication delay and predecessor span must be published together")
        elif frame["motion"]["retargetTime"] is None:
            continue
        else:
            instant = seconds(frame["motion"]["retargetTime"], "retarget instant")
            if instant < before:
                raise ValueError(f"retarget instant {instant!r} precedes the previous sample {before!r}")
            interval = instant - before + after
            delay = span = None
        measured.append({"interval": interval,
                         "preparation": seconds(frame["motion"].get("preparationDuration"), "preparation duration"),
                         "publicationDelay": delay, "predecessorSpan": span})
    return measured


def summary(values):
    ordered = sorted(values)
    return {"median": statistics.median(ordered), "p90": ordered[math.ceil(.9 * len(ordered)) - 1],
            "max": ordered[-1]}


def tail(intervals):
    ordered = sorted(intervals)
    longest = run = 0
    for interval in intervals:
        run = run + 1 if interval > FLOOR_INTERVAL else 0
        longest = max(longest, run)
    return {"p95": ordered[math.ceil(.95 * len(ordered)) - 1], "p99": ordered[math.ceil(.99 * len(ordered)) - 1],
            "max": ordered[-1],
            "above": [{"interval": bound, "share": sum(value > bound for value in intervals) / len(intervals)}
                      for bound in TAIL_INTERVALS],
            "longestRunAbove": {"interval": FLOOR_INTERVAL, "frames": longest}}


def qualifying(measured):
    return [item for item in measured if item["publicationDelay"] is not None and item["predecessorSpan"] >= TICK]


def evaluate(measured, minimum=MINIMUM_FRAMES):
    if len(measured) < minimum:
        raise ValueError(f"only {len(measured)} continued spring frames were published; {minimum} are required")
    interval = summary([item["interval"] for item in measured])
    preparation = summary([item["preparation"] for item in measured])
    floor = interval["median"] <= FLOOR_INTERVAL
    delays = sorted(item["publicationDelay"] for item in qualifying(measured))
    spans = [item["predecessorSpan"] for item in measured if item["predecessorSpan"] is not None]
    gap = {"frames": len(delays), "bound": GAP_BOUND, "evaluated": len(delays) >= minimum,
           "observed": len(spans), "longestSpan": max(spans, default=None)}
    gap["unexercised"] = not gap["evaluated"] and len(spans) >= minimum and gap["longestSpan"] < TICK
    if gap["evaluated"]:
        gap["delay"] = delays[math.ceil(GAP_PERCENTILE * len(delays)) - 1]
        gap["met"] = gap["delay"] <= GAP_BOUND
    return {"frames": len(measured), "interval": interval, "tail": tail([item["interval"] for item in measured]),
            "preparation": preparation,
            "floor": {"interval": FLOOR_INTERVAL, "met": floor}, "gap": gap,
            "target": {"interval": TARGET_INTERVAL, "preparation": TARGET_PREPARATION,
                       "met": interval["median"] <= TARGET_INTERVAL and preparation["median"] <= TARGET_PREPARATION}}


def measure(session, motion, presses, max_presses, frames_per_phase, budget=MAX_SECONDS):
    session.input.move(*OUTSIDE)
    session.reset("motion", True)
    session.settle(lambda current: not current["hovered"], "pointer outside the Command")
    start = len(motion.frames())
    press = 0
    deadline = time.monotonic() + budget
    while press < max_presses and not (
            press >= presses and (press - presses) % CHECK_STRIDE == 0
            and (len(qualifying(continuations(motion.frames()[start:]))) >= MINIMUM_FRAMES
                 or time.monotonic() > deadline)):
        for pressed in (True, False):
            phase = len(motion.frames())
            session.input.key(KEY_SPACE, pressed)
            session.wait(lambda: len([frame for frame in motion.frames()[phase:]
                                      if moving(frame) and bool(frame["state"]["hold"]) == pressed])
                         >= frames_per_phase,
                         f"{frames_per_phase} {'pressed' if pressed else 'released'} spring frames of press "
                         f"{press + 1}", timeout=60)
        press += 1
    return motion.frames()[start:], press


def describe(name, values, unit=1000):
    return f"{name} median {values['median'] * unit:.1f} ms, p90 {values['p90'] * unit:.1f} ms, " \
           f"max {values['max'] * unit:.1f} ms"


def describe_tail(scale, values):
    shares = ", ".join(f"{bound['share']:.1%} above {bound['interval'] * 1000:.1f} ms" for bound in values["above"])
    longest = values["longestRunAbove"]
    return (f"TAIL scale {scale:g}: published interval p95 {values['p95'] * 1000:.1f} ms, "
            f"p99 {values['p99'] * 1000:.1f} ms, max {values['max'] * 1000:.1f} ms; {shares}; longest run of "
            f"{longest['frames']} consecutive intervals above {longest['interval'] * 1000:.1f} ms "
            "(reported, not enforced)")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--module-path", type=Path, required=True)
    parser.add_argument("--font-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--scale", type=float, required=True, help="Output scale the compositor presents")
    parser.add_argument("--presses", type=int, default=10, help="Minimum number of press and release cycles")
    parser.add_argument("--max-presses", type=int, default=MAX_PRESSES,
                        help="Cycles after which measuring stops even if the publication gap lacks frames")
    parser.add_argument("--max-seconds", type=float, default=MAX_SECONDS,
                        help="Seconds after which measuring stops once the minimum has run, checked every five cycles")
    parser.add_argument("--frames-per-phase", type=int, default=6)
    parser.add_argument("--floor", choices=("enforce", "report"), default="enforce",
                        help="Report the 30 Hz floor without failing, for hosts that do not qualify frame timing")
    parser.add_argument("--require-gap", action="store_true",
                        help="Fail when too few frames exercise the publication path to bound its gap")
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
        session = Session(host, module, fonts, args.output_dir)
        verify_scale(session.current(), args.scale)
        motion = AppendedMotion(session)
        frames, presses = measure(session, motion, args.presses, max(args.presses, args.max_presses),
                                  args.frames_per_phase, args.max_seconds)
        measured = continuations(frames)
        (args.output_dir / "report.json").write_text(
            json.dumps({"scale": args.scale, "presses": presses, "measured": measured}), encoding="utf-8")
        result = evaluate(measured)
        (args.output_dir / "report.json").write_text(
            json.dumps(dict(result, scale=args.scale, presses=presses, measured=measured), indent=2), encoding="utf-8")
        print(f"MEASURED scale {args.scale:g}: {result['frames']} continued spring frames over {presses} presses; "
              + describe("published interval", result["interval"]) + "; "
              + describe("synchronous preparation", result["preparation"]))
        print(describe_tail(args.scale, result["tail"]))
        print(("MET" if result["target"]["met"] else "NOT MET") + " target: median preparation at most "
              f"{TARGET_PREPARATION * 1000:g} ms and median interval at most {TARGET_INTERVAL * 1000:g} ms "
              "(reported, not enforced)")
        gap = result["gap"]
        if gap["evaluated"]:
            print(f"MEASURED publication gap: 75th percentile {gap['delay'] * 1000:.2f} ms from publication to the "
                  f"next preparation over {gap['frames']} frames whose predecessor spanned an animation tick")
        elif gap["unexercised"]:
            print(f"NOT APPLICABLE publication gap: none of {gap['observed']} predecessors spanned an animation "
                  f"tick (longest {gap['longestSpan'] * 1000:.1f} ms against {TICK * 1000:.1f} ms), so no frame "
                  "could wait for a tick it had not seen")
        elif args.require_gap:
            raise ValueError(f"only {gap['frames']} frames had a predecessor that spanned an animation tick after "
                             f"{presses} press cycles; {MINIMUM_FRAMES} are "
                             "required to bound the publication gap")
        else:
            print(f"NOT EVALUATED publication gap: only {gap['frames']} frames had a predecessor that spanned an "
                  f"animation tick; {MINIMUM_FRAMES} are required")
        if gap["evaluated"] and not gap["met"]:
            raise ValueError(f"the next preparation started {gap['delay'] * 1000:.2f} ms after publication at the "
                             f"75th percentile, beyond {GAP_BOUND * 1000:g} ms; frames wait for another animation tick")
        floor = (f"median published interval {result['interval']['median'] * 1000:.1f} ms against the "
                 f"{FLOOR_INTERVAL * 1000:.1f} ms (30 Hz) motion floor")
        if result["floor"]["met"]:
            print(f"PASS {floor}")
        elif args.floor == "report":
            print(f"NOT MET {floor} (reported, not enforced on this host)")
        else:
            raise ValueError(f"{floor} is not met")
    except (OSError, ValueError, KeyError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"Quickshell Command motion budget check failed: {error}", file=sys.stderr)
        return 1
    finally:
        if session:
            session.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
