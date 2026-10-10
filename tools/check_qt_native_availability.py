import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import time

from check_slint_native_availability import (
    ACCESSIBLE, BUTTON_ROLE, EXPECTED, PROPERTIES, ROOT, assess, call,
    integer_values, payload, references, state_flags, string_value,
)

PREFIX = "resina-qt-native-availability "
ACTION = "org.a11y.atspi.Action"
RUNNER = ROOT / "tools/run_qt_availability_probe.py"


def action_names(reply):
    match = re.fullmatch(r"array\s*\[\s*(.*?)\s*\]", payload(reply), re.S)
    if not match:
        raise ValueError("expected native action array")
    pattern = r'struct\s*\{\s*string "([^"\\\n]*)"\s*string "[^"\\\n]*"\s*string "[^"\\\n]*"\s*\}'
    body = match[1]
    if re.sub(pattern, "", body).strip():
        raise ValueError("invalid native action")
    names = re.findall(pattern, body)
    if not names or len(names) > 4 or len(set(names)) != len(names) or names.count("Press") != 1:
        raise ValueError("missing, duplicate or excessive native Press actions")
    return names


def inspect(address, application):
    pending, visited, nodes, observed = [application], set(), {}, {}
    while pending:
        destination, path = pending.pop(0)
        if (destination, path) in visited or len(visited) >= 16:
            raise ValueError("cyclic or excessive native tree")
        visited.add((destination, path))
        role = integer_values(call(destination, path, ACCESSIBLE + ".GetRole", address=address))[0]
        if role == BUTTON_ROLE:
            name = string_value(call(destination, path, PROPERTIES + ".Get",
                "string:" + ACCESSIBLE, "string:Name", address=address))
            description = string_value(call(destination, path, PROPERTIES + ".Get",
                "string:" + ACCESSIBLE, "string:Description", address=address))
            if name not in EXPECTED or name in nodes or description != "Native availability conformance fixture.":
                raise ValueError("unexpected, duplicate or incorrectly described native command")
            nodes[name] = (destination, path)
            observed[name] = state_flags(call(destination, path, ACCESSIBLE + ".GetState", address=address))
        pending.extend(references(call(destination, path, ACCESSIBLE + ".GetChildren", address=address)))
    return nodes, observed


def callback_records(log):
    log.seek(0)
    return [json.loads(line.partition("RESINA_QT_ACTION ")[2])
            for line in log.read().splitlines() if "RESINA_QT_ACTION " in line]


def report(observed, actions, callbacks, after, binding="reference-protocol", host="qt-quick-6.11.2"):
    if binding not in ("reference-protocol", "rust-qt-runtime"):
        raise ValueError("unknown Qt binding evidence")
    if host not in ("qt-quick-6.11.2", "quickshell-0.3.1-qt-6.11.2") \
            or (host.startswith("quickshell-") and binding != "rust-qt-runtime"):
        raise ValueError("unknown or contradictory Qt host evidence")
    if not isinstance(observed, dict) or not isinstance(actions, dict):
        raise ValueError("invalid native observations")
    if any(not isinstance(state, dict) or set(state) != {"enabled", "sensitive", "focusable"}
           for state in observed.values()):
        raise ValueError("invalid native state fields")
    result = assess(observed)
    if set(actions) != set(EXPECTED) or any(
        not isinstance(names, list) or not names or len(names) > 4
        or any(type(name) is not str for name in names)
        or len(set(names)) != len(names) or names.count("Press") != 1
        for names in actions.values()
    ):
        raise ValueError("invalid native action observations")
    expected = [
        {"name": "Disabled command", "available": False, "activations": 0},
        {"name": "Excluded command", "available": False, "activations": 0},
        {"name": "Enabled command", "available": True, "activations": 1},
        {"name": "Enabled command", "available": False, "activations": 1},
    ]
    for record in expected:
        state = {"schemaVersion": "0.1.0", "enabled": record["available"], "focused": False, "hold": None}
        transition = {"schemaVersion": "0.1.0", "state": state,
                      "activate": record["available"], "pressed": False, "capture": None}
        disabled = dict(state, enabled=False)
        record.update(invokeResult=transition, availabilityResult=(
            dict(transition, state=disabled, activate=False) if record["available"] else None
        ), committedState=disabled)
    # JSON comparison distinguishes booleans from integers and rejects extra fields.
    if json.dumps(callbacks, sort_keys=True) != json.dumps(expected, sort_keys=True):
        raise ValueError("native callbacks do not prove guarded invocation and stale-action rejection")
    if json.dumps(after, sort_keys=True) != json.dumps(
        {"enabled": False, "sensitive": False, "focusable": False}, sort_keys=True
    ):
        raise ValueError("native state did not reflect availability revoked by the first action")
    return dict(result, actions=actions, callbacks=callbacks, after=after, binding=binding, host=host)


def parse_report(output):
    records = [line[len(PREFIX):] for line in output.splitlines() if line.startswith(PREFIX)]
    if len(records) != 1:
        raise ValueError("native session must produce exactly one report")
    result = json.loads(records[0])
    if not isinstance(result, dict) or set(result) != {
        "conformant", "observed", "issues", "actions", "callbacks", "after", "binding", "host"
    }:
        raise ValueError("invalid native Qt report")
    measured = report(result["observed"], result["actions"], result["callbacks"], result["after"],
                      result["binding"], result["host"])
    if json.dumps(result, sort_keys=True) != json.dumps(measured, sort_keys=True):
        raise ValueError("native Qt report contradicts its observations")
    return result


def session_probe():
    address = string_value(call("org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus.GetAddress"))
    for name in ("IsEnabled", "ScreenReaderEnabled"):
        call("org.a11y.Bus", "/org/a11y/bus", PROPERTIES + ".Set",
            "string:org.a11y.Status", "string:" + name, "variant:boolean:true")
    with tempfile.TemporaryFile(mode="w+") as log:
        process = subprocess.Popen([sys.executable, str(RUNNER)], stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    log.seek(0)
                    raise ValueError("native Qt host exited: " + log.read()[-2000:])
                applications = references(call("org.a11y.atspi.Registry", "/org/a11y/atspi/accessible/root",
                    ACCESSIBLE + ".GetChildren", address=address))
                if len(applications) > 1:
                    raise ValueError("isolated bus must contain one application")
                if applications:
                    nodes, observed = inspect(address, applications[0])
                    if set(nodes) == set(EXPECTED):
                        break
                time.sleep(.1)
            else:
                log.seek(0)
                raise ValueError("native Qt tree did not publish all fixture commands: " + log.read()[-2000:])
            actions = {name: action_names(call(*node, ACTION + ".GetActions", address=address))
                       for name, node in nodes.items()}
            sequence = ("Disabled command", "Excluded command", "Enabled command", "Enabled command")
            for count, name in enumerate(sequence, 1):
                reply = call(*nodes[name], ACTION + ".DoAction",
                    "int32:" + str(actions[name].index("Press")), address=address)
                if payload(reply) != "boolean true":
                    raise ValueError("native action was not delivered")
                deadline = time.monotonic() + 2
                while len(callback_records(log)) < count and time.monotonic() < deadline:
                    time.sleep(.02)
                if len(callback_records(log)) != count:
                    log.seek(0)
                    raise ValueError("missing or duplicate native callback: " + log.read()[-2000:])
            after = state_flags(call(*nodes["Enabled command"], ACCESSIBLE + ".GetState", address=address))
            binding = "rust-qt-runtime" if os.environ.get("RESINA_QT_RUNTIME_MODULE") else "reference-protocol"
            host = "quickshell-0.3.1-qt-6.11.2" if os.environ.get("RESINA_QUICKSHELL_HOST") else "qt-quick-6.11.2"
            return report(observed, actions, callback_records(log), after, binding, host)
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=3)


def run_session():
    with tempfile.TemporaryDirectory(prefix="resina-qt-atspi-") as runtime:
        environment = dict(os.environ)
        if environment.get("WAYLAND_DISPLAY"):
            display = Path(environment["WAYLAND_DISPLAY"])
            if not display.is_absolute():
                display = Path(environment.get("XDG_RUNTIME_DIR", "")) / display
            if not display.is_socket():
                raise ValueError("WAYLAND_DISPLAY does not identify a compositor socket")
            environment["WAYLAND_DISPLAY"] = str(display)
        environment.update(XDG_RUNTIME_DIR=runtime, XDG_CACHE_HOME=runtime,
            QT_QPA_PLATFORM="wayland" if environment.get("WAYLAND_DISPLAY") else "xcb",
            QT_QUICK_BACKEND="software", QT_LINUX_ACCESSIBILITY_ALWAYS_ON="1",
            LANG="C.UTF-8", LC_ALL="C.UTF-8", LANGUAGE="C")
        process = subprocess.Popen(["dbus-run-session", "--", sys.executable, str(Path(__file__).resolve()),
            "--session", "--report-only"], env=environment, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, text=True, start_new_session=True)
        try:
            output, diagnostics = process.communicate(timeout=30)
        except subprocess.TimeoutExpired:
            raise ValueError("native Qt probe exceeded 30 seconds") from None
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.communicate(timeout=3)
        if process.returncode:
            raise ValueError("native Qt session failed: " + diagnostics[-2000:])
        return parse_report(output)


def main():
    parser = argparse.ArgumentParser(description="Measure Qt 6.11.2 native AT-SPI command availability and invocation.")
    parser.add_argument("--report-only", action="store_true", help="report semantic mismatches without certifying conformance")
    parser.add_argument("--session", action="store_true", help=argparse.SUPPRESS)
    arguments = parser.parse_args()
    try:
        if sys.platform != "linux" or not (os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY")):
            raise ValueError("requires Linux and a native display")
        version = subprocess.run([sys.executable, str(RUNNER), "--version"],
            capture_output=True, text=True, timeout=10, check=False)
        expected_version = "Quickshell 0.3.1 / Qt Quick 6.11.2" if os.environ.get("RESINA_QUICKSHELL_HOST") else "Qt Quick 6.11.2"
        if version.returncode or version.stdout.strip() != expected_version:
            raise ValueError("requires Qt 6.11.2; Quickshell hosting also requires version 0.3.1 and the Rust module")
        result = session_probe() if arguments.session else run_session()
        print((PREFIX if arguments.session else "") + json.dumps(result, sort_keys=True))
        if not result["conformant"] and not arguments.report_only:
            raise ValueError("unsupported native enabled/focusable semantics; see reported observations")
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        print("FAIL Qt native command availability: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
