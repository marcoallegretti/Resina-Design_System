import argparse
import json
from pathlib import Path
import sys

from check_qt_command_snapshot_runtime import ROOT, request_for


def main():
    parser = argparse.ArgumentParser(description="Host the integrated Qt command for real AT-SPI probing.")
    parser.add_argument("--module-path", required=True, type=Path)
    parser.add_argument("--font", required=True, type=Path)
    args = parser.parse_args()
    from PySide6.QtCore import QPluginLoader, QTimer, QUrl, qVersion
    from PySide6.QtGui import QFontDatabase, QGuiApplication
    from PySide6.QtQml import QQmlComponent, QQmlEngine
    from PySide6.QtQuick import QQuickWindow
    if qVersion() != "6.11.2":
        raise ValueError("requires Qt 6.11.2")
    app = QGuiApplication([])
    plugin = QPluginLoader(str(args.module_path.resolve() / "Resina/Runtime/libResina_Runtime.so"))
    if plugin.instance() is None:
        raise ValueError(plugin.errorString())
    if QFontDatabase.applicationFontFamilies(QFontDatabase.addApplicationFont(str(args.font.resolve()))) != ["DejaVu Sans"]:
        raise ValueError("requires explicitly loaded DejaVu Sans")
    engine = QQmlEngine()
    engine.addImportPath(str(args.module_path.resolve()))
    component = QQmlComponent(engine, QUrl.fromLocalFile(str(ROOT / "conformance/quickshell/command-control-fixture.qml")))
    window = component.create()
    if window is None:
        raise ValueError("; ".join(error.toString() for error in component.errors()))
    window.setProperty("configuration", json.dumps(request_for("light", "Reconnect")))
    window.setProperty("revoke", True)
    window.setProperty("logEvents", True)
    window.availability(True)
    window.requestActivate()
    last = [None]

    def observe():
        result = json.loads(window.observation())
        if result["error"]:
            print("RESINA_CONTROL_FAILURE " + result["error"], flush=True)
            app.exit(1)
        if result["ready"]:
            if result["state"]["enabled"] and not result["state"]["focused"]:
                if window.isActive():
                    window.focusCommand()
                return
            mode = result["state"]["enabled"]
            if last[0] is None or last[0] != mode:
                last[0] = mode
                snapshot = json.loads(result["snapshot"])
                print("RESINA_CONTROL_READY " + json.dumps({"state":result["state"],
                    "accessibility":snapshot["accessibility"],"effects":result["effects"]}), flush=True)
    timer = QTimer()
    timer.timeout.connect(observe)
    timer.start(20)
    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
