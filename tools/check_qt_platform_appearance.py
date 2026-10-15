"""Verify the Qt runtime's platform appearance preferences against a real xdg-desktop-portal service."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import threading
import time

APPEARANCE = "org.freedesktop.appearance"
SETTINGS = {
    "reduced-motion": ("org.gnome.desktop.a11y.interface", "reduced-motion", "reduce", "no-preference"),
    "contrast": ("org.gnome.desktop.a11y.interface", "high-contrast", "true", "false"),
    "color-scheme": ("org.gnome.desktop.interface", "color-scheme", "prefer-dark", "prefer-light"),
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
        self.process.terminate()
        self.process.wait(5)


def matches(observed, expected):
    return observed["ready"] and not observed["error"] and all(observed[name] == value
                                                               for name, value in expected.items())


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
        report = {"cases": []}

        environment = dict(os.environ, QT_QPA_PLATFORM="offscreen", DBUS_SESSION_BUS_ADDRESS="unix:path=/nonexistent")
        isolated = Host(module, environment)
        processes.append(isolated)
        observed = isolated.until(lambda current: current["ready"], "readiness without a session bus")
        if not observed["error"] or any(observed[name] is not None for name in PROPERTIES.values()):
            raise ValueError(f"a missing session bus did not withdraw every preference diagnostically: {observed}")
        isolated.close()
        report["cases"].append("a missing session bus reports an error and no preference")

        bare = args.output_dir / "bare-bus.conf"
        bare.write_text("<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen>"
                        "<policy context=\"default\"><allow send_destination=\"*\"/><allow receive_sender=\"*\"/><allow own=\"*\"/>"
                        "</policy></busconfig>", encoding="utf-8")
        for configuration in (["--config-file", str(bare)], ["--session"]):
            bus = subprocess.Popen(["dbus-daemon", *configuration, "--nofork", "--print-address=1"],
                                   stdout=subprocess.PIPE, text=True)
            processes.append(bus)
            address = bus.stdout.readline().strip()
            config = tempfile.mkdtemp(dir=args.output_dir)
            environment = dict(os.environ, DBUS_SESSION_BUS_ADDRESS=address, XDG_CONFIG_HOME=config,
                               XDG_CURRENT_DESKTOP="GNOME", QT_QPA_PLATFORM="offscreen", GSETTINGS_BACKEND="dconf")
            if configuration[0] == "--session":
                break
            unavailable = Host(module, environment)
            processes.append(unavailable)
            observed = unavailable.until(lambda current: current["ready"], "readiness without a portal")
            if not observed["error"] or any(observed[name] is not None for name in PROPERTIES.values()):
                raise ValueError(f"an unavailable portal did not withdraw every preference diagnostically: {observed}")
            unavailable.close()
            report["cases"].append("an unavailable portal reports an error and no preference")

        log = (args.output_dir / "portal.log").open("w")
        for executable in (args.backend, args.portal):
            processes.append(subprocess.Popen([str(executable.resolve(strict=True))], env=environment,
                                              stdout=log, stderr=subprocess.STDOUT))
            time.sleep(1)
        deadline = time.monotonic() + 10
        while True:
            try:
                initial = portal_values(environment)
                break
            except ValueError:
                if time.monotonic() > deadline:
                    raise
                time.sleep(.2)
        report["providedKeys"] = [key for key, name in PROPERTIES.items() if initial[name] is not None]
        missing = [key for key in args.require if key not in report["providedKeys"]]
        if missing:
            raise ValueError("the portal backend does not provide required keys: " + ", ".join(missing))

        appearance = Host(module, environment)
        processes.append(appearance)
        appearance.until(lambda current: matches(current, initial), "initial preferences equal to the portal")
        report["cases"].append("initial preferences equal an independent portal read")
        for key in report["providedKeys"]:
            schema, setting, changed, restored = SETTINGS[key]
            for value in (changed, restored):
                subprocess.run(["gsettings", "set", schema, setting, value], env=environment, check=True, timeout=10)
                deadline = time.monotonic() + 10
                while True:
                    expected = portal_values(environment)
                    if expected != initial or value == restored or time.monotonic() > deadline:
                        break
                    time.sleep(.1)
                appearance.until(lambda current: matches(current, expected), f"{key} after {setting}={value}")
            report["cases"].append(f"{key} follows the platform setting through SettingChanged")
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
                process.process.terminate()
            elif process.poll() is None:
                process.terminate()
    return 0


if __name__ == "__main__":
    sys.exit(main())
