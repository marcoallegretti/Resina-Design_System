"""Verify compositor-delivered Toggle input, focus and lifetime in released Quickshell."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

from check_qt_toggle_control_runtime import ROOT, request_for
from check_quickshell_command_lifecycle import Session, effects, expect_effects, focused
from check_quickshell_command_updates import verify_version
from wayland_virtual_input import KEY_ENTER, KEY_SPACE, KEY_TAB

OUTSIDE = (760, 360)
KEYS = {"space": KEY_SPACE, "enter": KEY_ENTER}


def toggled(added, checked):
    for effect in added:
        if effect["checked"] is checked:
            raise ValueError("a consumer effect did not invert the committed checked value exactly once")
        checked = effect["checked"]
    return checked


def verify_publications(events):
    observed = {True: 0, False: 0}
    for event in events:
        if event["kind"] != "publication":
            continue
        if event["snapshotFocused"] is not event["nativeFocused"]:
            raise ValueError("a Toggle frame was published before native focus was synchronized")
        if not event["snapshotChecked"] is event["layoutChecked"] is event["checked"]:
            raise ValueError("a Toggle frame was published with a stale checked value")
        observed[event["nativeFocused"]] += 1
    return observed


def verify_pending(events, generation, change, then):
    pending = [event for event in events if event["kind"] == "pending" and event["generation"] == generation
               and event["change"] == change and event["then"] == then]
    if len(pending) != 1:
        raise ValueError(f"expected one {then} after the {change} change, observed {len(pending)}")
    event = pending[0]
    if event["accepted"] is not True or event["published"] or event["ready"]:
        raise ValueError(f"the {change} change did not leave its frame unpublished before {then}")


def outside_target(point, target):
    return abs(point[0] - target["x"]) > target["width"] / 2 or abs(point[1] - target["y"]) > target["height"] / 2


class ToggleSession(Session):
    component = "toggle"
    outside = OUTSIDE

    def requests(self):
        return {"standard": request_for("light")}

    def reset(self, checked=False):
        self.call("setReaction", "")
        for function in ("setWindowed", "setShown", "setMounted"):
            self.call(function, "true")
        self.call("setParented", "false")
        self.call("setFocusTaken", "false")
        if self.call("availability", "true") != "true" or self.call("setChecked", str(checked).lower()) != "true":
            raise ValueError("Toggle reset was rejected")
        self.ready()
        self.call("focusToggle")
        return self.settle(lambda current: focused(current) and current["state"]["focused"]
                           and current["checked"] is checked, "observed Toggle focus")

    def tap(self, key):
        self.input.key(key, True)
        self.input.key(key, False)

    def click(self, observed):
        self.point(observed)
        self.input.button(True)
        self.input.button(False)


def expect_toggles(session, before, count, checked, description):
    added = expect_effects(session, before, count, description)
    return added, toggled(added, checked)


def settle_checked(session, checked, description):
    session.settle(lambda current: current["checked"] is checked and current["state"]["hold"] is None,
                   description + " committed checked value")


def fresh_gesture(session, checked, name):
    current = session.reset(checked)
    before = len(effects(session.events()))
    session.tap(KEY_SPACE)
    fresh, _ = expect_toggles(session, before, 1, checked, f"fresh gesture after {name}")
    if fresh[0]["generation"] != current["generation"]:
        raise ValueError(f"a retired generation delivered the effect after {name}")


def activation_cases(session):
    results = []
    for checked in (False, True):
        observed = session.reset(checked)
        if not outside_target(OUTSIDE, observed["target"]):
            raise ValueError("the release point lies inside the Toggle target")
        before = len(effects(session.events()))
        session.point(observed)
        session.input.button(True)
        holding = session.settle(lambda current: (current["state"]["hold"] or {}).get("kind") == "pointer",
                                 "pointer hold")
        if not holding["state"]["hold"]["inside"] or holding["checked"] is not checked:
            raise ValueError("pointer hold did not start inside the target without toggling")
        session.input.button(False)
        _, now = expect_toggles(session, before, 1, checked, "pointer toggle")
        settle_checked(session, now, "pointer toggle")
        results.append(f"pointer toggle from {'on' if checked else 'off'}")

    observed = session.reset()
    before = len(effects(session.events()))
    session.point(observed)
    session.input.button(True)
    session.settle(lambda current: (current["state"]["hold"] or {}).get("inside") is True, "pointer hold")
    session.input.move(*OUTSIDE)
    session.settle(lambda current: (current["state"]["hold"] or {}).get("inside") is False, "pointer leaving target")
    session.input.button(False)
    expect_effects(session, before, 0, "pointer released outside")
    settle_checked(session, False, "pointer released outside")
    results.append("pointer press cancelled by moving off")

    for name, key in KEYS.items():
        session.reset()
        before = len(effects(session.events()))
        session.input.key(key, True)
        holding = session.settle(lambda current: (current["state"]["hold"] or {}).get("key") == name
                              and current["ready"], name + " hold with a published frame")
        session.quiet(1.2)
        repeated = session.current()["publications"] - holding["publications"]
        if repeated < 2:
            raise ValueError(f"{name} hold delivered {repeated} auto-repeat publications; at least 2 are required")
        current = session.current()
        retracted = current["retractions"] - holding["retractions"]
        if not current["ready"]:
            raise ValueError(f"{name} hold left no published frame after auto-repeat")
        if retracted:
            raise ValueError(f"{name} auto-repeat retracted the published Toggle frame {retracted} times")
        expect_effects(session, before, 0 if name == "space" else 1, name + " held with repeat")
        session.input.key(key, False)
        _, now = expect_toggles(session, before, 1, False, name + " toggle")
        settle_checked(session, now, name + " toggle")
        results.append(name + " toggle with repeat")

    observed = session.reset(True)
    if session.call("availability", "false") != "true":
        raise ValueError("availability withdrawal was rejected")
    observed = session.settle(lambda current: not current["state"]["enabled"], "unavailable Toggle")
    before = len(effects(session.events()))
    session.click(observed)
    if session.call("invoke") != "false":
        raise ValueError("an unavailable Toggle accepted semantic invocation")
    expect_effects(session, before, 0, "unavailable Toggle")
    settle_checked(session, True, "unavailable Toggle")
    results.append("unavailable Toggle rejects pointer and invocation")
    return results


def focus_cases(session):
    session.reset()
    session.tap(KEY_TAB)
    session.settle(lambda current: current["otherFocused"] and not current["nativeFocus"]["activeFocus"]
                   and not current["state"]["focused"], "traversal from Toggle to the next item")
    session.tap(KEY_TAB)
    session.settle(lambda current: focused(current) and current["state"]["focused"] and not current["otherFocused"],
                   "traversal into Toggle")
    session.call("focusOther")
    session.settle(lambda current: not current["state"]["focused"], "requested focus moved away")
    session.call("focusToggle")
    session.settle(lambda current: focused(current) and current["state"]["focused"], "requested focus observed")

    session.call("focusOther")
    session.call("setFocusTaken", "true")
    session.settle(lambda current: not current["nativeFocus"]["windowActive"], "keyboard focus taken by another surface")
    session.call("focusToggle")
    session.quiet()
    if session.current()["state"]["focused"]:
        raise ValueError("unobserved focus request was published as focus")
    session.call("setFocusTaken", "false")
    session.settle(lambda current: focused(current) and current["state"]["focused"],
                   "focus observed after keyboard focus returns")

    before = len(effects(session.events()))
    if session.call("availability", "false") != "true":
        raise ValueError("availability withdrawal was rejected")
    session.settle(lambda current: not current["state"]["enabled"] and not current["state"]["focused"],
                   "unavailable Toggle without focus")
    session.tap(KEY_SPACE)
    expect_effects(session, before, 0, "unavailable Toggle")
    results = ["traversal away and back", "requested focus", "failed focus request", "availability withdrawal"]

    for then, checked in (("blur", False), ("focus", True)):
        observed = session.reset(checked)
        if then == "focus":
            session.call("focusOther")
            observed = session.settle(lambda current: current["ready"] and not current["state"]["focused"]
                                      and current["otherFocused"], "ready unfocused Toggle")
        if session.call("changePending", "checked", then) != "true":
            raise ValueError(f"the checked change before {then} was rejected")
        session.settle(lambda current: current["ready"] and current["checked"] is not checked
                       and current["state"]["focused"] is (then == "focus")
                       and focused(current) is (then == "focus"), f"{then} while a frame is pending")
        verify_pending(session.events(), observed["generation"], "checked", then)
        results.append(f"{then} while a checked frame is pending")
    observed = verify_publications(session.events())
    if not observed[True] or not observed[False]:
        raise ValueError("focused and unfocused Toggle frames were not both published")
    return results + [f"{observed[True]} focused and {observed[False]} unfocused frames synchronized before publication"]


def withdraw(session):
    if session.call("availability", "false") != "true":
        raise ValueError("availability withdrawal was rejected")


INTERRUPTIONS = {
    "window deactivation": lambda session: session.call("setFocusTaken", "true"),
    "hidden surface": lambda session: session.call("setShown", "false"),
    "parent change": lambda session: session.call("setParented", "true"),
    "availability withdrawal": withdraw,
    "unmount": lambda session: session.call("setMounted", "false"),
    "closed surface": lambda session: session.call("setWindowed", "false"),
}
RETAINED = {"window deactivation", "hidden surface", "parent change", "availability withdrawal"}


def held(session, gesture, observed):
    if gesture == "space":
        session.input.key(KEY_SPACE, True)
    else:
        session.point(observed)
        session.input.button(True)
    session.settle(lambda current: current["state"]["hold"] is not None, gesture + " hold")


def release(session, gesture):
    if gesture == "space":
        session.input.key(KEY_SPACE, False)
    else:
        session.input.button(False)


def interruption_cases(session):
    results = []
    for gesture in ("space", "pointer"):
        for index, (name, interrupt) in enumerate(INTERRUPTIONS.items()):
            checked = index % 2 == 1
            observed = session.reset(checked)
            before = len(effects(session.events()))
            held(session, gesture, observed)
            interrupt(session)
            if name in RETAINED:
                session.settle(lambda current: current["state"]["hold"] is None, f"hold cancelled by {name}")
            session.quiet()
            release(session, gesture)
            expect_effects(session, before, 0, f"{gesture} held across {name}")
            if name in RETAINED and session.current()["checked"] is not checked:
                raise ValueError(f"{name} changed the checked value of a cancelled {gesture} gesture")
            fresh_gesture(session, checked, name)
            results.append(f"{gesture} held across {name}")
    return results


REACTIONS = {"check": 1, "invoke": 2, "unavailable": 1, "hide": 1, "unmount": 1, "close": 1}


def gesture(session, name, observed):
    if name == "pointer":
        session.click(observed)
    else:
        session.tap(KEYS[name])


def committed(effect, name):
    hold = effect["state"]["hold"]
    return effect["state"]["enabled"] and (hold is None or name == "enter" and hold.get("key") == "enter")


def reentrancy_cases(session):
    results = []
    for name in ("pointer", "space", "enter"):
        for index, (mode, count) in enumerate(REACTIONS.items()):
            checked = index % 2 == 0
            observed = session.reset(checked)
            before = len(effects(session.events()))
            session.call("setReaction", mode)
            gesture(session, name, observed)
            added, now = expect_toggles(session, before, count, checked, f"{mode} from consumer callback")
            if not all(committed(effect, name) for effect in added):
                raise ValueError(f"{mode} callback observed an uncommitted activation")
            if mode in ("check", "invoke", "unavailable"):
                current = session.settle(lambda current: current["state"]["hold"] is None, f"{mode} committed")
                if current["reactionResult"] is not True:
                    raise ValueError(f"{mode} from a consumer callback was rejected")
                expected = checked if mode == "check" else now
                settle_checked(session, expected, f"{mode} from consumer callback")
            if mode == "unavailable" and session.current()["state"]["enabled"]:
                raise ValueError("synchronous availability withdrawal was not committed")
            if mode == "hide":
                settle_checked(session, now, "hidden Toggle")
            fresh_gesture(session, checked, f"{mode} from a {name} callback")
            results.append(f"{mode} from consumer callback after {name} toggle")
    return results


def lifetime_cases(session):
    results = []
    for name in ("space", "enter", "pointer"):
        observed = session.reset()
        before = len(effects(session.events()))
        session.call("setReaction", "hideUnmount")
        gesture(session, name, observed)
        expect_toggles(session, before, 1, False, f"hide and unmount from a {name} callback")
        session.quiet()
        fresh_gesture(session, False, f"hide and unmount from a {name} callback")
        results.append(f"hide and unmount from consumer callback after {name} toggle")

    for gesture_name in ("space", "pointer"):
        for hard in (False, True):
            name = ("hard" if hard else "soft") + " reload"
            observed = session.reset(hard)
            before = len(effects(session.events()))
            held(session, gesture_name, observed)
            session.call("reloadShell", str(hard).lower())
            retired = observed["generation"]
            session.settle(lambda current: current["generation"] != retired,
                           f"previous generation retired after {name}")
            session.quiet()
            release(session, gesture_name)
            expect_effects(session, before, 0, f"{gesture_name} held across {name}")
            fresh_gesture(session, hard, name)
            results.append(f"{gesture_name} held across {name}")

    for change in ("checked", "availability"):
        observed = session.reset()
        before = len(effects(session.events()))
        if session.call("changePending", change, "unmount") != "true":
            raise ValueError(f"the {change} change before unmounting was rejected")
        session.quiet()
        verify_pending(session.events(), observed["generation"], change, "unmount")
        expect_effects(session, before, 0, f"unmount with a pending frame after the {change} change")
        fresh_gesture(session, False, f"unmount with a pending frame after the {change} change")
        results.append(f"unmount with a pending frame after the {change} change")
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
        session = ToggleSession(host, module, font, args.output_dir)
        report = {}
        for name, cases in (("activation", activation_cases), ("focus", focus_cases),
                            ("interruption", interruption_cases), ("reentrancy", reentrancy_cases),
                            ("lifetime", lifetime_cases)):
            report[name] = cases(session)
            verify_publications(session.events())
            report["rejections"] = session.rejections()
            print(f"PASS {name}: {len(report[name])} cases", flush=True)
        (args.output_dir / "report.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    except (OSError, ValueError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"Quickshell Toggle lifecycle check failed: {error}", file=sys.stderr)
        return 1
    finally:
        if session:
            session.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
