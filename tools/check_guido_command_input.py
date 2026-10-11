"""Compositor-delivered XTest input for the fixed-scale GUIdo Lab fixture."""
import argparse
import ctypes
import ctypes.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]


class ParentInput:
    def __init__(self, name, window):
        x11, xtst = ctypes.util.find_library("X11"), ctypes.util.find_library("Xtst")
        if not x11 or not xtst:
            raise ValueError("parent input requires libX11 and libXtst")
        self.xlib = ctypes.CDLL(x11)
        self.xtest = ctypes.CDLL(xtst)
        for method, arguments, result in (
            ("XOpenDisplay", [ctypes.c_char_p], ctypes.c_void_p),
            ("XDefaultRootWindow", [ctypes.c_void_p], ctypes.c_ulong),
            ("XSetInputFocus", [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong], ctypes.c_int),
            ("XSync", [ctypes.c_void_p, ctypes.c_int], ctypes.c_int),
            ("XCloseDisplay", [ctypes.c_void_p], ctypes.c_int),
            ("XKeysymToKeycode", [ctypes.c_void_p, ctypes.c_ulong], ctypes.c_uint),
            ("XTranslateCoordinates", [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_ulong,
                ctypes.c_int, ctypes.c_int, ctypes.POINTER(ctypes.c_int),
                ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_ulong)], ctypes.c_int),
        ):
            function = getattr(self.xlib, method)
            function.argtypes, function.restype = arguments, result
        self.xtest.XTestFakeMotionEvent.argtypes = [ctypes.c_void_p, ctypes.c_int,
            ctypes.c_int, ctypes.c_int, ctypes.c_ulong]
        self.xtest.XTestFakeButtonEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint,
            ctypes.c_int, ctypes.c_ulong]
        self.xtest.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint,
            ctypes.c_int, ctypes.c_ulong]
        self.display = self.xlib.XOpenDisplay(name.encode())
        if not self.display:
            raise ValueError("cannot open parent X11 display")
        self.held_button, self.held_key = False, 0
        self.errors = []
        callback = ctypes.CFUNCTYPE(ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p)
        self.error_handler = callback(lambda *_: self.errors.append(True) or 0)
        self.xlib.XSetErrorHandler.argtypes = [ctypes.c_void_p]
        self.xlib.XSetErrorHandler.restype = ctypes.c_void_p
        self.previous_handler = self.xlib.XSetErrorHandler(ctypes.cast(self.error_handler, ctypes.c_void_p))
        x, y, child = ctypes.c_int(), ctypes.c_int(), ctypes.c_ulong()
        root = self.xlib.XDefaultRootWindow(self.display)
        if not self.xlib.XTranslateCoordinates(self.display, window, root, 0, 0,
                ctypes.byref(x), ctypes.byref(y), ctypes.byref(child)):
            self.close()
            raise ValueError("parent output window has no measured root position")
        self.origin = x.value, y.value
        self.xlib.XSetInputFocus(self.display, window, 2, 0)
        self.xlib.XSync(self.display, False)
        if self.errors:
            self.close()
            raise ValueError("parent output window rejected native X11 requests")

    def sync(self):
        self.xlib.XSync(self.display, False)
        if self.errors:
            raise ValueError("parent X11 request was rejected")

    def move(self, x, y):
        if not self.xtest.XTestFakeMotionEvent(self.display, -1,
                round(x + self.origin[0]), round(y + self.origin[1]), 0):
            raise ValueError("parent pointer injection failed")
        self.sync()

    def button(self, down):
        if not self.xtest.XTestFakeButtonEvent(self.display, 1, down, 0):
            raise ValueError("parent button injection failed")
        self.held_button = down
        self.sync()

    def key(self, keysym, down):
        code = self.xlib.XKeysymToKeycode(self.display, keysym)
        if not code or not self.xtest.XTestFakeKeyEvent(self.display, code, down, 0):
            raise ValueError("parent key injection failed")
        self.held_key = code if down else 0
        self.sync()

    def close(self):
        if self.display:
            if self.held_button:
                self.xtest.XTestFakeButtonEvent(self.display, 1, False, 0)
            if self.held_key:
                self.xtest.XTestFakeKeyEvent(self.display, self.held_key, False, 0)
            self.xlib.XSync(self.display, False)
            self.xlib.XCloseDisplay(self.display)
            self.xlib.XSetErrorHandler(self.previous_handler)
            self.display = None


def check(args):
    outputs = json.loads(subprocess.check_output([str(args.swaymsg), "-s", str(args.sway_socket),
        "-t", "get_outputs", "-r"], text=True))
    if (len(outputs) != 1 or not outputs[0]["active"] or outputs[0]["name"] != "X11-1"
            or outputs[0]["scale"] != 1 or outputs[0]["rect"]["width"] < 500
            or outputs[0]["rect"]["height"] < 240):
        raise ValueError("requires an isolated Sway X11 output at fixed scale 1")
    if not os.environ.get("WAYLAND_DISPLAY") or not os.environ.get("XDG_RUNTIME_DIR"):
        raise ValueError("requires the live Sway Wayland session")
    with tempfile.TemporaryFile() as log:
        process = subprocess.Popen([str(args.probe.resolve()), str(ROOT), str(args.scenes.resolve()),
            str(args.font.resolve())], stdout=log, stderr=log)
        parent = None
        try:
            def log_text():
                # Popen shares this open-file description. Reading must not move
                # the child's write offset, including while it emits a new event.
                return os.pread(log.fileno(), os.fstat(log.fileno()).st_size, 0).decode(
                    "utf-8", errors="replace")

            def records(prefix):
                return [json.loads(line[len(prefix):]) for line in log_text().splitlines(keepends=True)
                    if line.startswith(prefix) and line.endswith("\n")]

            def until(predicate, description):
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        raise ValueError("native probe exited: " + log_text()[-2000:])
                    if predicate():
                        return
                    time.sleep(.02)
                raise ValueError(description + ": " + log_text()[-2000:])

            until(lambda: bool(records("RESINA_NATIVE_PAINT ")), "missing actual native paint")
            prepared = records("RESINA_NATIVE_PREPARED ")
            if len(prepared) != 1 or prepared[0]["nativeAT"] is not False:
                raise ValueError("unexpected fixture publication")
            target, placement = prepared[0]["target"], prepared[0]["placement"]
            parent = ParentInput(args.parent_display, args.parent_window)

            def state():
                return records("RESINA_NATIVE_STATE ")[-1]["state"]

            def effects():
                return records("RESINA_NATIVE_EFFECT ")

            parent.move(placement["x"] + target["width"] / 2,
                placement["y"] + target["height"] / 2)
            until(lambda: records("RESINA_NATIVE_PAINT ")[-1]["hovered"], "native hover")
            parent.button(True)
            until(lambda: state()["hold"] is not None, "native pointer press")
            if effects():
                raise ValueError("pointer press activated before release")
            parent.move(450, 200)
            until(lambda: state()["hold"] and not state()["hold"]["inside"], "outside retained delivery")
            parent.move(placement["x"] + target["width"] / 2,
                placement["y"] + target["height"] / 2)
            until(lambda: state()["hold"] and state()["hold"]["inside"], "native reentry")
            parent.button(False)
            until(lambda: len(effects()) == 1, "one pointer release effect")
            if state()["hold"] is not None:
                raise ValueError("pointer hold survived release")
            parent.key(0x20, True)
            until(lambda: state()["hold"] and state()["hold"]["kind"] == "key", "Space hold")
            until(lambda: any(record["event"].get("repeat") is True
                and record["event"].get("key") == "space"
                for record in records("RESINA_NATIVE_STATE ")), "native repeat delivery")
            if len(effects()) != 1:
                raise ValueError("Space press or repeat activated")
            parent.key(0x20, False)
            until(lambda: len(effects()) == 2, "one Space release effect")
            parent.key(0xff0d, True)
            until(lambda: len(effects()) == 3 and not state()["enabled"], "Enter effect and revocation")
            parent.key(0xff0d, False)
            until(lambda: records("RESINA_NATIVE_STATE ")[-1]["event"]
                == {"kind": "keyUp", "key": "enter"}, "native Enter release")
            baseline = len(records("RESINA_NATIVE_STATE "))
            parent.button(True)
            parent.button(False)
            parent.key(0x20, True)
            parent.key(0x20, False)
            def unavailable():
                return [r for r in records("RESINA_NATIVE_STATE ")[baseline:]
                    if r["event"]["kind"] in ("pointerDown", "pointerUp", "keyDown", "keyUp")]
            until(lambda: len(unavailable()) >= 4, "complete unavailable pointer/key delivery")
            if ([r["event"]["kind"] for r in unavailable()]
                    != ["pointerDown", "pointerUp", "keyDown", "keyUp"]
                    or any(r["state"]["enabled"] or r["state"]["hold"] for r in unavailable())):
                raise ValueError("unavailable delivery disagrees with current state")
            if len(effects()) != 3 or state()["hold"] is not None:
                raise ValueError("unavailable input activated or left a hold")
            until(lambda: not records("RESINA_NATIVE_PAINT ")[-1]["state"]["enabled"], "disabled paint")
            for index, record in enumerate(effects(), 1):
                if record["effects"] != index or not record["state"]["enabled"]:
                    raise ValueError("consumer did not observe committed eligible state")
            for record in records("RESINA_NATIVE_PAINT "):
                if record["target"] != target:
                    raise ValueError("native frame changed the reserved target")
                if record["state"]["focused"] != record["nativeFocused"]:
                    raise ValueError("painted focus differs from observed native focus")
            return {"profile": "GUIdo-static-command-DPR1-input-probe",
                "delivery": "XTest/X11/Sway/Wayland/GUIdo", "pointerEffects": 1,
                "spaceEffects": 1, "enterEffects": 1, "repeatObserved": True,
                "unavailableEffects": 0, "finalHold": None,
                "nativeAccessibility": False, "physicalDevices": False}
        finally:
            if parent:
                parent.close()
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe", required=True, type=Path)
    parser.add_argument("--scenes", required=True, type=Path)
    parser.add_argument("--font", required=True, type=Path)
    parser.add_argument("--swaymsg", default="swaymsg", type=Path)
    parser.add_argument("--sway-socket", required=True, type=Path)
    parser.add_argument("--parent-display", required=True)
    parser.add_argument("--parent-window", required=True, type=lambda v: int(v, 0))
    args = parser.parse_args()
    try:
        print(json.dumps(check(args), sort_keys=True))
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        parser.exit(1, "GUIdo native input: " + str(error) + "\n")


if __name__ == "__main__":
    main()
