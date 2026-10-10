import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class ActivationProbe:
    def __init__(self):
        from check_schemas import validator_for

        self.validator = validator_for("schemas/activation-result.schema.json")
        self.backend = os.environ.get("RESINA_ACTIVATION_BACKEND", str(ROOT / "target/debug/resina-activation"))
        self.states = {name: {"schemaVersion": "0.1.0", "enabled": enabled,
                              "focused": False, "hold": None}
                       for name, enabled in (("Enabled command", True),
                                             ("Disabled command", False), ("Excluded command", False))}
        self.activations = dict.fromkeys(self.states, 0)

    def transition(self, name, event):
        request = {"schemaVersion": "0.1.0", "state": self.states[name], "event": event}
        execution = subprocess.run([self.backend, "-"], input=json.dumps(request),
                                   capture_output=True, text=True, timeout=3, check=False)
        if execution.returncode or execution.stderr or len(execution.stdout) > 65536:
            raise ValueError("activation reference failed: " + execution.stderr[-1000:])
        result = json.loads(execution.stdout)
        self.validator.validate(result)
        self.states[name] = result["state"]
        return result

    def invoke(self, name, available):
        if name not in self.states or self.states[name]["enabled"] != available:
            raise ValueError("native availability and committed activation state disagree")
        result = self.transition(name, {"kind": "invoke"})
        availability = None
        if result["activate"]:
            self.activations[name] += 1
            availability = self.transition(name, {"kind": "availability", "enabled": False})
        return {"name": name, "available": available, "activations": self.activations[name],
                "invokeResult": result, "availabilityResult": availability,
                "committedState": self.states[name]}


def main():
    from PySide6.QtCore import QObject, QUrl, Slot, qVersion
    from PySide6.QtGui import QGuiApplication
    from PySide6.QtQml import QQmlApplicationEngine

    if sys.argv[1:] == ["--version"]:
        print("Qt Quick " + qVersion())
        return 0
    if sys.argv[1:]:
        print("usage: run_qt_availability_probe.py [--version]", file=sys.stderr)
        return 2
    if qVersion() != "6.11.2":
        print("requires Qt 6.11.2", file=sys.stderr)
        return 1
    class NativeLifecycle(QObject):
        def __init__(self):
            super().__init__()
            self.probe = ActivationProbe()

        @Slot(str, bool, result=str)
        def invoke(self, name, available):
            return json.dumps(self.probe.invoke(name, available))

    app = QGuiApplication(sys.argv)
    engine = QQmlApplicationEngine()
    lifecycle = NativeLifecycle()
    engine.rootContext().setContextProperty("resinaLifecycle", lifecycle)
    source = ROOT / "conformance/quickshell/availability-probe.qml"
    engine.load(QUrl.fromLocalFile(str(source)))
    if not engine.rootObjects():
        return 1
    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
