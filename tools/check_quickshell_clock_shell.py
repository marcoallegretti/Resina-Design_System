"""Verify the Quickshell clock shell with compositor-delivered input, live time, reload and composited pixels."""
import argparse
import copy
from datetime import datetime
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

from PIL import Image
from check_qt_command_snapshot_runtime import ROOT, verify_snapshot
from wayland_virtual_input import KEY_SPACE, VirtualInput

KEY_ESCAPE = 1
NOT_READY = "Not ready to accept queries yet."
STARTING = "No running instances"
SHELL = ROOT / "lab/quickshell/clock-shell"
QML_ERRORS = ("TypeError:", "ReferenceError:", "invalid context", "RESINA_CLOCK_ERROR")


def run(command, source):
    result = subprocess.run(command, input=source.encode(), capture_output=True, timeout=30)
    if result.returncode or result.stderr:
        raise ValueError(f"{Path(command[0]).name} failed: {result.stderr.decode(errors='replace')[-1000:]}")
    return json.loads(result.stdout)


def expected_states(control):
    state, hovered = control["state"], control["hovered"]
    hold = state["hold"]
    pressed = bool(hold and (hold["kind"] == "key" or hold["inside"]))
    signals = ["rest"] if not hovered and not pressed else []
    if hovered:
        signals.append("hover")
    if state["focused"]:
        signals.append("focused")
    if pressed:
        signals.append("pressed")
    if not state["enabled"]:
        signals = [signal for signal in signals if signal != "rest"] + ["disabled"]
    return {"schemaVersion": "0.1.0", "states": signals or ["rest"]}


class Shell:
    def __init__(self, host, module, font, theme, output):
        self.host, self.output = host, output
        config = output / "clock-shell"
        config.mkdir(parents=True)
        for name in ("shell.qml", "preferences.json"):
            shutil.copyfile(SHELL / name, config / name)
        (config / "controls").symlink_to(ROOT / "backends/quickshell/resina-qml/qml", target_is_directory=True)
        self.path = config / "shell.qml"
        self.environment = dict(os.environ, QT_QPA_PLATFORM="wayland", QT_QUICK_BACKEND="software",
                                QML_IMPORT_PATH=str(module), RESINA_ROOT=str(ROOT), RESINA_CLOCK_FONT=str(font),
                                RESINA_CLOCK_THEME=theme, QS_NO_RELOAD_POPUP="1")
        self.environment.pop("DISPLAY", None)
        self.environment.pop("QT_SCALE_FACTOR", None)
        self.log = output / "shell.log"
        self.process = subprocess.Popen([str(host), "--no-color", "--path", str(self.path)], env=self.environment,
                                        stdout=self.log.open("w"), stderr=subprocess.STDOUT)
        self.input = None

    def close(self):
        if self.input:
            self.input.close()
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()

    def call(self, function, *arguments, timeout=20):
        deadline = time.monotonic() + timeout
        while True:
            try:
                result = subprocess.run([str(self.host), "ipc", "--path", str(self.path), "call", "clock", function,
                                         *map(str, arguments)], env=self.environment, capture_output=True,
                                        text=True, timeout=10)
            except subprocess.TimeoutExpired:
                if time.monotonic() > deadline:
                    raise
                continue
            output = (result.stdout + result.stderr).strip()
            if (NOT_READY not in output and STARTING not in output) or time.monotonic() > deadline:
                break
            time.sleep(.1)
        if result.returncode or NOT_READY in output or STARTING in output:
            raise ValueError(f"clock shell call {function} failed: {output[-500:]}")
        return result.stdout.strip()

    def observe(self):
        self.check_log()
        observed = json.loads(self.call("observe"))
        errors = {part: error for part, error in observed["errors"].items() if error}
        if observed["loadError"] or (errors and observed["open"] and observed["clock"]):
            raise ValueError(f"clock shell reported errors: {observed['loadError'] or errors}")
        return observed

    def check_log(self):
        for line in self.log.read_text(encoding="utf-8", errors="replace").splitlines():
            if any(marker in line for marker in QML_ERRORS):
                raise ValueError("clock shell logged an error: " + line.strip()[-400:])

    def until(self, predicate, description, timeout=20):
        deadline = time.monotonic() + timeout
        observed = None
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise ValueError(f"{description}: the shell exited")
            observed = self.observe()
            if predicate(observed):
                return observed
            time.sleep(.1)
        raise ValueError(f"{description} was not reached: open={observed and observed['open']} "
                         f"errors={observed and observed['errors']}")

    def click(self, point):
        self.input.move(point["x"], point["y"])
        time.sleep(.2)
        self.input.button(True)
        self.input.button(False)

    def key(self, code):
        self.input.key(code, True)
        self.input.key(code, False)


def closed(observed):
    return not observed["open"] and observed["showClock"] and observed["targets"]["showClock"]


def opened(observed):
    return observed["open"] and observed["clock"] and observed["close"] and observed["focus"]["close"]


def verify_surfaces(shell, observed, resolvers):
    bar = json.loads(shell.call("request", "bar"))
    theme = run([resolvers["theme"], "-"], json.dumps(bar["body"]["theme"]))
    backdrop = theme["opaqueColorFallbacks"]["surface.base"]
    if observed["backdrop"] != backdrop:
        raise ValueError("painted backdrop differs from the resolved opaque surface.base colour")
    checked = {"bar": bar}
    if observed["clock"]:
        checked["clock"] = json.loads(shell.call("request", "clock"))
    for part, request in checked.items():
        body = request["body"]
        if body["adjacentColor"] != backdrop or body["postTreatmentBackdrop"] != backdrop:
            raise ValueError(f"{part} declares an adjacent colour other than the painted backdrop")
        if observed[part] != run([resolvers["surface"], "-"], json.dumps(request)):
            raise ValueError(f"{part} publication differs from the reference surface paint")
    commands = {"showClock": "bar"} | ({"close": "clock"} if observed["clock"] else {})
    for part, surface in commands.items():
        request = json.loads(shell.call("request", part))
        pigment = observed[surface]["body"]["pigment"]["body"]
        if any(request["body"][key] != pigment for key in ("adjacentColor", "postTreatmentBackdrop",
                                                            "surroundingColor")):
            raise ValueError(f"{part} declares surroundings other than its surface body")
        control = observed[part]
        source = copy.deepcopy(request)
        source.update(activation=control["state"], hovered=control["hovered"])
        verify_snapshot(control["snapshot"], source, expected_states(control))
    return backdrop


def verify_time(shell):
    first = shell.until(lambda observed: observed["time"], "clock time")
    time.sleep(1.3)
    second = shell.observe()
    for observed in (first, second):
        date = datetime.fromtimestamp(observed["clockDate"] / 1000)
        if observed["time"] != date.strftime("%H:%M:%S"):
            raise ValueError(f"displayed time {observed['time']} differs from SystemClock {date}")
    if not 1 <= (second["clockDate"] - first["clockDate"]) / 1000 <= 2:
        raise ValueError("SystemClock did not advance by about one second")
    if abs(second["clockDate"] / 1000 - time.time()) > 1.5:
        raise ValueError("SystemClock differs from host wall time")


def pixel(image, point):
    return image.getpixel((round(point[0]), round(point[1])))[:3]


def verify_composition(shell, observed, backdrop, grim, path):
    subprocess.run([grim, str(path)], env=shell.environment, check=True, timeout=20)
    with Image.open(path) as captured:
        image = captured.convert("RGB")
        scale = image.width / observed["outputWidth"]

        def expect(point, color, description):
            expected = tuple(round(channel * 255) for channel in color["components"])
            actual = pixel(image, (point[0] * scale, point[1] * scale))
            if any(abs(a - e) > 1 for a, e in zip(actual, expected)):
                raise ValueError(f"composited {description} {actual} differs from declared {expected}")

        expect((observed["outputWidth"] - 4, observed["outputHeight"] - 4), backdrop, "backdrop")
        expect(observed["probes"]["bar"], observed["bar"]["body"]["pigment"]["body"], "bar body")
        if observed["clock"]:
            expect(observed["probes"]["clock"], observed["clock"]["body"]["pigment"]["body"], "clock body")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--module-path", type=Path, required=True)
    parser.add_argument("--font", type=Path, required=True)
    parser.add_argument("--surface-resolver", type=Path, required=True)
    parser.add_argument("--theme-resolver", type=Path, required=True)
    parser.add_argument("--theme", choices=["light", "dark"], default="light")
    parser.add_argument("--grim", type=Path)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    shell = None
    try:
        if not os.environ.get("WAYLAND_DISPLAY") or not os.environ.get("XDG_RUNTIME_DIR"):
            raise ValueError("requires a live Wayland compositor session with layer-shell and virtual input")
        host, module, font = (path.resolve(strict=True) for path in (args.host, args.module_path, args.font))
        resolvers = {"surface": str(args.surface_resolver.resolve(strict=True)),
                     "theme": str(args.theme_resolver.resolve(strict=True))}
        args.output_dir.mkdir(parents=True, exist_ok=False)
        shell = Shell(host, module, font, args.theme, args.output_dir)
        report = {"theme": args.theme, "cases": []}

        observed = shell.until(closed, "published bar with a ready Show clock Command")
        shell.input = VirtualInput(observed["outputWidth"], observed["outputHeight"],
                                   os.environ["XDG_RUNTIME_DIR"], os.environ["WAYLAND_DISPLAY"])
        if observed["clockEnabled"]:
            raise ValueError("SystemClock runs while the clock surface is closed")
        backdrop = verify_surfaces(shell, observed, resolvers)
        if args.grim:
            verify_composition(shell, observed, backdrop, args.grim, args.output_dir / "closed.png")
        report["cases"].append("bar and backdrop publish the reference paint and truthful adjacent colours")

        shell.click(observed["targets"]["showClock"])
        observed = shell.until(opened, "clock surface opened by a compositor pointer click")
        verify_surfaces(shell, observed, resolvers)
        if not observed["clockEnabled"]:
            raise ValueError("SystemClock is paused while the clock surface is open")
        verify_time(shell)
        if args.grim:
            verify_composition(shell, shell.observe(), backdrop, args.grim, args.output_dir / "open.png")
        report["cases"].append("pointer activation opens the clock surface with keyboard focus on Close and live time")

        shell.key(KEY_SPACE)
        observed = shell.until(lambda current: not current["open"] and not current["clock"],
                               "clock surface closed by Space on the focused Close Command")
        paused = observed["clockDate"]
        time.sleep(1.2)
        if shell.observe()["clockDate"] != paused or observed["clockEnabled"]:
            raise ValueError("SystemClock kept running after the clock surface closed")
        report["cases"].append("keyboard activation of Close unmaps the surface and pauses the clock")

        shell.until(closed, "Show clock ready after closing")
        shell.key(KEY_SPACE)
        time.sleep(1)
        returned = shell.observe()["open"]
        if returned:
            shell.key(KEY_ESCAPE)
            shell.until(lambda current: not current["open"], "clock surface closed by Escape")
        report["spaceActivatesShowClockAfterClose"] = returned

        observed = shell.until(closed, "Show clock ready before Escape")
        shell.click(observed["targets"]["showClock"])
        shell.until(opened, "clock surface reopened by pointer")
        shell.key(KEY_ESCAPE)
        shell.until(lambda current: not current["open"] and not current["clock"], "clock surface closed by Escape")
        report["cases"].append("Escape closes the clock surface")

        observed = shell.until(closed, "Show clock ready before reload")
        shell.click(observed["targets"]["showClock"])
        observed = shell.until(opened, "clock surface open before reload")
        for hard in (False, True):
            generation = observed["generation"]
            shell.call("reload", str(hard).lower())
            if hard:
                observed = shell.until(lambda current: current["generation"] != generation and closed(current),
                                       "hard reload resets the clock surface to closed")
            else:
                observed = shell.until(lambda current: current["generation"] != generation and opened(current),
                                       "soft reload keeps the clock surface open")
                verify_surfaces(shell, observed, resolvers)
                verify_time(shell)
        report["cases"].append("soft reload keeps the open clock surface; hard reload closes it")

        (args.output_dir / "report.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
        for case in report["cases"]:
            print("PASS " + case)
        print(f"INFO Space after closing activates Show clock: {report['spaceActivatesShowClockAfterClose']}")
    except (OSError, ValueError, KeyError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"Quickshell clock shell check failed: {error}", file=sys.stderr)
        return 1
    finally:
        if shell:
            shell.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
