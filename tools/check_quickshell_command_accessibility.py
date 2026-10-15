"""Verify native Command accessibility through AT-SPI in released Quickshell."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

from check_qt_command_snapshot_runtime import ROOT
from check_qt_native_availability import ACTION, action_names
from check_quickshell_command_lifecycle import REPLACEMENT, Session, effects, expect_effects
from check_quickshell_command_updates import verify_version
from check_slint_native_availability import (ACCESSIBLE, BUTTON_ROLE, PROPERTIES, call, integer_values, payload,
                                             references, state_flags, string_value)

FOCUSED = 12
DESCRIPTION = "Connect using saved settings."
REPORT = "RESINA_ACCESSIBILITY_REPORT "


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


class Tree:
    def __init__(self, session, address):
        self.session, self.address = session, address

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

    def command(self, name):
        def single():
            found = buttons(self.address, self.application())
            if len(found) == 1 and text(self.address, found[0], "Name") == name:
                return found[0]
            return None
        node = self.settled(single, f"one native Button named {name}")
        if text(self.address, node, "Description") != DESCRIPTION:
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
        except ValueError:
            pass

    def expect_state(self, node, enabled, focused):
        expected = {"enabled": enabled, "sensitive": enabled, "focusable": enabled, "focused": focused}
        self.settled(lambda: state(self.address, node) == expected, f"native state {expected}")


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

    session.call("availability", "false")
    tree.expect_state(node, False, False)
    before = len(effects(session.events()))
    tree.press(node)
    expect_effects(session, before, 0, "native Press while unavailable")
    results.append("unavailable Command rejects native Press")

    session.reset()
    session.call("configure", "replacement")
    session.ready()
    if tree.command(REPLACEMENT) != node:
        raise ValueError("configuration replacement changed the native Command identity")
    before = len(effects(session.events()))
    tree.press(node)
    expect_effects(session, before, 1, "native Press after replacement")
    results.append("replacement keeps the native identity, republishes the localized name and acts once")

    for operation, description in (("setMounted", "unmounted Command"), ("setWindowed", "closed surface")):
        session.reset()
        node = tree.command("Reconnect")
        before = len(effects(session.events()))
        session.call(operation, "false")
        tree.absent(f"native Button removed with the {description}")
        tree.press_retired(node)
        expect_effects(session, before, 0, f"native Press after the {description}")
        results.append(f"{description} withdraws the native Button without stale delivery")

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
    return results


def presentation(log, start):
    lines = log.read_text(encoding="utf-8", errors="replace").splitlines()[start:]
    speech = [line.split("SPEECH OUTPUT: ", 1)[1] for line in lines if "SPEECH OUTPUT: " in line]
    braille = [line.split("BRAILLE LINE:", 1)[1].strip() for line in lines if "BRAILLE LINE:" in line]
    return speech, braille


def lines(log):
    return len(log.read_text(encoding="utf-8", errors="replace").splitlines()) if log.exists() else 0


def quiet(session, log, seconds=1.5):
    previous = [lines(log), time.monotonic()]

    def settled():
        current = lines(log)
        if current != previous[0]:
            previous[:] = [current, time.monotonic()]
        return time.monotonic() - previous[1] >= seconds
    session.wait(settled, "quiet screen reader", timeout=30)
    return lines(log)


def announced(speech, *items):
    spoken = [item.split("' ", 1)[0] + "'" for item in speech]
    expected = [f"'{item}'" for item in items]
    return any(spoken[index:index + len(expected)] == expected for index in range(len(spoken)))


def screen_reader(session, output):
    log = output / "orca.debug"
    home = output / "orca-home"
    home.mkdir()
    environment = dict(os.environ, LANGUAGE="en", LC_ALL="C.UTF-8", HOME=str(home),
                       XDG_CONFIG_HOME=str(home / "config"), XDG_DATA_HOME=str(home / "data"))
    version = subprocess.run(["orca", "--version"], env=environment, capture_output=True, text=True,
                             timeout=30).stdout.strip()
    reader = subprocess.Popen(["orca", "--debug-file", str(log)], stdout=(output / "orca.log").open("w"),
                              stderr=subprocess.STDOUT, env=environment)
    try:
        session.wait(lambda: reader.poll() is None and log.exists()
                     and any("Screen reader on" in item for item in presentation(log, 0)[0]), "Orca start", timeout=30)
        session.reset()
        session.call("focusOther")
        session.settle(lambda current: not current["state"]["focused"], "focus away from Command")
        start = quiet(session, log)
        session.call("focusCommand")
        session.wait(lambda: announced(presentation(log, start)[0], "Reconnect", "button.", DESCRIPTION) and any(
            "Reconnect button " + DESCRIPTION in item for item in presentation(log, start)[1]),
            "Orca focus presentation", timeout=10)
        focus = presentation(log, start)
        (output / "screen-reader.json").write_text(json.dumps({
            "screenReader": version, "focusSpeech": focus[0], "focusBraille": focus[1]},
            indent=2, ensure_ascii=False), encoding="utf-8")
        return ["Orca speaks and brailles the focused Command name, role and description"]
    finally:
        if reader.poll() is None:
            reader.terminate()
            try:
                reader.wait(5)
            except subprocess.TimeoutExpired:
                reader.kill()
                reader.wait()


def run(args):
    address = string_value(call("org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus.GetAddress"))
    for name in ("IsEnabled", "ScreenReaderEnabled"):
        call("org.a11y.Bus", "/org/a11y/bus", PROPERTIES + ".Set", "string:org.a11y.Status",
             "string:" + name, "variant:boolean:true")
    os.environ["QT_LINUX_ACCESSIBILITY_ALWAYS_ON"] = "1"
    session = Session(args.host, args.module_path, args.font, args.output_dir)
    try:
        results = verify(session, Tree(session, address))
        if args.screen_reader:
            results += screen_reader(session, args.output_dir)
        return results
    finally:
        session.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--module-path", type=Path, required=True)
    parser.add_argument("--font", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--screen-reader", action="store_true",
                        help="also verify Orca presentation of the focused and unavailable Command")
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
