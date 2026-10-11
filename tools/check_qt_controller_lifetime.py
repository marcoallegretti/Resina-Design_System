"""Compiled QObject destruction regressions; synthetic operations are not native input evidence."""
import argparse
import json
from pathlib import Path
import subprocess
import sys


def worker(args):
    from PySide6.QtCore import QCoreApplication, QEvent, QPluginLoader, QUrl
    from PySide6.QtQml import QQmlComponent, QQmlEngine
    from PySide6 import QtQuick  # Load the host plugin dependency.
    from shiboken6 import isValid

    pointer_case = args.scenario in ("pointer-publication", "pointer-capture", "pointer-activated", "pointer-stale", "target-publication")
    if pointer_case:
        pointer_worker(args)
        return
    app = QCoreApplication([])
    loader = QPluginLoader(str(args.module_path.resolve() / "Resina/Runtime/libResina_Runtime.so"))
    if loader.instance() is None:
        raise ValueError(loader.errorString())
    engine = QQmlEngine()
    engine.addImportPath(str(args.module_path.resolve()))
    component = QQmlComponent(engine)
    component.setData(("import Resina.Runtime\n" + args.controller + " {}").encode(), QUrl())
    controller = component.create()
    if controller is None:
        raise ValueError(str(component.errors()))
    if not controller.dispatch('{"kind":"availability","enabled":true}'):
        raise ValueError("cannot enable controller")
    effects = []
    destroyed = []
    controller.activated.connect(lambda *values: effects.append(("activated", values)))
    controller.captureRequested.connect(lambda *values: effects.append(("capture", values)))

    def dispose(*_):
        controller.deleteLater()
        app.sendPostedEvents(None, QEvent.DeferredDelete)
        if isValid(controller):
            raise ValueError("deferred destruction did not execute")
        destroyed.append(True)

    if args.scenario == "baseline":
        dispose()
    else:
        event = {"kind": "invoke"}
        if args.scenario == "capture-release":
            controller.dispatch('{"kind":"pointerDown","id":"native-lifetime","inside":true}')
            effects.clear()
            event = {"kind": "pointerUp", "id": "native-lifetime", "inside": True}
        elif args.scenario == "capture-acquire":
            event = {"kind": "pointerDown", "id": "native-lifetime", "inside": True}
        signal = (controller.publicationChanged if args.scenario in ("publication", "checked")
                  else controller.activated if args.scenario == "activated"
                  else controller.captureRequested)
        signal.connect(dispose)
        if args.scenario == "checked":
            controller.setChecked('{"checked":true}')
        else:
            controller.dispatch(json.dumps(event))
        if not destroyed or isValid(controller):
            raise ValueError("destructive observer did not run")
        expected = ([] if args.scenario in ("publication", "checked")
                    else ["activated"] if args.scenario == "activated" else ["capture"])
        if [name for name, _ in effects] != expected:
            raise ValueError("effect delivered after owner destruction: " + repr(effects))
    print("PASS " + args.controller + " " + args.scenario)


def pointer_worker(args):
    import os
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    os.environ.setdefault("QT_QUICK_BACKEND", "software")
    from PySide6.QtCore import QCoreApplication, QEvent, QObject, QPoint, QPluginLoader, Qt, QUrl
    from PySide6.QtGui import QGuiApplication
    from PySide6.QtQml import QQmlComponent, QQmlEngine
    from PySide6.QtTest import QTest
    from shiboken6 import isValid
    app = QGuiApplication([])
    loader = QPluginLoader(str(args.module_path.resolve() / "Resina/Runtime/libResina_Runtime.so"))
    if loader.instance() is None:
        raise ValueError(loader.errorString())
    engine = QQmlEngine()
    engine.addImportPath(str(args.module_path.resolve()))
    component = QQmlComponent(engine)
    component.setData(("""import QtQuick
import Resina.Runtime
Window {
    width:200; height:200; visible:true
    """ + args.controller + """ { id:semanticOwner; objectName:"controller" }
    CommandPointerItem {
        objectName:"pointer"; anchors.fill:parent
        controller:semanticOwner; coordinateFrame:parent
    }
}""").encode(), QUrl())
    window = component.create()
    if window is None:
        raise ValueError(str(component.errors()))
    window.requestActivate()
    for _ in range(20):
        app.processEvents()
    controller = window.findChild(QObject, "controller")
    pointer = window.findChild(QObject, "pointer")
    controller.dispatch('{"kind":"availability","enabled":true}')
    environment = json.loads((Path(__file__).resolve().parents[1] / "conformance/headless/valid-request.json").read_text())["environment"]
    environment["inputCapabilities"] = ["finePointer"]
    target = json.dumps({"schemaVersion":"0.1.0", "environment":environment,
        "visualBounds":{"x":20,"y":20,"width":100,"height":60},
        "availableBounds":{"x":0,"y":0,"width":200,"height":200},
        "componentMinimum":{"width":24,"height":24},"occupiedRegions":[]})
    destroyed = []
    effects = []
    controller.activated.connect(lambda *_: effects.append(True))
    removed_ids = []
    def dispose(*_):
        if destroyed:
            return
        current = json.loads(controller.property("publication"))
        activation = current["activation"] if args.controller == "ToggleController" else current
        hold = activation["state"]["hold"]
        if hold:
            removed_ids.append(hold["id"])
        pointer.deleteLater()
        app.sendPostedEvents(None, QEvent.DeferredDelete)
        destroyed.append(True)
    if args.scenario == "target-publication":
        pointer.targetPublicationChanged.connect(dispose)
        pointer.prepareTarget(target)
    else:
        if not pointer.prepareTarget(target):
            raise ValueError("cannot prepare pointer target: " + pointer.property("lastError"))
        if args.scenario in ("pointer-publication", "pointer-stale"):
            controller.publicationChanged.connect(dispose)
        elif args.scenario == "pointer-capture":
            controller.captureRequested.connect(dispose)
        else:
            controller.activated.connect(dispose)
        QTest.mousePress(window, Qt.LeftButton, Qt.NoModifier, QPoint(50,50))
        if args.scenario == "pointer-activated":
            QTest.mouseRelease(window, Qt.LeftButton, Qt.NoModifier, QPoint(50,50))
    if not destroyed or isValid(pointer):
        raise ValueError("native pointer observer did not destroy item")
    if effects != ([True] if args.scenario == "pointer-activated" else []):
        raise ValueError("native pointer destruction replayed activation")
    if args.scenario == "pointer-stale":
        controller.dispatch('{"kind":"availability","enabled":false}')
        controller.dispatch('{"kind":"availability","enabled":true}')
        controller.dispatch(json.dumps({"kind":"pointerDown","id":removed_ids[0],"inside":True}))
    for _ in range(3):
        app.processEvents()
    publication = json.loads(controller.property("publication"))
    activation = publication["activation"] if args.controller == "ToggleController" else publication
    if args.scenario == "pointer-stale":
        if not activation["state"]["hold"] or activation["state"]["hold"]["id"] != removed_ids[0]:
            raise ValueError("queued native removal cancelled a replacement gesture")
        controller.dispatch('{"kind":"cancel"}')
    elif activation["state"]["hold"] is not None:
        raise ValueError("removed native pointer left a live hold")
    window.close()
    print("PASS " + args.controller + " " + args.scenario)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--module-path", type=Path, required=True)
    parser.add_argument("--controller", choices=("CommandController", "ToggleController"), default="CommandController")
    parser.add_argument("--scenario", choices=("baseline", "publication", "capture-acquire", "capture-release", "activated", "checked", "pointer-publication", "pointer-capture", "pointer-activated", "pointer-stale", "target-publication"))
    args = parser.parse_args()
    if args.scenario:
        worker(args)
        return 0
    scenarios = ["baseline", "publication", "capture-acquire", "capture-release", "activated", "pointer-publication", "pointer-capture", "pointer-activated", "pointer-stale", "target-publication"]
    if args.controller == "ToggleController":
        scenarios.append("checked")
    for scenario in scenarios:
        result = subprocess.run([sys.executable, str(Path(__file__).resolve()), "--module-path",
                                 str(args.module_path.resolve()), "--controller", args.controller,
                                 "--scenario", scenario], capture_output=True, text=True, timeout=30)
        if result.returncode or "Traceback" in result.stderr:
            print(f"FAIL {args.controller} {scenario}: exit {result.returncode}\n" + result.stdout + result.stderr,
                  file=sys.stderr)
            return 1
        print(result.stdout.strip())
    return 0


if __name__ == "__main__":
    sys.exit(main())
