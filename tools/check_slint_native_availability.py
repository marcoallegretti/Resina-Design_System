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


ROOT = Path(__file__).resolve().parents[1]
ACCESSIBLE = "org.a11y.atspi.Accessible"
PROPERTIES = "org.freedesktop.DBus.Properties"
BUTTON_ROLE = 43
EXPECTED = {
    "Enabled command": (True, True),
    "Disabled command": (False, True),
    "Excluded command": (False, False),
}
# AT-SPI State enum positions; GetState returns two uint32 words.
ENABLED = 8
FOCUSABLE = 11
SENSITIVE = 24


def payload(reply):
    header, separator, body = reply.partition("\n")
    if not separator or not header.startswith("method return "):
        raise ValueError("missing D-Bus method reply")
    return body.strip()


def string_value(reply):
    match = re.fullmatch(r'(?:variant\s+)?string "([^"\\\n]*)"', payload(reply))
    if not match:
        raise ValueError("expected an unescaped fixture string")
    return match[1]


def integer_values(reply, array=False):
    body = payload(reply)
    if array:
        match = re.fullmatch(r"array\s*\[\s*(.*?)\s*\]", body, re.S)
        if not match:
            raise ValueError("expected a D-Bus integer array")
        body = match[1]
    if not re.fullmatch(r"(?:uint32 [0-9]+(?:\s+|$))+", body):
        raise ValueError("expected uint32 values")
    values = [int(value) for value in re.findall(r"uint32 ([0-9]+)", body)]
    if any(value > 0xffffffff for value in values):
        raise ValueError("D-Bus uint32 overflow")
    if not array and len(values) != 1:
        raise ValueError("expected one uint32")
    return values


def references(reply):
    match = re.fullmatch(r"array\s*\[\s*(.*?)\s*\]", payload(reply), re.S)
    if not match:
        raise ValueError("expected accessible references")
    body = match[1]
    pattern = r'struct\s*\{\s*string "(:[0-9]+\.[0-9]+)"\s*object path "(/[A-Za-z0-9_/]+)"\s*\}'
    if re.sub(pattern, "", body).strip():
        raise ValueError("invalid accessible reference")
    result = re.findall(pattern, body)
    if len(result) > 16 or len(set(result)) != len(result):
        raise ValueError("duplicate or excessive accessible children")
    return result


def state_flags(reply):
    words = integer_values(reply, array=True)
    if len(words) != 2:
        raise ValueError("expected both AT-SPI state words")
    bits = words[0] | (words[1] << 32)
    return {"enabled": bool(bits & (1 << ENABLED)),
            "sensitive": bool(bits & (1 << SENSITIVE)),
            "focusable": bool(bits & (1 << FOCUSABLE))}


def assess(observed):
    if set(observed) != set(EXPECTED):
        raise ValueError("native tree must expose exactly the three fixture commands: " + repr(observed))
    issues = []
    for name, (enabled, focusable) in EXPECTED.items():
        if not isinstance(observed[name], dict):
            raise ValueError("invalid native command state: " + name)
        for field, expected in (("enabled", enabled), ("sensitive", enabled), ("focusable", focusable)):
            if type(observed[name].get(field)) is not bool:
                raise ValueError("missing or invalid native state: " + name + "." + field)
            actual = observed[name][field]
            if actual != expected:
                issues.append({"command": name, "state": field, "expected": expected, "actual": actual})
    return {"conformant": not issues, "observed": observed, "issues": issues}


def call(destination, path, method, *arguments, address=None):
    connection = "--session" if address is None else "--bus=" + address
    result = subprocess.run(["dbus-send", connection, "--dest=" + destination,
        "--type=method_call", "--print-reply", path, method, *arguments],
        capture_output=True, text=True, timeout=3, check=False)
    if result.returncode:
        raise ValueError("D-Bus call failed: " + result.stderr[-1000:])
    return result.stdout


def inspect_tree(address, application):
    pending = [application]
    visited = set()
    observed = {}
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
            if name not in EXPECTED or name in observed or description != "Native availability conformance fixture.":
                raise ValueError("unexpected, duplicate or incorrectly described native command")
            observed[name] = state_flags(call(destination, path, ACCESSIBLE + ".GetState", address=address))
        pending.extend(references(call(destination, path, ACCESSIBLE + ".GetChildren", address=address)))
    return observed


def session_probe(command):
    address = string_value(call("org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus.GetAddress"))
    for name in ("IsEnabled", "ScreenReaderEnabled"):
        call("org.a11y.Bus", "/org/a11y/bus", PROPERTIES + ".Set",
            "string:org.a11y.Status", "string:" + name, "variant:boolean:true")
    environment = dict(os.environ, SLINT_BACKEND="winit-software")
    with tempfile.TemporaryFile(mode="w+") as log:
        process = subprocess.Popen(command + [str(ROOT / "conformance/slint/availability-probe.slint")],
            env=environment, stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    log.seek(0)
                    raise ValueError("native viewer exited: " + log.read()[-2000:])
                applications = references(call("org.a11y.atspi.Registry", "/org/a11y/atspi/accessible/root",
                    ACCESSIBLE + ".GetChildren", address=address))
                if applications:
                    if len(applications) != 1:
                        raise ValueError("isolated bus must contain exactly one application")
                    observed = inspect_tree(address, applications[0])
                    if set(observed) == set(EXPECTED):
                        return assess(observed)
                time.sleep(.1)
            raise ValueError("native tree did not publish all three fixture commands before the deadline")
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=3)


def parse_report(output):
    reports = [line.removeprefix("resina-native-availability ") for line in output.splitlines()
               if line.startswith("resina-native-availability ")]
    if len(reports) != 1:
        raise ValueError("native session did not produce exactly one report")
    report = json.loads(reports[0])
    if not isinstance(report, dict) or set(report) != {"conformant", "observed", "issues"}:
        raise ValueError("invalid native availability report")
    if not isinstance(report["observed"], dict):
        raise ValueError("invalid native availability observations")
    if json.dumps(report, sort_keys=True) != json.dumps(assess(report["observed"]), sort_keys=True):
        raise ValueError("native availability report contradicts its observations")
    return report


def run_session(command):
    with tempfile.TemporaryDirectory(prefix="resina-atspi-") as runtime:
        environment = dict(os.environ)
        if environment.get("WAYLAND_DISPLAY"):
            display = Path(environment["WAYLAND_DISPLAY"])
            if not display.is_absolute():
                display = Path(environment.get("XDG_RUNTIME_DIR", "")) / display
            if not display.is_socket():
                raise ValueError("WAYLAND_DISPLAY does not identify a compositor socket")
            environment["WAYLAND_DISPLAY"] = str(display)
        environment["XDG_RUNTIME_DIR"] = runtime
        process = subprocess.Popen(["dbus-run-session", "--", sys.executable, str(Path(__file__).resolve()),
            "--session", "--report-only", "--", *command], env=environment,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
        try:
            output, diagnostics = process.communicate(timeout=30)
        except subprocess.TimeoutExpired:
            raise ValueError("native availability probe exceeded 30 seconds") from None
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.communicate(timeout=3)
        if process.returncode:
            raise ValueError("native session failed: " + diagnostics[-2000:])
        return parse_report(output)


def main():
    parser = argparse.ArgumentParser(description="Check Slint command availability in the native Linux AT-SPI tree.")
    parser.add_argument("--report-only", action="store_true", help="report semantic mismatches without certifying conformance")
    parser.add_argument("--session", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command[1:] if arguments.command[:1] == ["--"] else arguments.command
    if sys.platform != "linux" or not command:
        parser.error("requires Linux, a native compositor and the official Slint viewer command")
    try:
        version = subprocess.run(command + ["--version"], capture_output=True, text=True, timeout=10, check=False)
        if version.returncode or version.stdout.strip() != "slint-viewer 1.18.1":
            raise ValueError("requires the official slint-viewer 1.18.1")
        result = session_probe(command) if arguments.session else run_session(command)
        prefix = "resina-native-availability " if arguments.session else ""
        print(prefix + json.dumps(result, sort_keys=True))
        if not result["conformant"] and not arguments.report_only:
            print("FAIL Slint native command availability: unsupported enabled/focusable semantics", file=sys.stderr)
            return 1
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        print("FAIL Slint native command availability: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
