"""Verify native Toggle accessibility through AT-SPI in released Quickshell."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

from check_qt_toggle_control_runtime import ROOT
from check_quickshell_command_accessibility import (DESCRIPTION, REPORT, NativeEvents, Tree, enable_accessibility,
                                                    isolated_results, ready_focus_requests, state, text)
from check_quickshell_command_lifecycle import focused
from check_quickshell_command_updates import verify_version
from check_quickshell_toggle_lifecycle import ToggleSession
from check_slint_native_availability import ACCESSIBLE, call, references
import conformance_font

NAME = "Automatic updates"


def named(address, application, name):
    """Nodes carrying the Toggle's name; the role is not asserted because released Qt publishes Switch as CheckBox."""
    pending, visited, found = [application], set(), []
    while pending:
        node = pending.pop(0)
        if node in visited or len(visited) >= 32:
            raise ValueError("cyclic or excessive native tree")
        visited.add(node)
        children = references(call(*node, ACCESSIBLE + ".GetChildren", address=address))
        if node != application and text(address, node, "Name") == name:
            if children:
                raise ValueError("decorative Toggle anatomy leaked into the native tree")
            found.append(node)
        pending.extend(children)
    return found


class ToggleTree(Tree):
    def toggle(self):
        node = self.settled(lambda: (found := named(self.address, self.application(), NAME)) and len(found) == 1
                            and found[0], f"one native node named {NAME}")
        if text(self.address, node, "Description") != DESCRIPTION:
            raise ValueError("native Toggle lost its complete description")
        return node

    def withdrawn(self, description):
        self.settled(lambda: not named(self.address, self.application(), NAME), description)


def verify(session, tree):
    session.reset()
    session.call("setMounted", "false")
    tree.withdrawn("native Toggle removed with the unmounted Toggle")
    start, requests = tree.events.mark(), len(ready_focus_requests(session))
    session.call("focusOnReady")
    session.call("setMounted", "true")
    session.until(lambda current: current["ready"] and focused(current) and current["state"]["focused"],
                  "Toggle focused by its consumer when it became ready", 20)
    focus_requests = ready_focus_requests(session)[requests:]
    if len(focus_requests) != 1:
        raise ValueError("the consumer did not focus the Toggle from its ready notification")
    node = tree.toggle()
    tree.events.focused(start, node, "the Toggle focused when it became ready")
    if focus_requests[0].get("ignored") is not False:
        raise ValueError("the Toggle was ignored by accessibility when it announced readiness")
    if not state(tree.address, node)["focused"]:
        raise ValueError("the native Toggle does not report focus")
    return ["a Toggle focused from its consumer's ready notification emits a native focus event "
            "on its complete native node"]


def run(args):
    address = enable_accessibility()
    session = events = None
    try:
        session = ToggleSession(args.host, args.module_path, conformance_font.qml_sources(args.font_dir),
                                args.output_dir)
        events = NativeEvents(session.wait, address, args.output_dir)
        return verify(session, ToggleTree(session, address, events))
    finally:
        if events:
            events.close()
        if session:
            session.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--module-path", type=Path, required=True)
    parser.add_argument("--font-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--session", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    try:
        args.host, args.module_path, args.font_dir = (path.resolve(strict=True)
                                                      for path in (args.host, args.module_path, args.font_dir))
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
        results = isolated_results(Path(__file__).resolve(), args)
        (args.output_dir / "report.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
        for result in results:
            print("PASS " + result)
    except (OSError, ValueError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"Quickshell Toggle accessibility check failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
