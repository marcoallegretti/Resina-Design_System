"""Verify the Qt runtime's platform appearance preferences against a real xdg-desktop-portal service."""
import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import threading
import time
from xml.sax.saxutils import escape

APPEARANCE = "org.freedesktop.appearance"
SETTINGS = {
    "reduced-motion": ("org.gnome.desktop.a11y.interface", "reduced-motion", "reduce", True),
    "contrast": ("org.gnome.desktop.a11y.interface", "high-contrast", "true", True),
    "color-scheme": ("org.gnome.desktop.interface", "color-scheme", "prefer-dark", "dark"),
}
PROPERTIES = {"reduced-motion": "reducedMotion", "contrast": "highContrast", "color-scheme": "colorScheme"}


def host(module):
    from PySide6.QtCore import QCoreApplication, QTimer, QUrl
    from PySide6.QtQml import QQmlComponent, QQmlEngine
    application = QCoreApplication([])
    engine = QQmlEngine()
    engine.addImportPath(str(module))
    component = QQmlComponent(engine)
    component.setData(b"import QtQml\nimport Resina.Runtime 0.1\nPlatformAppearance {}", QUrl())
    appearance = component.create()
    if appearance is None:
        print(json.dumps({"failure": repr(component.errors())}), flush=True)
        return 1
    last = [None]

    def report():
        observed = {"ready": appearance.property("ready"), "error": appearance.property("error"),
                    **{name: appearance.property(name) for name in PROPERTIES.values()}}
        if observed != last[0]:
            last[0] = observed
            print(json.dumps(observed), flush=True)
    timer = QTimer()
    timer.timeout.connect(report)
    timer.start(20)
    return application.exec()


def portal_values(environment):
    result = subprocess.run(["gdbus", "call", "--session", "--dest", "org.freedesktop.portal.Desktop",
                             "--object-path", "/org/freedesktop/portal/desktop",
                             "--method", "org.freedesktop.portal.Settings.ReadAll", f"['{APPEARANCE}']"],
                            env=environment, capture_output=True, text=True, timeout=10)
    if result.returncode:
        raise ValueError("independent portal read failed: " + result.stderr.strip())
    values = {key: int(value) for key, value in re.findall(r"'([a-z-]+)': <uint32 (\d+)>", result.stdout)}
    expected = {}
    for key, name in PROPERTIES.items():
        if key not in values:
            expected[name] = None
        elif key == "color-scheme":
            expected[name] = {1: "dark", 2: "light"}.get(values[key], "none")
        else:
            expected[name] = values[key] == 1
    return expected


class Host:
    def __init__(self, module, environment):
        self.process = subprocess.Popen([sys.executable, __file__, "--host-mode", "--module-path", str(module)],
                                        env=environment, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        self.lines, self.current = [], None
        threading.Thread(target=lambda: self.lines.extend(self.process.stdout), daemon=True).start()

    def read(self):
        while self.lines:
            line = self.lines.pop(0).strip()
            if line.startswith("{"):
                self.current = json.loads(line)
                if "failure" in self.current:
                    raise ValueError("platform appearance fixture failed: " + self.current["failure"])
            elif any(marker in line for marker in ("TypeError", "ReferenceError", "QML")):
                raise ValueError("platform appearance host reported: " + line)
        return self.current

    def until(self, predicate, description, timeout=10):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            current = self.read()
            if current and predicate(current):
                return current
            time.sleep(.02)
        raise ValueError(f"{description} was not reached: {self.current}")

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()


def matches(observed, expected):
    return observed["ready"] and not observed["error"] and all(observed[name] == value
                                                               for name, value in expected.items())


def session(configuration, output, processes):
    bus = subprocess.Popen(["dbus-daemon", *configuration, "--nofork", "--print-address=1"],
                           stdout=subprocess.PIPE, text=True, start_new_session=True)
    processes.append(bus)
    address = bus.stdout.readline().strip()
    bus.stdout.close()
    return dict(os.environ, DBUS_SESSION_BUS_ADDRESS=address, XDG_CONFIG_HOME=tempfile.mkdtemp(dir=output),
                XDG_CURRENT_DESKTOP="GNOME", QT_QPA_PLATFORM="offscreen", GSETTINGS_BACKEND="dconf")


def start_portal(args, environment, processes, log):
    for executable in (args.backend, args.portal):
        processes.append(subprocess.Popen([str(executable.resolve(strict=True))], env=environment,
                                          stdout=log, stderr=subprocess.STDOUT, start_new_session=True))
        time.sleep(1)


def stop(process):
    for sent in (signal.SIGTERM, signal.SIGKILL):
        try:
            os.killpg(process.pid, sent)
        except ProcessLookupError:
            break
        try:
            process.wait(5)
            break
        except subprocess.TimeoutExpired:
            continue


def wait_portal(environment, expected, description, timeout=15):
    deadline = time.monotonic() + timeout
    while True:
        try:
            values = portal_values(environment)
            if expected is None or values == expected:
                return values
        except ValueError:
            values = None
        if time.monotonic() > deadline:
            raise ValueError(f"{description} was not reached: {values}")
        time.sleep(.1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--module-path", type=Path, required=True)
    parser.add_argument("--host-mode", action="store_true")
    parser.add_argument("--portal", type=Path, default=Path("/usr/libexec/xdg-desktop-portal"))
    parser.add_argument("--backend", type=Path, default=Path("/usr/libexec/xdg-desktop-portal-gnome"))
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--require", nargs="*", choices=list(PROPERTIES), default=[])
    args = parser.parse_args()
    if args.host_mode:
        return host(args.module_path.resolve())
    processes = []
    try:
        if not args.output_dir:
            raise ValueError("--output-dir is required")
        if not os.environ.get("WAYLAND_DISPLAY"):
            raise ValueError("the GNOME portal backend requires a Wayland compositor session")
        module = args.module_path.resolve(strict=True)
        args.output_dir.mkdir(parents=True, exist_ok=False)
        args.output_dir = args.output_dir.resolve()
        report = {"cases": []}

        environment = dict(os.environ, QT_QPA_PLATFORM="offscreen", DBUS_SESSION_BUS_ADDRESS="unix:path=/nonexistent")
        isolated = Host(module, environment)
        processes.append(isolated)
        observed = isolated.until(lambda current: current["ready"], "readiness without a session bus")
        if not observed["error"] or any(observed[name] is not None for name in PROPERTIES.values()):
            raise ValueError(f"a missing session bus did not withdraw every preference diagnostically: {observed}")
        isolated.close()
        report["cases"].append("a missing session bus reports an error and no preference")

        services = args.output_dir / "services"
        services.mkdir()
        for service in Path("/usr/share/dbus-1/services").glob("*.service"):
            if service.name != "org.freedesktop.portal.Desktop.service":
                (services / service.name).write_bytes(service.read_bytes())
        bare = args.output_dir / "bare-bus.conf"
        bare.write_text("<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen>"
                        f"<servicedir>{escape(str(services))}</servicedir>"
                        "<policy context=\"default\"><allow send_destination=\"*\"/>"
                        "<allow receive_sender=\"*\"/><allow own=\"*\"/></policy></busconfig>", encoding="utf-8")
        log = (args.output_dir / "portal.log").open("w")
        processes.append(log)

        environment = session(["--config-file", str(bare)], args.output_dir, processes)
        late = Host(module, environment)
        processes.append(late)
        observed = late.until(lambda current: current["ready"], "readiness without a portal")
        if not observed["error"] or any(observed[name] is not None for name in PROPERTIES.values()):
            raise ValueError(f"an unavailable portal did not withdraw every preference diagnostically: {observed}")
        report["cases"].append("an unavailable portal reports an error and no preference")
        start_portal(args, environment, processes, log)
        expected = wait_portal(environment, None, "portal on the bus without activation")
        late.until(lambda current: matches(current, expected), "preferences once the portal appears")
        report["cases"].append("preferences are read once the portal appears on the bus")
        frontend = processes.pop()
        stop(frontend)
        observed = late.until(lambda current: current["error"], "withdrawal when the portal leaves the bus")
        if any(observed[name] is not None for name in PROPERTIES.values()):
            raise ValueError(f"preferences survived the portal leaving the bus: {observed}")
        processes.append(subprocess.Popen([str(args.portal.resolve(strict=True))], env=environment, stdout=log,
                                          stderr=subprocess.STDOUT, start_new_session=True))
        late.until(lambda current: matches(current, wait_portal(environment, None, "restarted portal")),
                   "preferences once the portal returns")
        report["cases"].append("preferences are withdrawn when the portal leaves and read again when it returns")

        environment = session(["--session"], args.output_dir, processes)
        start_portal(args, environment, processes, log)
        initial = wait_portal(environment, None, "portal Settings interface")
        report["providedKeys"] = [key for key, name in PROPERTIES.items() if initial[name] is not None]
        missing = [key for key in args.require if key not in report["providedKeys"]]
        if missing:
            raise ValueError("the portal backend does not provide required keys: " + ", ".join(missing))

        appearance = Host(module, environment)
        processes.append(appearance)
        appearance.until(lambda current: matches(current, initial), "initial preferences equal to the portal")
        report["cases"].append("initial preferences equal an independent portal read")
        for key in report["providedKeys"]:
            schema, setting, value, changed = SETTINGS[key]
            before = wait_portal(environment, None, "portal before the change")
            target = dict(before, **{PROPERTIES[key]: changed})
            if target == before:
                raise ValueError(f"{key} change would not differ from the current portal value")
            subprocess.run(["gsettings", "set", schema, setting, value], env=environment, check=True, timeout=10)
            wait_portal(environment, target, f"portal {key} after {setting}={value}")
            appearance.until(lambda current: matches(current, target), f"{key} after {setting}={value}")
            subprocess.run(["gsettings", "reset", schema, setting], env=environment, check=True, timeout=10)
            wait_portal(environment, before, f"portal {key} after resetting {setting}")
            appearance.until(lambda current: matches(current, before), f"{key} after resetting {setting}")
            report["cases"].append(f"{key} follows a platform change and its reset through SettingChanged")
        absent = [key for key in PROPERTIES if key not in report["providedKeys"]]
        if absent:
            report["cases"].append("keys absent from the portal stay null: " + ", ".join(absent))
        (args.output_dir / "report.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
        for case in report["cases"]:
            print("PASS " + case)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Qt platform appearance check failed: {error}", file=sys.stderr)
        return 1
    finally:
        for process in reversed(processes):
            if isinstance(process, Host):
                process.close()
            elif isinstance(process, subprocess.Popen):
                stop(process)
            else:
                process.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
