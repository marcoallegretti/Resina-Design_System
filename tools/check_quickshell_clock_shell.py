"""Verify the Quickshell clock shell with compositor-delivered input, live time, reload and composited pixels."""
import argparse
import copy
from datetime import datetime
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import sys
import time

from PIL import Image
from check_qt_command_motion_runtime import motion_mismatch
from check_qt_command_snapshot_runtime import ROOT, verify_snapshot
from check_qt_native_availability import ACTION, action_names
from check_qt_platform_appearance import APPEARANCE, session, start_portal, stop, wait_portal
from check_quickshell_command_accessibility import NativeEvents, ScreenReader, buttons, state, text
from check_quickshell_command_lifecycle import LOST_REPLY
from check_slint_native_availability import ACCESSIBLE, PROPERTIES, call, payload, references, \
    string_value
from sway_output import Sway, changed_scale
from wayland_virtual_input import KEY_SPACE, KEY_TAB, VirtualInput

KEY_ESCAPE = 1
NOT_READY = "Not ready to accept queries yet."
STARTING = "No running instances"
QUERIES = {"frame", "publications", "request", "observe"}
RELOADS = {"reload"}
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


def verify_device_scale(observed, expected, description):
    if type(observed) not in (int, float) or observed != expected:
        raise ValueError(f"{description} {observed!r} is not device scale {expected!r}")


def verify_command_scale(observed, part, request, expected):
    verify_device_scale((observed.get("deviceScale") or {}).get(part), expected,
                        f"the {part} Command's device scale")
    verify_device_scale(request["content"]["environment"].get("scale"), expected,
                        f"the {part} request's environment scale")


def verify_publication_scales(runs, initial, final=None):
    if not runs.get("bar") or not runs.get("showClock"):
        raise ValueError("the shell logged no bar or Show clock publication")
    allowed = [[initial]] if final is None else [[initial], [initial, final], [final]]
    for part, sequence in runs.items():
        scales = [scale for scale, _ in sequence]
        if any(type(scale) not in (int, float) for scale in scales):
            raise ValueError(f"a {part} publication has no device scale")
        if scales not in allowed:
            raise ValueError(f"{part} was published at device scales {scales}, not only "
                             + (f"{initial!r}" if final is None else f"{initial!r} and then {final!r}"))


class Shell:
    def __init__(self, host, module, font, theme, output, platform, scale):
        self.host, self.output, self.scale, self.initial_scale = host, output, scale, scale
        config = output / "clock-shell"
        config.mkdir(parents=True)
        for name in ("shell.qml", "preferences.json"):
            shutil.copyfile(SHELL / name, config / name)
        (config / "controls").symlink_to(ROOT / "backends/quickshell/resina-qml/qml", target_is_directory=True)
        self.path = config / "shell.qml"
        self.environment = dict(os.environ, QT_QPA_PLATFORM="wayland", QT_QUICK_BACKEND="software",
                                QML_IMPORT_PATH=str(module), RESINA_ROOT=str(ROOT), RESINA_CLOCK_FONT=str(font),
                                QS_NO_RELOAD_POPUP="1", QT_LINUX_ACCESSIBILITY_ALWAYS_ON="1")
        for name in ("DBUS_SESSION_BUS_ADDRESS", "XDG_CONFIG_HOME", "XDG_CURRENT_DESKTOP", "GSETTINGS_BACKEND"):
            self.environment[name] = platform[name]
        if theme:
            self.environment["RESINA_CLOCK_THEME"] = theme
        else:
            self.environment.pop("RESINA_CLOCK_THEME", None)
        self.environment.pop("DISPLAY", None)
        self.environment.pop("QT_SCALE_FACTOR", None)
        self.client_environment = {name: value for name, value in self.environment.items()
                                   if name != "QT_LINUX_ACCESSIBILITY_ALWAYS_ON"}
        self.log = output / "shell.log"
        self.recorded = []
        self.process = subprocess.Popen([str(host), "--no-color", "--path", str(self.path)], env=self.environment,
                                        stdout=self.log.open("w"), stderr=subprocess.STDOUT)
        self.input = None

    def close(self):
        try:
            if self.input:
                self.input.close()
        finally:
            self.stop()

    def stop(self):
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
                                         *map(str, arguments)], env=self.client_environment, capture_output=True,
                                        text=True, timeout=10)
            except subprocess.TimeoutExpired:
                if time.monotonic() > deadline:
                    raise
                continue
            output = (result.stdout + result.stderr).strip()
            lost = function in QUERIES and LOST_REPLY in output
            if (NOT_READY not in output and STARTING not in output and not lost) or time.monotonic() > deadline:
                break
            time.sleep(.1)
        if result.returncode or NOT_READY in output or STARTING in output \
                or LOST_REPLY in output and function not in RELOADS:
            raise ValueError(f"clock shell call {function} failed: {output[-500:]}")
        return "" if LOST_REPLY in output else result.stdout.strip()

    def frames(self):
        while True:
            frame = json.loads(self.call("frame", len(self.recorded)))
            if frame is None:
                return self.recorded
            self.recorded.append(frame)

    def record(self, enabled):
        self.call("record", str(enabled).lower())
        self.recorded = []

    def publication_scales(self, final=None):
        verify_publication_scales(json.loads(self.call("publications")), self.initial_scale, final)

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

    def wait_for(self, predicate, description, timeout=20):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise ValueError(f"{description}: the shell exited")
            result = predicate()
            if result:
                return result
            time.sleep(.1)
        raise ValueError(f"{description} was not reached")

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
    return (not observed["open"] and observed["showClock"] and observed["targets"]["showClock"]
            and observed["showClock"]["state"]["enabled"])


def opened(observed):
    return (observed["open"] and observed["clock"] and observed["close"] and observed["focus"]["close"]
            and observed["showClock"] and not observed["showClock"]["state"]["enabled"])


def published(observed, part, request):
    front = observed[part] and observed[part]["body"]["geometry"]["front"]["bounds"]
    size = request["body"]["size"]
    return bool(front) and (front["width"], front["height"]) == (size["width"], size["height"])


def current_requests(shell, parts):
    sources = {part: shell.call("request", part) for part in parts}
    return {part: json.loads(source) for part, source in sources.items() if source}


def settled(shell, parts, timeout=10):
    deadline = time.monotonic() + timeout
    while True:
        requests = current_requests(shell, parts)
        observed = shell.observe()
        current = current_requests(shell, parts)
        if requests == current and all(observed[part] and published(observed, part, request)
                                       for part, request in requests.items()):
            return observed, requests
        if time.monotonic() > deadline:
            raise ValueError("surface publications did not settle on their current requests")
        time.sleep(.1)


def verify_surfaces(shell, observed, resolvers):
    observed, requests = settled(shell, ("bar", "clock"))
    bar = requests["bar"]
    theme = run([resolvers["theme"], "-"], json.dumps(bar["body"]["theme"]))
    backdrop = theme["opaqueColorFallbacks"]["surface.base"]
    if observed["backdrop"] != backdrop:
        raise ValueError("painted backdrop differs from the resolved opaque surface.base colour")
    for part, request in requests.items():
        body = request["body"]
        verify_device_scale(body["theme"]["environment"].get("scale"), shell.scale,
                            f"the {part} request's environment scale")
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
        verify_command_scale(observed, part, request, shell.scale)
        control = observed[part]
        source = copy.deepcopy(request)
        source.update(activation=control["state"], hovered=control["hovered"], focusable=control["state"]["enabled"])
        verify_snapshot(control["snapshot"], source, expected_states(control))
    return backdrop


def verify_time(shell):
    first = shell.until(lambda observed: observed["time"], "clock time")
    started = time.monotonic()
    time.sleep(1.3)
    second = shell.observe()
    elapsed = time.monotonic() - started
    for observed in (first, second):
        date = datetime.fromtimestamp(observed["clockDate"] / 1000)
        if observed["time"] != date.strftime("%H:%M:%S"):
            raise ValueError(f"displayed time {observed['time']} differs from SystemClock {date}")
    if not 1 <= (second["clockDate"] - first["clockDate"]) / 1000 <= elapsed + 1:
        raise ValueError("SystemClock did not advance with the elapsed host time")
    if abs(second["clockDate"] / 1000 - time.time()) > 1.5:
        raise ValueError("SystemClock differs from host wall time")


def footprint(snapshot, origin):
    paint = snapshot["paint"]["paint"]
    if "focus" in paint:
        outer = paint["focus"]["geometry"]["outer"]
        bounds = dict(outer["contour"]["bounds"])
        bounds["x"] += outer["offset"]["x"]
        bounds["y"] += outer["offset"]["y"]
    else:
        bounds = paint["body"]["geometry"]["silhouette"]["bounds"]
    return (origin["x"] + bounds["x"], origin["y"] + bounds["y"], bounds["width"], bounds["height"])


def ring(rectangle, distance=2, step=4):
    x, y, width, height = rectangle
    left, top, right, bottom = x - distance, y - distance, x + width + distance, y + height + distance
    points = []
    position = left
    while position <= right:
        points += [(position, top), (position, bottom)]
        position += step
    position = top
    while position <= bottom:
        points += [(left, position), (right, position)]
        position += step
    return points + [(left, top), (right, top), (left, bottom), (right, bottom)]


def pixel(image, point):
    return image.getpixel((round(point[0]), round(point[1])))[:3]


def verify_capture_size(capture, output, scale):
    for device, logical in zip(capture, output):
        if abs(device - logical * scale) > 1e-6:
            raise ValueError(f"a {capture[0]}x{capture[1]} capture is not the {output[0]}x{output[1]} logical output "
                             f"at device scale {scale:g}; its logical size must be whole at that scale")


def edge_points(width, height):
    return [(0, 0), (width // 2, 0), (width - 1, 0), (0, height // 2), (width - 1, height // 2),
            (0, height - 1), (width // 2, height - 1), (width - 1, height - 1)]


def verify_composition(shell, observed, backdrop, grim, path):
    subprocess.run([grim, str(path)], env=shell.environment, check=True, timeout=20)
    with Image.open(path) as captured:
        image = captured.convert("RGB")
        scale = shell.scale
        verify_capture_size(image.size, (observed["outputWidth"], observed["outputHeight"]), scale)

        def expect(point, color, description, device=False):
            expected = tuple(round(channel * 255) for channel in color["components"])
            actual = pixel(image, point if device else (point[0] * scale, point[1] * scale))
            if any(abs(a - e) > 1 for a, e in zip(actual, expected)):
                raise ValueError(f"composited {description} {actual} differs from declared {expected}")

        expect((observed["outputWidth"] - 4, observed["outputHeight"] - 4), backdrop, "backdrop")
        for point in edge_points(image.width, image.height):
            expect(point, backdrop, f"output edge pixel {point}", device=True)
        expect(observed["probes"]["bar"], observed["bar"]["body"]["pigment"]["body"], "bar body")
        if observed["clock"]:
            expect(observed["probes"]["clock"], observed["clock"]["body"]["pigment"]["body"], "clock body")
        for part, surface in (("showClock", "bar"), ("close", "clock")):
            if observed[part] and observed[surface] and observed["frames"][part]:
                color = observed[surface]["body"]["pigment"]["body"]
                for point in ring(footprint(observed[part]["snapshot"], observed["frames"][part])):
                    expect(point, color, f"{surface} body around the {part} Command")


def held_frames(shell, part):
    frames = [frame for frame in shell.frames() if frame["part"] == part and frame["state"]["hold"]]
    return frames if frames and all(frames[-1]["motion"]["motion"][name]["settled"]
                                    for name in ("bodyMix", "depthScale")) else None


def press_samples(shell, part):
    observed = shell.until(lambda current: current[part] and current["targets"][part], f"{part} ready")
    shell.input.move(observed["targets"][part]["x"], observed["targets"][part]["y"])
    time.sleep(.2)
    # The shell records frames as it publishes them: a polled observation misses
    # every unsettled frame when one frame lasts as long as the spring takes to settle.
    shell.record(True)
    shell.input.button(True)
    try:
        return shell.wait_for(lambda: held_frames(shell, part), f"settled {part} motion while pressed", timeout=10)
    finally:
        shell.input.button(False)
        shell.record(False)


def verify_motion(shell, part, samples, channels, policy):
    request = json.loads(shell.call("request", part))
    for sample in samples:
        verify_device_scale(sample.get("deviceScale"), shell.scale, f"a {part} motion frame's device scale")
        metadata = sample["motion"]
        if any(metadata["channels"][name]["dynamics"] != channels[name]["dynamics"] for name in channels):
            raise ValueError(f"{part} motion dynamics differ from the calibrated Elastomer profiles")
        if metadata["motion"]["policy"] != policy:
            raise ValueError(f"{part} motion policy {metadata['motion']['policy']} is not {policy}")
        source = copy.deepcopy(request)
        source.update(activation=sample["state"], hovered=sample["hovered"], focusable=sample["state"]["enabled"])
        verify_snapshot(sample["snapshot"], source, expected_states(sample), metadata["motion"])
        difference = motion_mismatch(metadata["motion"], source, metadata)
        if difference:
            raise ValueError(f"{part} motion differs from the independent spring equations at {difference}")
    settled = [all(sample["motion"]["motion"][name]["settled"] for name in ("bodyMix", "depthScale"))
               for sample in samples]
    if policy == "spring" and all(settled):
        raise ValueError(f"{part} published no unsettled calibrated spring sample")
    if policy == "reducedMotion" and not all(settled):
        raise ValueError(f"{part} published sampled motion under reduced motion")
    return {"policy": policy, "scale": shell.scale, "heldFrames": len(samples), "unsettled": settled.count(False)}


def calibrated_channels():
    return {"bodyMix": {"dynamics": json.loads((ROOT / "definitions/motion/elastomer-body.json").read_text()),
                        "initial": {"position": 0, "velocity": 0}},
            "depthScale": {"dynamics": json.loads((ROOT / "definitions/motion/elastomer-depth.json").read_text()),
                           "initial": {"position": 1, "velocity": 0}}}


def verify_theme_files(shell, theme):
    bar = json.loads(shell.call("request", "bar"))
    command = json.loads(shell.call("request", "showClock"))
    source = (ROOT / f"tokens/themes/{theme}.json").read_bytes().decode("utf-8")
    appearance = json.loads((ROOT / f"definitions/command-appearance-{theme}.json").read_text(encoding="utf-8"))
    if bar["body"]["theme"]["themeSource"] != source or command["content"]["themeSource"] != source \
            or command["body"]["commandAppearance"] != appearance:
        raise ValueError(f"requests do not use the {theme} theme source and Command appearance")


class Accessibility:
    def __init__(self, shell):
        self.shell = shell
        self.address = string_value(call("org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus.GetAddress"))

    def applications(self):
        return references(call("org.a11y.atspi.Registry", "/org/a11y/atspi/accessible/root",
                               ACCESSIBLE + ".GetChildren", address=self.address))

    def buttons(self):
        found = {}
        for application in self.applications():
            if "quickshell" not in text(self.address, application, "Name").lower():
                continue
            for node in buttons(self.address, application):
                found.setdefault(text(self.address, node, "Name"), []).append(node)
        return found

    def exposed(self):
        nodes = []
        for application in self.applications():
            if "quickshell" not in text(self.address, application, "Name").lower():
                continue
            pending, visited = [application], set()
            while pending:
                node = pending.pop(0)
                if node in visited or len(visited) >= 64:
                    raise ValueError("cyclic or excessive native tree")
                visited.add(node)
                role = payload(call(*node, ACCESSIBLE + ".GetRoleName", address=self.address)).strip('"')
                name = text(self.address, node, "Name")
                if node != application:
                    nodes.append({"role": role.replace("string ", "").strip('"'), "name": name})
                pending.extend(references(call(*node, ACCESSIBLE + ".GetChildren", address=self.address)))
        return nodes

    def wait(self, predicate, description):
        last = [None]

        def attempt():
            try:
                last[0] = self.buttons()
                return predicate(last[0])
            except ValueError as error:
                last[0] = str(error)
                return None
        try:
            return self.shell.wait_for(attempt, description)
        except ValueError as error:
            raise ValueError(f"{error}; last native buttons: {last[0]}") from None

    def settled(self, predicate, description):
        last = [None]

        def attempt():
            try:
                last[0] = self.exposed()
                return last[0] if predicate(last[0]) else None
            except ValueError as error:
                last[0] = str(error)
                return None
        try:
            return self.shell.wait_for(attempt, description)
        except ValueError as error:
            raise ValueError(f"{error}; last native tree: {last[0]}") from None

    def press(self, node):
        actions = action_names(call(*node, ACTION + ".GetActions", address=self.address))
        if payload(call(*node, ACTION + ".DoAction", "int32:" + str(actions.index("Press")),
                        address=self.address)) != "boolean true":
            raise ValueError("native Press was not accepted")


def single(found, name, description, enabled):
    def check(address):
        nodes = found.get(name, [])
        if len(nodes) != 1:
            return None
        node = nodes[0]
        flags = state(address, node)
        if text(address, node, "Description") != description or flags["enabled"] != enabled \
                or flags["sensitive"] != enabled:
            return None
        return node
    return check


def accessibility_cases(shell, report, reader=None):
    tree = Accessibility(shell)
    focus = NativeEvents(shell.wait_for, tree.address, shell.output)
    try:
        closed_tree = tree.wait(lambda found: found if set(found) == {"Show clock"}
                                and single(found, "Show clock", "Open the clock surface.", True)(tree.address)
                                else None, "one available native Button named Show clock")
        report["exposedClosed"] = tree.settled(lambda nodes: True, "a readable native tree while closed")
        events, spoken = focus.mark(), reader.mark() if reader else None
        tree.press(closed_tree["Show clock"][0])
        shell.until(opened, "clock surface opened by a native Press")
        found = tree.wait(lambda found: found if set(found) == {"Show clock", "Close"}
                          and single(found, "Show clock", "Open the clock surface.", False)(tree.address)
                          and single(found, "Close", "Close the clock surface.", True)(tree.address) else None,
                          "Show clock unavailable and Close available while open")
        if not state(tree.address, found["Close"][0])["focused"]:
            raise ValueError("native Close is not focused while the clock surface is open")
        focus.focused(events, found["Close"][0], "Close")
        if reader:
            report["screenReader"] = {"version": reader.version, "close": reader.expect(
                spoken, ("Close", "button.", "Close the clock surface."),
                "quickshell application frame Close button Close the clock surface.", "Orca presentation of Close")}

        def readable(nodes):
            labels = [node["name"] for node in nodes if node["role"] == "label"]
            return (shell.observe()["date"] in labels
                    and any(re.fullmatch(r"[0-9]{2}:[0-9]{2}:[0-9]{2}", name) for name in labels))
        report["exposedOpen"] = tree.settled(readable, "the displayed time and date as native static text")
        tree.press(found["Close"][0])
        shell.until(lambda current: not current["open"] and not current["clock"], "clock closed by a native Press")
        show = tree.wait(lambda found: found if set(found) == {"Show clock"}
                         and single(found, "Show clock", "Open the clock surface.", True)(tree.address) else None,
                         "Show clock available again after closing")["Show clock"][0]
        report["cases"].append("the native tree exposes exactly the shell's Commands with complete names, "
                               "descriptions, availability and focus, plus the displayed time and date, and native "
                               "Press opens and closes the clock surface")

        x, y = shell.observe()["probes"]["bar"]
        shell.click({"x": x, "y": y})
        time.sleep(.5)
        if shell.observe()["focus"]["showClock"]:
            raise ValueError("a click on the bar body focused Show clock before Tab")
        events, spoken = focus.mark(), reader.mark() if reader else None
        shell.key(KEY_TAB)
        shell.until(lambda current: current["focus"]["showClock"] and not current["open"],
                    "Tab moving focus to Show clock after a click on the bar")
        if not state(tree.address, show)["focused"]:
            raise ValueError("native Show clock is not focused after Tab")
        focus.focused(events, show, "Show clock")
        report["cases"].append("native focus events reach Close when the surface opens and Show clock when a click "
                               "on the bar and Tab move keyboard focus to it")
        if reader:
            report["screenReader"]["showClock"] = reader.expect(
                spoken, ("Show clock", "button.", "Open the clock surface."),
                "quickshell application frame Show clock button Open the clock surface.",
                "Orca presentation of Show clock")
            report["cases"].append("Orca speaks and brailles the focused Close and Show clock with name, role "
                                   "and description")
    finally:
        focus.close()


PLATFORM_CASES = {
    "reducedMotion": ("reduced-motion", "the platform reduced-motion setting switches the Commands to immediate "
                                        "settled frames and its reset restores calibrated spring frames"),
    "highContrast": ("contrast", "the platform high-contrast setting reaches every surface and Command"),
}
COLOR_SCHEME_CASE = "the platform colour scheme selects the theme"


def platform_expectations(portal, authored, theme):
    expected = {"reducedTransparency": authored["reducedTransparency"]}
    sources, skips = {}, []
    for name, (key, case) in PLATFORM_CASES.items():
        if portal[name] is None:
            expected[name], sources[name] = authored[name], "authored"
            skips.append({"case": case, "reason": f"the portal does not provide {APPEARANCE} {key}; the authored "
                                                  f"{name} {json.dumps(authored[name])} is in effect"})
        else:
            expected[name], sources[name] = portal[name], "platform"
    if theme:
        skips.append({"case": COLOR_SCHEME_CASE, "reason": f"--theme fixes the {theme} theme"})
    elif portal["colorScheme"] is None:
        skips.append({"case": COLOR_SCHEME_CASE, "reason": f"the portal does not provide {APPEARANCE} color-scheme; "
                                                           "the light theme is in effect"})
    return expected, sources, skips


def verify_platform_preferences(observed, environment, portal, expected, sources):
    reported = {name: observed["platform"].get(name) for name in ("reducedMotion", "highContrast", "colorScheme")}
    if reported != {name: portal[name] for name in reported}:
        raise ValueError(f"the shell's platform preferences {reported} differ from an independent portal read")
    if observed["preferenceSources"] != sources:
        raise ValueError(f"the shell takes preferences from {observed['preferenceSources']}, not {sources}")
    if observed["preferences"] != expected or environment["accessibilityPreferences"] != expected:
        raise ValueError(f"platform preferences did not reach the Command environment: {observed['preferences']}")


def gsettings(environment, *arguments):
    subprocess.run(["gsettings", *arguments], env=environment, check=True, timeout=10)


def live_scale_case(shell, compositor, resolvers, grim, final):
    initial = shell.scale
    observed = shell.until(closed, "Show clock ready before the scale change")
    before = observed["targets"]["showClock"]
    compositor.set_scale(final)
    shell.scale = final
    observed = shell.until(lambda current: closed(current) and current["deviceScale"]["showClock"] == final,
                           f"Show clock republished at device scale {final:g}")
    shell.input.width, shell.input.height = observed["outputWidth"], observed["outputHeight"]
    backdrop = verify_surfaces(shell, observed, resolvers)
    observed = shell.until(closed, f"Show clock ready at device scale {final:g}")
    if observed["targets"]["showClock"] != before:
        raise ValueError(f"the Show clock target moved from {before} to {observed['targets']['showClock']} when the "
                         "device scale changed")
    if grim:
        verify_composition(shell, observed, backdrop, grim, shell.output / "rescaled.png")
    tree = Accessibility(shell)
    tree.wait(lambda found: found if set(found) == {"Show clock"}
              and single(found, "Show clock", "Open the clock surface.", True)(tree.address) else None,
              f"one available native Button named Show clock at device scale {final:g}")
    motion = verify_motion(shell, "showClock", press_samples(shell, "showClock"), calibrated_channels(), "spring")
    shell.until(opened, f"clock surface opened at device scale {final:g}")
    shell.key(KEY_ESCAPE)
    shell.until(lambda current: not current["open"] and not current["clock"],
                f"clock closed at device scale {final:g}")
    shell.publication_scales(final)
    return motion, (f"a live output scale change from {initial:g} to {final:g} republishes the bar and Show clock at "
                    "the new device scale with the same target, the reference paint"
                    + (", composited pixels" if grim else "")
                    + ", one available native Button and calibrated spring frames")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--module-path", type=Path, required=True)
    parser.add_argument("--font", type=Path, required=True)
    parser.add_argument("--surface-resolver", type=Path, required=True)
    parser.add_argument("--theme-resolver", type=Path, required=True)
    parser.add_argument("--theme", choices=["light", "dark"],
                        help="fixed theme; by default the platform colour scheme selects it")
    parser.add_argument("--portal", type=Path, default=Path("/usr/libexec/xdg-desktop-portal"))
    parser.add_argument("--backend", type=Path, default=Path("/usr/libexec/xdg-desktop-portal-gnome"))
    parser.add_argument("--grim", type=Path)
    parser.add_argument("--screen-reader", action="store_true",
                        help="also verify Orca presentation of the focused Close and Show clock")
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--scale", type=float, required=True, help="Output scale the compositor presents")
    parser.add_argument("--live-scale", type=float,
                        help="Output scale Sway changes to while the shell runs, before the accessibility cases")
    args = parser.parse_args()
    shell = reader = None
    processes = []
    try:
        live = changed_scale(args.scale, args.live_scale)
        if not os.environ.get("WAYLAND_DISPLAY") or not os.environ.get("XDG_RUNTIME_DIR"):
            raise ValueError("requires a live Wayland compositor session with layer-shell and virtual input")
        compositor = Sway(os.environ) if live is not None else None
        host, module, font = (path.resolve(strict=True) for path in (args.host, args.module_path, args.font))
        resolvers = {"surface": str(args.surface_resolver.resolve(strict=True)),
                     "theme": str(args.theme_resolver.resolve(strict=True))}
        args.output_dir.mkdir(parents=True, exist_ok=False)
        platform = session(args.output_dir, processes)
        os.environ["DBUS_SESSION_BUS_ADDRESS"] = platform["DBUS_SESSION_BUS_ADDRESS"]
        for name in ("IsEnabled", "ScreenReaderEnabled"):
            call("org.a11y.Bus", "/org/a11y/bus", PROPERTIES + ".Set", "string:org.a11y.Status",
                 "string:" + name, "variant:boolean:true")
        log = (args.output_dir / "portal.log").open("w")
        processes.append(log)
        start_portal(args, platform, processes, log)
        wait_portal(platform, None, "portal Settings interface")
        shell = Shell(host, module, font, args.theme, args.output_dir, platform, args.scale)
        report = {"theme": args.theme or "platform", "scale": args.scale, "liveScale": live, "cases": []}

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
        shell.check_log()
        report["cases"].append("keyboard activation of Close unmaps the surface without QML errors and pauses the clock")

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
            shell.publication_scales()
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

        observed = shell.until(closed, "Show clock ready before the platform cases")
        authored = json.loads((SHELL / "preferences.json").read_text())["accessibilityPreferences"]
        portal = wait_portal(platform, None, "portal Settings interface")
        expected, sources, report["skips"] = platform_expectations(portal, authored, args.theme)
        verify_platform_preferences(observed, json.loads(shell.call("request", "showClock"))["content"]["environment"],
                                    portal, expected, sources)
        report["cases"].append("platform appearance preferences reach the Command environment")

        channels = calibrated_channels()
        report["motion"] = [verify_motion(shell, "showClock", press_samples(shell, "showClock"), channels, "spring")]
        shell.until(opened, "clock surface opened after the sampled press")
        shell.key(KEY_ESCAPE)
        shell.until(lambda current: not current["open"] and not current["clock"], "clock closed after the press")
        report["cases"].append("a held Show clock publishes calibrated Elastomer spring frames that match the equations")

        if sources["reducedMotion"] == "platform":
            gsettings(platform, "set", "org.gnome.desktop.a11y.interface", "reduced-motion", "reduce")
            shell.until(lambda current: current["preferences"]["reducedMotion"] and closed(current),
                        "platform reduced motion in the shell environment")
            report["motion"].append(verify_motion(shell, "showClock", press_samples(shell, "showClock"), channels,
                                                  "reducedMotion"))
            shell.until(opened, "clock surface opened under reduced motion")
            shell.key(KEY_ESCAPE)
            shell.until(lambda current: not current["open"] and not current["clock"], "clock closed under reduced motion")
            gsettings(platform, "reset", "org.gnome.desktop.a11y.interface", "reduced-motion")
            shell.until(lambda current: not current["preferences"]["reducedMotion"] and closed(current),
                        "platform reduced motion reset")
            report["motion"].append(verify_motion(shell, "showClock", press_samples(shell, "showClock"), channels,
                                                  "spring"))
            shell.until(opened, "clock surface opened after the reduced-motion reset")
            shell.key(KEY_ESCAPE)
            shell.until(lambda current: not current["open"] and not current["clock"], "clock closed after the reset")
            report["cases"].append(PLATFORM_CASES["reducedMotion"][1])

        if sources["highContrast"] == "platform":
            gsettings(platform, "set", "org.gnome.desktop.a11y.interface", "high-contrast", "true")
            observed = shell.until(lambda current: current["preferences"]["highContrast"] and closed(current),
                                   "platform high contrast in the shell environment")
            verify_surfaces(shell, observed, resolvers)
            gsettings(platform, "reset", "org.gnome.desktop.a11y.interface", "high-contrast")
            observed = shell.until(lambda current: not current["preferences"]["highContrast"] and closed(current),
                                   "platform high contrast reset")
            verify_surfaces(shell, observed, resolvers)
            report["cases"].append(PLATFORM_CASES["highContrast"][1])

        if not args.theme and portal["colorScheme"] is not None:
            gsettings(platform, "set", "org.gnome.desktop.interface", "color-scheme", "prefer-dark")
            observed = shell.until(lambda current: current["theme"] == "dark" and closed(current),
                                   "dark theme from the platform colour scheme")
            verify_surfaces(shell, observed, resolvers)
            verify_theme_files(shell, "dark")
            gsettings(platform, "reset", "org.gnome.desktop.interface", "color-scheme")
            observed = shell.until(lambda current: current["theme"] == "light" and closed(current),
                                   "light theme after resetting the colour scheme")
            verify_surfaces(shell, observed, resolvers)
            verify_theme_files(shell, "light")
            report["cases"].append(COLOR_SCHEME_CASE)

        if live is not None:
            motion, case = live_scale_case(shell, compositor, resolvers, args.grim, live)
            report["motion"].append(motion)
            report["cases"].append(case)

        shell.until(closed, "Show clock ready before the accessibility cases")
        if args.screen_reader:
            reader = ScreenReader(shell.wait_for, args.output_dir)
        accessibility_cases(shell, report, reader)
        shell.publication_scales(live)
        report["cases"].append("every bar, clock surface and Command publication from startup uses its window's "
                               "device scale" + (f", moving once from {args.scale:g} to {live:g}" if live else ""))

        (args.output_dir / "report.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
        for case in report["cases"]:
            print("PASS " + case)
        for skip in report["skips"]:
            print(f"SKIP {skip['case']}: {skip['reason']}")
        print(f"INFO Space after closing activates Show clock: {report['spaceActivatesShowClockAfterClose']}")
        for press in report["motion"]:
            print(f"INFO {press['policy']} press at device scale {press['scale']:g}: {press['heldFrames']} held frames, "
                  f"{press['unsettled']} unsettled")
    except (OSError, ValueError, KeyError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"Quickshell clock shell check failed: {error}", file=sys.stderr)
        return 1
    finally:
        if reader:
            reader.close()
        if shell:
            shell.close()
        for process in reversed(processes):
            if isinstance(process, subprocess.Popen):
                stop(process)
            else:
                process.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
