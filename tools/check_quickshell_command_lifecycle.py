"""Verify compositor-delivered Command input, focus and lifetime in released Quickshell."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

from check_qt_command_snapshot_runtime import ROOT, request_for
from check_quickshell_command_updates import verify_qml_line, verify_version
from wayland_virtual_input import KEY_ENTER, KEY_SPACE, KEY_TAB, VirtualInput

OUTSIDE = (360, 160)
EVENT = "RESINA_LIFECYCLE "
NOT_READY = "Not ready to accept queries yet."
REPLACEMENT = "Riconnetti già"


def completed(log):
    return log[:log.rfind("\n") + 1]


def read_events(log):
    events = []
    for line in log.splitlines():
        verify_qml_line(line)
        if EVENT not in line:
            continue
        event = json.loads(line.split(EVENT, 1)[1])
        if not isinstance(event, dict) or not isinstance(event.get("kind"), str) \
                or not isinstance(event.get("generation"), str):
            raise ValueError("malformed lifecycle event")
        if event["kind"] == "failure":
            raise ValueError("probe failure: " + str(event.get("message")))
        events.append(event)
    return events


def effects(events):
    return [event for event in events if event["kind"] == "effect"]


def focused(observation):
    native = observation["nativeFocus"]
    return bool(native) and all(native[field] for field in ("activeFocus", "windowActive", "visible", "enabled"))


def authored_requests():
    def variant(reduced_motion, family=None, reduced_transparency=False):
        source = request_for("light", "Reconnect")
        preferences = source["content"]["environment"]["accessibilityPreferences"]
        preferences.update(reducedMotion=reduced_motion, reducedTransparency=reduced_transparency)
        if family:
            theme = json.loads(source["content"]["themeSource"])
            theme["materialAssignments"]["control"]["interactive"] = family
            source["content"]["themeSource"] = json.dumps(theme)
        return source
    return {"standard": request_for("light", "Reconnect"), "replacement": request_for("light", REPLACEMENT),
            "motion": variant(False), "reduced": variant(True), "cast": variant(False, "cast"),
            "frost": variant(False, "frost", True),
            "channels": json.loads((ROOT / "conformance/ir/command-motion-request.json").read_text())["channels"]}


class Session:
    component = "command"
    outside = OUTSIDE

    def __init__(self, host, module, font, output):
        self.host, self.output = host, output
        self.shell = self.input = None
        try:
            self.start(module, font, output)
        except BaseException:
            self.close()
            raise

    def start(self, module, font, output):
        self.config = output / "config"
        self.config.mkdir()
        probe = ROOT / f"conformance/quickshell/{self.component}-lifecycle-probe.qml"
        shutil.copyfile(probe, self.config / "shell.qml")
        shutil.copytree(ROOT / "backends/quickshell/resina-qml/qml", self.config / "controls")
        requests = output / "requests.json"
        requests.write_text(json.dumps(self.requests()), encoding="utf-8")
        # Quickshell's reload popup is a separate engine generation that stays open once hovered,
        # and IPC refuses queries while more than one generation exists.
        self.environment = dict(os.environ, QT_QPA_PLATFORM="wayland", QT_QUICK_BACKEND="software",
                                QML_IMPORT_PATH=str(module), RESINA_LIFECYCLE_REQUESTS=str(requests),
                                QS_NO_RELOAD_POPUP="1", **{f"RESINA_{self.component.upper()}_FONT": font.as_uri()})
        self.environment.pop("DISPLAY", None)
        self.environment.pop("QT_SCALE_FACTOR", None)
        self.log = output / "shell.log"
        self.shell = subprocess.Popen([str(self.host), "--no-color", "--path", str(self.config / "shell.qml")],
                                      env=self.environment, stdout=self.log.open("w"), stderr=subprocess.STDOUT)
        observed = self.ready()
        screen = observed["screen"]
        if not screen or screen["width"] <= self.outside[0] or screen["height"] <= self.outside[1]:
            raise ValueError("requires one output large enough for the lifecycle surface")
        self.input = VirtualInput(screen["width"], screen["height"], os.environ["XDG_RUNTIME_DIR"],
                                  os.environ["WAYLAND_DISPLAY"])

    def requests(self):
        return authored_requests()

    def close(self):
        try:
            if self.input:
                self.input.close()
        finally:
            if self.shell and self.shell.poll() is None:
                self.shell.terminate()
                try:
                    self.shell.wait(5)
                except subprocess.TimeoutExpired:
                    self.shell.kill()
                    self.shell.wait()

    def wait(self, predicate, description, timeout=20):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.shell and self.shell.poll() is not None:
                raise ValueError(description + " was not reached: the shell exited")
            result = predicate()
            if result:
                return result
            time.sleep(0.05)
        raise ValueError(description + " was not reached")

    def call(self, function, *arguments, timeout=20):
        deadline = time.monotonic() + timeout
        while True:
            result = subprocess.run([str(self.host), "ipc", "--path", str(self.config / "shell.qml"), "call",
                                     "resina", function, *map(str, arguments)],
                                    env=self.environment, capture_output=True, text=True, timeout=10)
            output = (result.stdout + result.stderr).strip()
            if NOT_READY not in output or time.monotonic() > deadline:
                break
            time.sleep(0.1)
        if result.returncode or NOT_READY in output or function == "observe" and not result.stdout.strip():
            raise ValueError(f"probe call {function} failed: " + output[-500:])
        return result.stdout.strip()

    def observe(self):
        try:
            raw = self.call("observe")
            try:
                observed = json.loads(raw)
            except json.JSONDecodeError:
                raise ValueError("probe observation is not JSON: " + raw[:300]) from None
            if "exception" in observed:
                raise ValueError("probe observation failed: " + observed["exception"])
            return observed
        except ValueError as error:
            self.unavailable = str(error)
            return None

    def current(self):
        observed = self.observe()
        if observed is None:
            raise ValueError(self.unavailable)
        return observed

    def events(self):
        return read_events(completed(self.log.read_text(encoding="utf-8", errors="replace")))

    def until(self, predicate, description, timeout):
        last = [None]

        def check():
            last[0] = self.observe()
            return last[0] if last[0] and predicate(last[0]) else None
        try:
            return self.wait(check, description, timeout)
        except ValueError as error:
            observed = json.dumps(last[0]) if last[0] else getattr(self, "unavailable", "none")
            raise ValueError(f"{error}; last observation {observed}") from None

    def ready(self):
        return self.until(lambda observed: observed["ready"] and observed["target"] and observed["state"]["enabled"],
                          f"ready {self.component.capitalize()} publication", 20)

    def settle(self, predicate, description):
        return self.until(predicate, description, 5)

    def reset(self, configuration="standard", motion=False):
        self.call("setReaction", "")
        for function in ("setWindowed", "setShown", "setMounted"):
            self.call(function, "true")
        self.call("setFocusTaken", "false")
        self.call("setDisabledFocusable", "false")
        self.call("setCommandX", "40")
        self.call("setMotion", str(motion).lower())
        self.call("configure", configuration)
        self.call("availability", "true")
        self.ready()
        self.call("focusCommand")
        return self.settle(lambda current: focused(current) and current["state"]["focused"], "observed Command focus")

    def point(self, observed):
        target = observed["target"]
        self.input.move(target["x"], target["y"])

    def quiet(self, seconds=0.4):
        time.sleep(seconds)


def expect_effects(session, before, count, description):
    if count:
        try:
            session.wait(lambda: len(effects(session.events())) - before >= count, description, timeout=5)
        except ValueError:
            pass
    session.quiet()
    added = effects(session.events())[before:]
    if len(added) != count:
        raise ValueError(f"{description}: expected {count} consumer effects, observed {len(added)}")
    return added


def activation_cases(session):
    results = []
    observed = session.reset()
    before = len(effects(session.events()))
    session.point(observed)
    session.input.button(True)
    held = session.settle(lambda current: (current["state"]["hold"] or {}).get("kind") == "pointer", "pointer hold")
    if not held["state"]["hold"]["inside"]:
        raise ValueError("pointer hold did not start inside the target")
    session.input.button(False)
    expect_effects(session, before, 1, "pointer activation")
    results.append("pointer activation")

    before = len(effects(session.events()))
    session.point(session.current())
    session.input.button(True)
    session.settle(lambda current: (current["state"]["hold"] or {}).get("inside") is True, "pointer hold")
    session.input.move(*OUTSIDE)
    session.settle(lambda current: (current["state"]["hold"] or {}).get("inside") is False, "pointer leaving target")
    session.input.button(False)
    expect_effects(session, before, 0, "pointer released outside")
    results.append("pointer release outside")

    for key, name in ((KEY_SPACE, "space"), (KEY_ENTER, "enter")):
        before = len(effects(session.events()))
        session.input.key(key, True)
        held = session.settle(lambda current: (current["state"]["hold"] or {}).get("key") == name, name + " hold")
        session.quiet(1.2)
        repeated = session.current()["publications"] - held["publications"]
        if repeated < 2:
            raise ValueError(f"{name} hold delivered {repeated} auto-repeat publications; at least 2 are required")
        expect_effects(session, before, 0 if key == KEY_SPACE else 1, name + " held with repeat")
        session.input.key(key, False)
        expect_effects(session, before, 1, name + " activation")
        results.append(name + " activation with repeat")
    return results


def focus_cases(session):
    session.reset()
    session.input.key(KEY_TAB, True)
    session.input.key(KEY_TAB, False)
    session.settle(lambda current: current["otherFocused"] and not current["nativeFocus"]["activeFocus"]
                   and not current["state"]["focused"], "traversal from Command to the next item")
    session.input.key(KEY_TAB, True)
    session.input.key(KEY_TAB, False)
    session.settle(lambda current: focused(current) and current["state"]["focused"] and not current["otherFocused"],
                   "traversal into Command")
    session.call("focusOther")
    session.settle(lambda current: not current["state"]["focused"], "requested focus moved away")
    session.call("focusCommand")
    session.settle(lambda current: focused(current) and current["state"]["focused"], "requested focus observed")

    session.call("focusOther")
    session.call("setFocusTaken", "true")
    session.settle(lambda current: not current["nativeFocus"]["windowActive"], "keyboard focus taken by another surface")
    session.call("focusCommand")
    session.quiet()
    current = session.current()
    if current["state"]["focused"]:
        raise ValueError("unobserved focus request was published as focus")
    session.call("setFocusTaken", "false")
    session.settle(lambda current: focused(current) and current["state"]["focused"],
                   "focus observed after keyboard focus returns")

    before = len(effects(session.events()))
    if session.call("availability", "false") != "true":
        raise ValueError("availability withdrawal was rejected")
    session.settle(lambda current: not current["state"]["enabled"] and not current["state"]["focused"],
                   "unavailable Command without focus")
    session.input.key(KEY_SPACE, True)
    session.input.key(KEY_SPACE, False)
    expect_effects(session, before, 0, "unavailable Command")
    return ["traversal away and back", "requested focus", "failed focus request", "availability withdrawal"]


INTERRUPTIONS = {
    "replacement": lambda session: session.call("configure", "replacement"),
    "unmount": lambda session: session.call("setMounted", "false"),
    "hidden surface": lambda session: session.call("setShown", "false"),
    "closed surface": lambda session: session.call("setWindowed", "false"),
    "soft reload": lambda session: session.call("reloadShell", "false"),
    "hard reload": lambda session: session.call("reloadShell", "true"),
}


def interruption_cases(session):
    results = []
    for gesture in ("space", "pointer"):
        for name, interrupt in INTERRUPTIONS.items():
            observed = session.reset()
            before = len(effects(session.events()))
            if gesture == "space":
                session.input.key(KEY_SPACE, True)
            else:
                session.point(observed)
                session.input.button(True)
            session.settle(lambda current: current["state"]["hold"] is not None, gesture + " hold")
            interrupt(session)
            if name.endswith("reload"):
                retired = observed["generation"]
                session.settle(lambda current: current["generation"] != retired,
                               f"previous generation retired after {name}")
            session.quiet()
            if gesture == "space":
                session.input.key(KEY_SPACE, False)
            else:
                session.input.button(False)
            expect_effects(session, before, 0, f"{gesture} held across {name}")
            current = session.reset()
            before = len(effects(session.events()))
            session.input.key(KEY_SPACE, True)
            session.input.key(KEY_SPACE, False)
            fresh = expect_effects(session, before, 1, f"fresh gesture after {name}")
            if fresh[0]["generation"] != current["generation"]:
                raise ValueError(f"a retired generation delivered the effect after {name}")
            results.append(f"{gesture} held across {name}")
    return results


REACTIONS = {"replace": 1, "invoke": 2, "unavailable": 1, "hide": 1, "unmount": 1, "hideUnmount": 1, "close": 1}


def reentrancy_cases(session):
    results = []
    for gesture, (mode, count) in ((gesture, reaction) for gesture in ("pointer", "space")
                                   for reaction in REACTIONS.items()):
        observed = session.reset()
        before = len(effects(session.events()))
        session.call("setReaction", mode)
        if gesture == "space":
            session.input.key(KEY_SPACE, True)
            session.input.key(KEY_SPACE, False)
        else:
            session.point(observed)
            session.input.button(True)
            session.input.button(False)
        for effect in expect_effects(session, before, count, f"{mode} from consumer callback"):
            if not effect["state"]["enabled"] or effect["state"]["hold"] is not None:
                raise ValueError(f"{mode} callback observed an uncommitted activation")
        if mode == "replace":
            session.settle(lambda current: current["ready"] and current["state"]["enabled"], "replacement publication")
        elif mode == "invoke":
            if session.current()["reactionResult"] is not True:
                raise ValueError("semantic invocation from a consumer callback was not delivered")
        elif mode == "unavailable":
            current = session.current()
            if current["reactionResult"] is not True or current["state"]["enabled"]:
                raise ValueError("synchronous availability withdrawal was not committed")
            session.input.key(KEY_SPACE, True)
            session.input.key(KEY_SPACE, False)
            expect_effects(session, before, count, "delivery after availability withdrawal")
        results.append(f"{mode} from consumer callback after {gesture} activation")
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
        report = {}
        for name, cases in (("activation", activation_cases), ("focus", focus_cases),
                            ("interruption", interruption_cases), ("reentrancy", reentrancy_cases)):
            report[name] = cases(session)
            print(f"PASS {name}: {len(report[name])} cases", flush=True)
        (args.output_dir / "report.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    except (OSError, ValueError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"Quickshell Command lifecycle check failed: {error}", file=sys.stderr)
        return 1
    finally:
        if session:
            session.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
