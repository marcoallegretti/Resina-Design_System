import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

from check_qt_native_availability import ACTION, action_names
from check_slint_native_availability import (ACCESSIBLE, PROPERTIES, ROOT,
    call, integer_values, payload, references, state_flags, string_value)


def records(log, prefix):
    log.seek(0)
    return [json.loads(line.partition(prefix)[2]) for line in log.read().splitlines() if prefix in line]


def inspect(address, application):
    pending, visited, commands = [application], set(), []
    while pending:
        node = pending.pop(0)
        if node in visited or len(visited) >= 16:
            raise ValueError("cyclic or excessive native tree")
        visited.add(node)
        role = integer_values(call(*node, ACCESSIBLE + ".GetRole", address=address))[0]
        children = references(call(*node, ACCESSIBLE + ".GetChildren", address=address))
        if role == 7:
            name=string_value(call(*node, PROPERTIES + ".Get", "string:" + ACCESSIBLE, "string:Name", address=address))
            if name == "Reconnect":
                raise ValueError("native Toggle published CheckBox (role 7); requires Switch (role 130)")
        if role == 130:  # AT-SPI Switch; CheckBox is never accepted.
            if children:
                raise ValueError("decorative Toggle descendants leaked into native tree")
            name = string_value(call(*node, PROPERTIES + ".Get", "string:" + ACCESSIBLE, "string:Name", address=address))
            description = string_value(call(*node, PROPERTIES + ".Get", "string:" + ACCESSIBLE, "string:Description", address=address))
            if (name, description) != ("Reconnect", "Connect using saved settings."):
                raise ValueError("native Toggle lost complete name/description")
            commands.append(node)
        pending.extend(children)
    if len(commands) != 1:
        raise ValueError("native tree must expose exactly one integrated Toggle")
    return commands[0]


def session(args):
    address = string_value(call("org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus.GetAddress"))
    for name in ("IsEnabled", "ScreenReaderEnabled"):
        call("org.a11y.Bus", "/org/a11y/bus", PROPERTIES + ".Set", "string:org.a11y.Status", "string:" + name, "variant:boolean:true")
    with tempfile.TemporaryFile(mode="w+") as log:
        process = subprocess.Popen([sys.executable, str(ROOT / "tools/run_qt_toggle_control_probe.py"),
            "--module-path", str(args.module_path.resolve()), "--font", str(args.font.resolve())], stdout=log, stderr=log)
        try:
            def until(predicate, description):
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        log.seek(0)
                        raise ValueError("native host exited: " + log.read()[-2000:])
                    if predicate():
                        return
                    time.sleep(.02)
                log.seek(0)
                raise ValueError(description + ": " + log.read()[-2000:])

            until(lambda: len(records(log, "RESINA_CONTROL_READY ")) == 1, "missing enabled publication")
            applications = references(call("org.a11y.atspi.Registry", "/org/a11y/atspi/accessible/root", ACCESSIBLE + ".GetChildren", address=address))
            if len(applications) != 1:
                raise ValueError("isolated bus must expose one native application")
            node = inspect(address, applications[0])
            def check_state(enabled):
                reply = call(*node, ACCESSIBLE + ".GetState", address=address)
                flags = state_flags(reply)
                words = integer_values(reply, array=True)
                focused = bool(words[0] & (1 << 12))
                if flags != {"enabled":enabled,"sensitive":enabled,"focusable":enabled} or focused != enabled:
                    raise ValueError("native state disagrees with explicitly excluding disabled profile: " + str((flags,focused)))
                checked = bool(words[0] & (1 << 4))
                checkable = bool(words[1] & (1 << (41-32)))
                if checked != (not enabled) or not checkable or words[0] & ((1 << 20) | (1 << 31)):
                    raise ValueError("native Switch checked/checkable/pressed dimensions disagree")
                return {**flags,"focused":focused,"checked":checked,"checkable":checkable}
            before = check_state(True)
            action_reply = call(*node, ACTION + ".GetActions", address=address)
            try:
                actions = action_names(action_reply)
            except ValueError as error:
                raise ValueError(str(error) + ": " + action_reply) from error
            def press():
                if payload(call(*node, ACTION + ".DoAction", "int32:" + str(actions.index(args.action)), address=address)) != "boolean true":
                    raise ValueError("native action was not delivered")
            press()
            until(lambda: len(records(log, "RESINA_CONTROL_READY ")) == 2, "missing disabled publication after product revocation")
            if inspect(address, applications[0]) != node:
                raise ValueError("native action replacement changed Toggle identity")
            after = check_state(False)
            effects = records(log, "RESINA_CONTROL_EFFECT ")
            if len(effects) != 1 or effects[0]["effects"] != 1 or effects[0]["state"]["enabled"]:
                raise ValueError("real AT action did not reach exactly one Rust/product effect with revocation")
            press()
            time.sleep(.2)
            if len(records(log, "RESINA_CONTROL_EFFECT ")) != 1:
                raise ValueError("stale native action produced another product effect")
            ready = records(log, "RESINA_CONTROL_READY ")
            if len(ready) != 2:
                raise ValueError("duplicate or unexpected native publication records")
            action_names(call(*node, ACTION + ".GetActions", address=address))
            for enabled, record in zip((True,False), ready):
                expected = {"schemaVersion":"0.1.0","role":"switch","name":"Reconnect",
                    "description":"Connect using saved settings.","value":None,
                    "state":{"enabled":enabled,"focused":enabled,"checked":not enabled},"focusable":enabled,
                    "actions":[{"kind":"invoke","available":enabled}],"relationships":[]}
                if record["accessibility"] != expected or record["effects"] != (0 if enabled else 1):
                    raise ValueError("native publication differs from complete portable semantics")
            return {"profile":"static-toggle-excluding-disabled","host":"qt-quick-6.11.2",
                "binding":"rust-qt-runtime","before":before,"after":after,"actions":actions,
                "effects":1,"nativeAction":args.action,"staleActionEffects":0,"conformant":True}
        finally:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=3)


def main():
    parser = argparse.ArgumentParser(description="Verify real AT-SPI publication/action for the integrated Qt excluding-disabled static Toggle profile.")
    parser.add_argument("--module-path", required=True, type=Path)
    parser.add_argument("--font", required=True, type=Path)
    parser.add_argument("--action", choices=("Toggle","Press"),default="Toggle")
    parser.add_argument("--session", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    try:
        if sys.platform != "linux" or not (os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY")):
            raise ValueError("requires Linux and a native display")
        if args.session:
            result = session(args)
        else:
            with tempfile.TemporaryDirectory(prefix="resina-control-atspi-") as runtime:
                environment = dict(os.environ)
                if environment.get("WAYLAND_DISPLAY"):
                    display = Path(environment["WAYLAND_DISPLAY"])
                    if not display.is_absolute():
                        display = Path(environment.get("XDG_RUNTIME_DIR", "")) / display
                    if not display.is_socket():
                        raise ValueError("requires an actual compositor socket")
                    environment["WAYLAND_DISPLAY"] = str(display)
                environment.update(XDG_RUNTIME_DIR=runtime,XDG_CACHE_HOME=runtime,
                    QT_QPA_PLATFORM="wayland" if environment.get("WAYLAND_DISPLAY") else "xcb",
                    QT_QUICK_BACKEND="software",QT_LINUX_ACCESSIBILITY_ALWAYS_ON="1")
                process = subprocess.Popen(["dbus-run-session", "--", sys.executable, str(Path(__file__).resolve()),
                    "--module-path", str(args.module_path.resolve()), "--font", str(args.font.resolve()), "--session", "--action", args.action],
                    env=environment,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
                try:
                    output, diagnostics = process.communicate(timeout=35)
                    if process.returncode:
                        raise ValueError("native session failed: " + diagnostics[-2000:])
                    reports = [line[len("RESINA_CONTROL_REPORT "):] for line in output.splitlines() if line.startswith("RESINA_CONTROL_REPORT ")]
                    if len(reports) != 1:
                        raise ValueError("native session must produce exactly one report")
                    result = json.loads(reports[0])
                    if result.get("conformant") is not True:
                        raise ValueError("native session did not qualify the explicit profile")
                finally:
                    if process.poll() is None:
                        os.killpg(process.pid,signal.SIGKILL)
                        process.communicate(timeout=3)
        print(("RESINA_CONTROL_REPORT " if args.session else "") + json.dumps(result,sort_keys=True))
        return 0
    except (OSError,ValueError,subprocess.TimeoutExpired) as error:
        print("Qt integrated native accessibility check failed: " + str(error),file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
