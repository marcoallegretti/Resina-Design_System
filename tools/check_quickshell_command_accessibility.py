"""Verify native Command accessibility through AT-SPI in released Quickshell."""
import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time

from check_qt_command_snapshot_runtime import ROOT
from check_qt_native_availability import ACTION, action_names
from check_quickshell_command_lifecycle import (REPLACEMENT, REPLACEMENT_DESCRIPTION, Session, effects, expect_effects,
                                                focused)
from check_quickshell_command_updates import verify_version
from check_slint_native_availability import (ACCESSIBLE, BUTTON_ROLE, PROPERTIES, call, integer_values, payload,
                                             references, state_flags, string_value)

FOCUSED = 12
DESCRIPTION = "Connect using saved settings."
REPORT = "RESINA_ACCESSIBILITY_REPORT "
UNKNOWN_OBJECT = "org.freedesktop.DBus.Error.UnknownObject"
SIGNAL = re.compile(r"signal .* path=(\S+); interface=org\.a11y\.atspi\.Event\.(\w+); member=(\w+)")
SIGNAL_TEXT = re.compile(r'\s+string "(.*)"')
SIGNAL_DETAIL = re.compile(r"\s+int32 (-?\d+)")
SIGNAL_VALUE = re.compile(r'\s+(?:variant\s+string|object path) "(.*)"')


def buttons(address, application):
    pending, visited, found = [application], set(), []
    while pending:
        node = pending.pop(0)
        if node in visited or len(visited) >= 32:
            raise ValueError("cyclic or excessive native tree")
        visited.add(node)
        role = integer_values(call(*node, ACCESSIBLE + ".GetRole", address=address))[0]
        children = references(call(*node, ACCESSIBLE + ".GetChildren", address=address))
        if role == BUTTON_ROLE:
            if children:
                raise ValueError("decorative Command anatomy leaked into the native tree")
            found.append(node)
        pending.extend(children)
    return found


def text(address, node, field):
    return string_value(call(*node, PROPERTIES + ".Get", "string:" + ACCESSIBLE, "string:" + field, address=address))


def state(address, node):
    reply = call(*node, ACCESSIBLE + ".GetState", address=address)
    return {**state_flags(reply), "focused": bool(integer_values(reply, array=True)[0] & (1 << FOCUSED))}


def listen_for_events(address, *events):
    from PySide6.QtCore import QCoreApplication
    from PySide6.QtDBus import QDBusConnection, QDBusMessage
    application = QCoreApplication([])
    bus = QDBusConnection.connectToBus(address, "resina-event-listener")
    for event in events:
        message = QDBusMessage.createMethodCall("org.a11y.atspi.Registry", "/org/a11y/atspi/registry",
                                                "org.a11y.atspi.Registry", "RegisterEvent")
        message.setArguments([event])
        reply = bus.call(message)
        if reply.type() == QDBusMessage.MessageType.ErrorMessage:
            print(f"event listener registration for {event} failed: " + reply.errorMessage(), flush=True)
            return 1
    print("event listener registered", flush=True)
    return application.exec()


def signals(log):
    found = []
    for line in log.splitlines():
        if match := SIGNAL.match(line):
            found.append([match[1], match[2] + "." + match[3], None, None, None])
        elif found and found[-1][2] is None and (match := SIGNAL_TEXT.fullmatch(line)):
            found[-1][2] = match[1]
        elif found and found[-1][2] is not None and found[-1][3] is None \
                and (match := SIGNAL_DETAIL.fullmatch(line)):
            found[-1][3] = int(match[1])
        elif found and found[-1][3] is not None and found[-1][4] is None \
                and (match := SIGNAL_VALUE.fullmatch(line)):
            found[-1][4] = match[1]
    return [tuple(item) for item in found]


class NativeEvents:
    def __init__(self, wait, address, output):
        self.wait, self.log = wait, output / "atspi-events.log"
        self.processes = []
        try:
            self.processes.append(subprocess.Popen(
                ["dbus-monitor", "--address", address, "type='signal',interface='org.a11y.atspi.Event.Focus'",
                 "type='signal',interface='org.a11y.atspi.Event.Object'"],
                stdout=self.log.open("w"), stderr=subprocess.STDOUT))
            wait(lambda: "NameAcquired" in self.read(), "AT-SPI event monitor", timeout=10)
            # Qt's AT-SPI bridge emits these events only while a listener is registered for them.
            listener = output / "atspi-listener.log"
            self.processes.append(subprocess.Popen(
                [sys.executable, "-c", "import sys; sys.path.insert(0, sys.argv[1]); "
                 "from check_quickshell_command_accessibility import listen_for_events; "
                 "sys.exit(listen_for_events(*sys.argv[2:]))",
                 str(Path(__file__).resolve().parent), address, "Focus:", "Object:StateChanged:",
                 "Object:PropertyChange:", "Object:ChildrenChanged:"],
                stdout=listener.open("w"), stderr=subprocess.STDOUT))
            wait(lambda: "registered" in listener.read_text(encoding="utf-8", errors="replace")
                 or self.processes[-1].poll() is not None, "AT-SPI event listener", timeout=15)
            if self.processes[-1].poll() is not None:
                raise ValueError(listener.read_text(encoding="utf-8", errors="replace").strip()[-500:])
        except BaseException:
            self.close()
            raise

    def read(self):
        return self.log.read_text(encoding="utf-8", errors="replace")

    def mark(self):
        return len(signals(self.read()))

    def expect(self, start, expected, description):
        self.wait(lambda: all(item in [found[:4] for found in signals(self.read())[start:]] for item in expected),
                  description, timeout=10)

    def retained(self, start, node, description):
        for path, event, kind, _, value in signals(self.read())[start:]:
            if event == "Object.ChildrenChanged" and value == node[1]:
                raise ValueError(f"{description} withdrew the native Command from the tree ({kind})")
            if path == node[1] and event == "Object.PropertyChange" \
                    and kind in ("accessible-name", "accessible-description") and not value:
                raise ValueError(f"{description} cleared the native Command {kind.split('-')[1]}")

    def focused(self, start, node, name):
        self.expect(start, [(node[1], "Focus.Focus", "", 0)], f"native focus event for {name}")

    def close(self):
        for process in self.processes:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


class Tree:
    def __init__(self, session, address, events=None):
        self.session, self.address, self.events = session, address, events

    def application(self):
        applications = references(call("org.a11y.atspi.Registry", "/org/a11y/atspi/accessible/root",
                                        ACCESSIBLE + ".GetChildren", address=self.address))
        if len(applications) != 1:
            raise ValueError("the isolated accessibility bus must expose one native application")
        return applications[0]

    def settled(self, predicate, description):
        failure = [None]

        def attempt():
            try:
                return predicate()
            except ValueError as error:
                failure[0] = error
                return None
        try:
            return self.session.wait(attempt, description, timeout=10)
        except ValueError as error:
            raise ValueError(f"{error}; last native error: {failure[0]}" if failure[0] else str(error)) from None

    def command(self, name, description=DESCRIPTION):
        def single():
            found = buttons(self.address, self.application())
            if len(found) == 1 and text(self.address, found[0], "Name") == name:
                return found[0]
            return None
        node = self.settled(single, f"one native Button named {name}")
        if text(self.address, node, "Description") != description:
            raise ValueError("native Command lost its complete description")
        return node

    def absent(self, description):
        self.settled(lambda: not buttons(self.address, self.application()), description)

    def press(self, node):
        actions = action_names(call(*node, ACTION + ".GetActions", address=self.address))
        if payload(call(*node, ACTION + ".DoAction", "int32:" + str(actions.index("Press")),
                        address=self.address)) != "boolean true":
            raise ValueError("native Press was not accepted by the live Command")

    def press_retired(self, node):
        try:
            self.press(node)
        except ValueError as error:
            if UNKNOWN_OBJECT in str(error):
                return
            raise ValueError("native Press on a retired Command did not fail as an unknown object: "
                             + str(error)) from None
        raise ValueError("a retired native Command accepted Press")

    def expect_state(self, node, enabled, focused):
        expected = {"enabled": enabled, "sensitive": enabled, "focusable": enabled, "focused": focused}
        self.settled(lambda: state(self.address, node) == expected, f"native state {expected}")


def ready_focus_requests(session):
    return [event for event in session.events() if event["kind"] == "readyFocus"]


def stable_updates(session, tree):
    session.reset()
    node = tree.command("Reconnect")

    def update(description, perform, settled, name="Reconnect", text=DESCRIPTION, events=()):
        start = tree.events.mark()
        perform()
        session.until(lambda current: current["ready"] and settled(current), description, 10)
        if tree.command(name, text) != node:
            raise ValueError(f"{description} changed the native Command path")
        tree.events.expect(start, [(node[1], "Object.StateChanged", flag, detail) for flag, detail in events],
                           f"native state-change events on the Command for {description}")
        session.quiet()
        tree.events.retained(start, node, description)

    def feedback():
        # A pointer already resting on the target delivers no motion, hence no hover.
        session.input.move(*session.outside)
        session.point(session.current())
        session.settle(lambda current: current["hovered"], "hovered Command")
        session.input.button(True)
        session.settle(lambda current: (current["state"]["hold"] or {}).get("inside") is True, "pointer hold")
        session.input.move(*session.outside)
        session.settle(lambda current: (current["state"]["hold"] or {}).get("inside") is False, "pointer leaving")
        session.input.button(False)
    update("hover and press feedback", feedback,
           lambda current: not current["hovered"] and current["state"]["hold"] is None)
    update("availability withdrawal", lambda: session.call("availability", "false"),
           lambda current: not current["state"]["enabled"],
           events=(("enabled", 0), ("sensitive", 0)))
    tree.expect_state(node, False, False)
    update("availability restoration", lambda: session.call("availability", "true"),
           lambda current: current["state"]["enabled"],
           events=(("enabled", 1), ("sensitive", 1)))
    tree.expect_state(node, True, False)
    update("label replacement", lambda: session.call("configure", "replacement"),
           lambda current: True, REPLACEMENT, REPLACEMENT_DESCRIPTION)
    return ["feedback, availability withdrawal and restoration, and label replacement update the same native "
            "Button in place: no empty name or description, no tree removal, and state-change events on that node"]


def verify(session, tree):
    results = []
    session.reset()
    node = tree.command("Reconnect")
    tree.expect_state(node, True, True)
    results.append("one truthful Button with complete name, description, focus and availability")

    before = len(effects(session.events()))
    tree.press(node)
    for effect in expect_effects(session, before, 1, "native Press"):
        if not effect["state"]["enabled"] or effect["state"]["hold"] is not None:
            raise ValueError("native Press reached the consumer before committed activation")
    results.append("native Press produces exactly one committed effect")

    start = tree.events.mark()
    session.call("availability", "false")
    tree.events.expect(start, [(node[1], "Object.StateChanged", flag, 0) for flag in ("enabled", "sensitive")],
                       "native enabled and sensitive state cleared on the Command")
    tree.expect_state(node, False, False)
    before = len(effects(session.events()))
    tree.press(node)
    expect_effects(session, before, 0, "native Press while unavailable")
    results.append("withdrawn availability clears enabled and sensitive through native state-change events, "
                   "and the unavailable Command rejects native Press")

    session.reset()
    session.call("configure", "replacement")
    session.ready()
    if tree.command(REPLACEMENT, REPLACEMENT_DESCRIPTION) != node:
        raise ValueError("configuration replacement changed the native Command identity")
    before = len(effects(session.events()))
    tree.press(node)
    expect_effects(session, before, 1, "native Press after replacement")
    results.append("replacement keeps the native identity, republishes the localized name and description "
                   "and acts once")

    for operation, description in (("setMounted", "unmounted Command"), ("setWindowed", "closed surface")):
        session.reset()
        node = tree.command("Reconnect")
        before = len(effects(session.events()))
        session.call(operation, "false")
        tree.absent(f"native Button removed with the {description}")
        tree.press_retired(node)
        expect_effects(session, before, 0, f"native Press after the {description}")
        results.append(f"{description} withdraws the native Button without stale delivery")

    for hard, description in ((False, "soft reload"), (True, "hard reload")):
        retired = session.reset()["generation"]
        node = tree.command("Reconnect")
        before = len(effects(session.events()))
        session.call("reloadShell", str(hard).lower())
        session.until(lambda current: current["generation"] != retired, f"generation replaced by the {description}", 20)
        generation = session.reset()["generation"]
        fresh = tree.command("Reconnect")
        if fresh == node:
            raise ValueError(f"the {description} kept the retired native Button path")
        tree.press_retired(node)
        expect_effects(session, before, 0, f"native Press on the Button retired by the {description}")
        tree.press(fresh)
        for effect in expect_effects(session, before, 1, f"native Press after the {description}"):
            if effect["generation"] != generation:
                raise ValueError(f"the effect after the {description} came from a retired generation")
        results.append(f"{description} retires the native Button: a Press held from before fails without an effect "
                       "and the new generation's Button acts once")

    session.reset()
    session.call("setMounted", "false")
    tree.absent("native Button removed with the unmounted Command")
    start, requests = tree.events.mark(), len(ready_focus_requests(session))
    session.call("focusOnReady")
    session.call("setMounted", "true")
    session.until(lambda current: current["ready"] and focused(current) and current["state"]["focused"],
                  "Command focused by its consumer when it became ready", 20)
    focus_requests = ready_focus_requests(session)[requests:]
    if len(focus_requests) != 1:
        raise ValueError("the consumer did not focus the Command from its ready notification")
    node = tree.command("Reconnect")
    tree.events.focused(start, node, "the Command focused when it became ready")
    if focus_requests[0].get("ignored") is not False:
        raise ValueError("the Command was ignored by accessibility when it announced readiness")
    tree.expect_state(node, True, True)
    results.append("a Command focused from its consumer's ready notification emits a native focus event "
                   "on its complete Button")

    session.reset()
    node = tree.command("Reconnect")
    before = len(effects(session.events()))
    session.call("setReaction", "unavailable")
    tree.press(node)
    expect_effects(session, before, 1, "native Press with callback-time withdrawal")
    tree.expect_state(node, False, False)
    tree.press(node)
    expect_effects(session, before, 1, "native Press after callback-time withdrawal")
    results.append("callback-time withdrawal rejects the next native Press")

    session.reset()
    session.call("setDisabledFocusable", "true")
    session.until(lambda current: "does not support discoverable disabled focus" in current["error"]
                  and not current["ready"], "unsupported disabled-focus diagnostic", 10)
    tree.absent("native Button withdrawn for the unsupported disabled-focus policy")
    results.append("unsupported disabled-focus policy fails diagnostically without a native Button")
    return results + stable_updates(session, tree)


def presentation(log, start):
    lines = log.read_text(encoding="utf-8", errors="replace").splitlines()[start:]
    speech = [line.split("SPEECH OUTPUT: ", 1)[1] for line in lines if "SPEECH OUTPUT: " in line]
    braille = [line.split("BRAILLE LINE:", 1)[1].strip() for line in lines if "BRAILLE LINE:" in line]
    return speech, braille


def lines(log):
    return len(log.read_text(encoding="utf-8", errors="replace").splitlines()) if log.exists() else 0


def quiet(wait, log, seconds=1.5):
    previous = [lines(log), time.monotonic()]

    def settled():
        current = lines(log)
        if current != previous[0]:
            previous[:] = [current, time.monotonic()]
        return time.monotonic() - previous[1] >= seconds
    wait(settled, "quiet screen reader", timeout=30)
    return lines(log)


def announced(speech, *items):
    spoken = [item.split("' ", 1)[0] + "'" for item in speech]
    expected = [f"'{item}'" for item in items]
    return any(spoken[index:index + len(expected)] == expected for index in range(len(spoken)))


class ScreenReader:
    def __init__(self, wait, output):
        self.wait, self.log = wait, output / "orca.debug"
        home = output / "orca-home"
        home.mkdir()
        environment = dict(os.environ, LANGUAGE="en", LC_ALL="C.UTF-8", HOME=str(home),
                           XDG_CONFIG_HOME=str(home / "config"), XDG_DATA_HOME=str(home / "data"))
        self.version = subprocess.run(["orca", "--version"], env=environment, capture_output=True, text=True,
                                      timeout=30).stdout.strip()
        self.process = subprocess.Popen(["orca", "--debug-file", str(self.log)],
                                        stdout=(output / "orca.log").open("w"), stderr=subprocess.STDOUT,
                                        env=environment)
        try:
            wait(lambda: self.process.poll() is None and self.log.exists()
                 and any("Screen reader on" in item for item in presentation(self.log, 0)[0]), "Orca start", timeout=30)
        except BaseException:
            self.close()
            raise

    def mark(self):
        return quiet(self.wait, self.log)

    def expect(self, start, speech, braille, description):
        self.wait(lambda: announced(presentation(self.log, start)[0], *speech)
                  and f"'{braille}'" in presentation(self.log, start)[1], description, timeout=10)
        spoken, shown = presentation(self.log, start)
        return {"speech": spoken, "braille": shown}

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()


def screen_reader(session, output):
    reader = ScreenReader(session.wait, output)
    try:
        session.reset()
        session.call("focusOther")
        session.settle(lambda current: not current["state"]["focused"], "focus away from Command")
        start = reader.mark()
        session.call("focusCommand")
        braille = "quickshell application frame Reconnect button " + DESCRIPTION
        focus = reader.expect(start, ("Reconnect", "button.", DESCRIPTION), braille, "Orca focus presentation")
        start = reader.mark()
        session.call("availability", "false")
        session.settle(lambda current: not current["state"]["enabled"], "unavailable Command")
        reader.mark()
        unavailable = dict(zip(("speech", "braille"), presentation(reader.log, start)))
        (output / "screen-reader.json").write_text(json.dumps({
            "screenReader": reader.version, "focus": focus, "unavailableUnverified": unavailable},
            indent=2, ensure_ascii=False), encoding="utf-8")
        return ["Orca speaks and brailles the focused Command name, role and description"]
    finally:
        reader.close()


def run(args):
    address = string_value(call("org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus.GetAddress"))
    for name in ("IsEnabled", "ScreenReaderEnabled"):
        call("org.a11y.Bus", "/org/a11y/bus", PROPERTIES + ".Set", "string:org.a11y.Status",
             "string:" + name, "variant:boolean:true")
    os.environ["QT_LINUX_ACCESSIBILITY_ALWAYS_ON"] = "1"
    session = events = None
    try:
        session = Session(args.host, args.module_path, args.font, args.output_dir)
        events = NativeEvents(session.wait, address, args.output_dir)
        results = verify(session, Tree(session, address, events))
        if args.screen_reader:
            results += screen_reader(session, args.output_dir)
        return results
    finally:
        if events:
            events.close()
        if session:
            session.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--module-path", type=Path, required=True)
    parser.add_argument("--font", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--screen-reader", action="store_true",
                        help="also verify Orca presentation of the focused Command")
    parser.add_argument("--session", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    try:
        args.host, args.module_path, args.font = (path.resolve(strict=True)
                                                  for path in (args.host, args.module_path, args.font))
        if args.session:
            print(REPORT + json.dumps(run(args)), flush=True)
            return 0
        if not os.environ.get("WAYLAND_DISPLAY") or not os.environ.get("XDG_RUNTIME_DIR"):
            raise ValueError("requires a live Wayland compositor session with layer-shell and virtual input")
        environment = dict(os.environ, RESINA_QUICKSHELL_HOST=str(args.host),
                           RESINA_QT_RUNTIME_MODULE=str(args.module_path), QT_QPA_PLATFORM="offscreen")
        verify_version(subprocess.run([sys.executable, str(ROOT / "tools/run_qt_availability_probe.py"), "--version"],
                                      env=environment, capture_output=True, text=True, timeout=15))
        args.output_dir.mkdir(parents=True, exist_ok=False)
        process = subprocess.Popen(["dbus-run-session", "--", sys.executable, str(Path(__file__).resolve()),
                                    "--host", str(args.host), "--module-path", str(args.module_path),
                                    "--font", str(args.font), "--output-dir", str(args.output_dir), "--session",
                                    *(["--screen-reader"] if args.screen_reader else [])],
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
        try:
            output, diagnostics = process.communicate(timeout=300)
        finally:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            if process.poll() is None:
                process.communicate(timeout=5)
        reports = [line[len(REPORT):] for line in output.splitlines() if line.startswith(REPORT)]
        if process.returncode or len(reports) != 1:
            raise ValueError("accessibility session failed: " + diagnostics.strip()[-2000:])
        results = json.loads(reports[0])
        (args.output_dir / "report.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
        for result in results:
            print("PASS " + result)
    except (OSError, ValueError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"Quickshell Command accessibility check failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
