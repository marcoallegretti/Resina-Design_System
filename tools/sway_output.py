"""Change the output scale of a running Sway session through its IPC socket."""
import json
import os
import re
import subprocess
import time

SOCKET = re.compile(r"sway-ipc\.[0-9]+\.[0-9]+\.sock")


def sway_socket(explicit, runtime_dir, entries):
    found = sorted(entry for entry in entries if SOCKET.fullmatch(entry))
    if len(found) != 1:
        raise ValueError(f"requires exactly one Sway IPC socket in {runtime_dir}, found {found}")
    socket = os.path.join(runtime_dir, found[0])
    if explicit and os.path.normpath(explicit) != os.path.normpath(socket):
        raise ValueError(f"SWAYSOCK {explicit} is not the Sway IPC socket {socket} of this session")
    return socket


def single_output(outputs):
    active = [output for output in outputs if output.get("active")]
    if len(active) != 1:
        raise ValueError(f"requires exactly one active Sway output, found {len(active)}")
    return active[0]


def changed_scale(initial, final):
    if final is None:
        return None
    if final == initial:
        raise ValueError(f"the live scale {final:g} does not differ from the initial scale {initial:g}")
    if not 0 < final <= 4:
        raise ValueError(f"the live scale {final!r} is not a positive output scale of at most 4")
    return final


class Sway:
    def __init__(self, environment):
        runtime = environment["XDG_RUNTIME_DIR"]
        self.socket = sway_socket(environment.get("SWAYSOCK"), runtime, os.listdir(runtime))

    def message(self, *arguments):
        result = subprocess.run(["swaymsg", "-s", self.socket, *arguments], capture_output=True, text=True,
                                timeout=10)
        if result.returncode:
            raise ValueError(f"swaymsg {' '.join(arguments)} failed: {(result.stdout + result.stderr).strip()}")
        return result.stdout

    def output(self):
        return single_output(json.loads(self.message("-r", "-t", "get_outputs")))

    def set_scale(self, scale, timeout=10):
        name = self.output()["name"]
        self.message("output", name, "scale", f"{scale:g}")
        deadline = time.monotonic() + timeout
        while self.output()["scale"] != scale:
            if time.monotonic() > deadline:
                raise ValueError(f"Sway output {name} did not report scale {scale:g}")
            time.sleep(.1)
        return name
