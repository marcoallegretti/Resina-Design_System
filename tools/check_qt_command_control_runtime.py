import argparse
import copy
import json
import os
from pathlib import Path
import sys
import time

from check_qt_command_snapshot_runtime import ROOT, request_for, verify_snapshot
import conformance_font


def wait(app, predicate, description):
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        app.processEvents()
        if predicate():
            return
    raise ValueError("integrated command did not reach " + description)


def main():
    parser = argparse.ArgumentParser(description="Verify the integrated static Qt command control with injected native-window input.")
    parser.add_argument("--module-path", required=True, type=Path)
    parser.add_argument("--font-dir", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--motion", action="store_true", help="Exercise explicit arithmetic spring channels in the same native control")
    parser.add_argument("--parent-x11-display", help="XTest injection through a named nested Weston output")
    parser.add_argument("--parent-x11-window", type=lambda value:int(value,0))
    args = parser.parse_args()
    if bool(args.parent_x11_display) != (args.parent_x11_window is not None):
        parser.error("supply both parent X11 display and output window")
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    os.environ.setdefault("QT_QUICK_BACKEND", "software")
    try:
        from PySide6.QtCore import QCoreApplication, QEvent, QPointF, QPluginLoader, QObject, Qt, QUrl, qVersion
        from PySide6.QtGui import QGuiApplication, QKeyEvent, QMouseEvent, QPointingDevice
        from PySide6.QtQml import QQmlComponent, QQmlEngine
        from PySide6.QtQuick import QQuickWindow
        if qVersion() != "6.11.2":
            raise ValueError("requires Qt 6.11.2")
        app = QGuiApplication([])
        plugin = QPluginLoader(str(args.module_path.resolve() / "Resina/Runtime/libResina_Runtime.so"))
        if plugin.instance() is None:
            raise ValueError(plugin.errorString())
        conformance_font.register_qt(args.font_dir)
        engine = QQmlEngine()
        engine.addImportPath(str(args.module_path.resolve()))
        component = QQmlComponent(engine, QUrl.fromLocalFile(str(ROOT / "conformance/quickshell/command-control-fixture.qml")))
        window = component.create()
        if window is None:
            raise ValueError("; ".join(error.toString() for error in component.errors()))
        control = window.findChild(QObject, "commandControl")
        snapshot_property = control.metaObject().property(control.metaObject().indexOfProperty("snapshot"))
        if snapshot_property.isWritable():
            raise ValueError("native command snapshot became writable through its QML alias")
        views = [child for child in control.childItems()
                 if child.metaObject().indexOfProperty("nativeDeviceScale") >= 0
                 and child.metaObject().indexOfProperty("paintReady") >= 0]
        if len(views) != 1:
            raise ValueError("integrated command must own one complete decorative view")
        premature_paint = []
        def observe_visibility():
            if views[0].isVisible() and not json.loads(window.observation())["ready"]:
                premature_paint.append(True)
        views[0].visibleChanged.connect(observe_visibility)
        window.requestActivate()
        wait(app, lambda: window.isActive() and window.isExposed(), "exposed active native window")
        if views[0].property("nativeDeviceScale") != window.effectiveDevicePixelRatio():
            raise ValueError("Command view density is not its window's device pixel ratio")
        args.output_dir.mkdir(parents=True, exist_ok=True)

        def observe():
            return json.loads(window.observation())

        source = request_for("light", "Reconnect")
        if args.motion:
            source["content"]["environment"]["accessibilityPreferences"]["reducedMotion"] = False
            channels = json.loads((ROOT / "conformance/ir/command-motion-request.json").read_text())["channels"]
            window.setProperty("motionChannels", json.dumps(channels))

        def prepare(request):
            window.setProperty("configuration", json.dumps(request) if isinstance(request, dict) else request)
            wait(app, lambda: observe()["ready"] or bool(observe()["error"]), "preparation result")

        def check():
            wait(app, lambda: observe()["ready"] or bool(observe()["error"]), "coherent publication")
            result = observe()
            if result["readinessViolations"] or premature_paint:
                raise ValueError("command advertised readiness before arming native input")
            if result["error"] or not result["ready"]:
                raise ValueError(result["error"] or "missing command publication")
            current = copy.deepcopy(source)
            current.update(activation=result["state"], hovered=result["hovered"], focusable=result["state"]["enabled"])
            held = result["state"]["hold"]
            pressed = bool(held and (held["kind"] == "key" or held["inside"]))
            states = ["rest"] if not result["hovered"] and not pressed else []
            if result["hovered"]:
                states.append("hover")
            if result["state"]["focused"]:
                states.append("focused")
            if pressed:
                states.append("pressed")
            if not result["state"]["enabled"]:
                if states == ["rest"]:
                    states = []
                states.append("disabled")
            expected = {"schemaVersion":"0.1.0","states":states or ["rest"]}
            actual = json.loads(result["snapshot"])["paint"]["paint"]["body"]["states"]
            if actual != expected:
                raise ValueError(f"body signals disagree: expected {expected}, actual {actual}, state {result['state']}")
            motion = json.loads(result["motion"])["motion"] if args.motion else None
            verify_snapshot(json.loads(result["snapshot"]), current, expected, motion)
            if json.loads(result["target"]) != json.loads(result["snapshot"])["hitRegion"]:
                raise ValueError("native input and visible command reservations differ")
            return result

        def capture(name):
            image = window.grabWindow()
            if image.isNull() or not image.save(str(args.output_dir / name)):
                raise ValueError("native control capture failed")
            if len({image.pixelColor(x,y).rgba() for x in range(image.width()) for y in range(image.height())}) < 3:
                raise ValueError("native control capture lacks body/glyph drawing")
            return image

        def effects(expected, hold=None):
            result = observe()
            if result["effects"] != expected or result["state"]["hold"] != hold:
                raise ValueError(f"activation/cancellation differs: expected effects={expected}, hold={hold}; actual effects={result['effects']}, hold={result['state']['hold']}, ready={result['ready']}, error={result['error']}")

        def mouse(kind, x=80, y=80):
            event = QMouseEvent(kind, QPointF(x,y), QPointF(x,y), QPointF(x,y),
                Qt.NoButton if kind == QEvent.MouseMove else Qt.LeftButton,
                Qt.NoButton if kind == QEvent.MouseButtonRelease else Qt.LeftButton,
                Qt.NoModifier, QPointingDevice.primaryPointingDevice())
            QCoreApplication.sendEvent(window, event)

        def key(kind, code):
            QCoreApplication.sendEvent(window, QKeyEvent(kind, code, Qt.NoModifier))

        prepare(source)
        disabled = check()
        if disabled["state"]["enabled"] or json.loads(disabled["snapshot"])["accessibility"]["focusable"]:
            raise ValueError("explicit excluding disabled policy was not preserved")
        if window.invoke():
            raise ValueError("disabled action was accepted")
        effects(0)
        window.availability(True)
        stable = check()["target"]
        # No event-loop turn between hover, down and up. Existing capture must
        # terminate even while the new drawing is being decoded.
        mouse(QEvent.MouseMove)
        mouse(QEvent.MouseButtonPress)
        mouse(QEvent.MouseButtonRelease)
        effects(1)
        if check()["target"] != stable:
            raise ValueError("native feedback changed stable reservation")
        hover_image = capture("enabled-hover.png")
        mouse(QEvent.MouseButtonPress)
        held = observe()["state"]["hold"]
        if not held or not held["inside"]:
            raise ValueError("integrated input did not hold a native pointer")
        check()
        if args.motion:
            start = json.loads(observe()["motion"])["time"]
            wait(app, lambda: observe()["ready"] and json.loads(observe()["motion"])["time"] > start + .4, "visible sampled pressed feedback")
            primary = control.property("coordinateFrame")
            alternates = [child for child in control.childItems()
                          if child is not primary and child.metaObject().indexOfProperty("paintReady") >= 0]
            if len(alternates) != 1:
                raise ValueError("a motion control must own one alternate frame view")
            def verify_alternate_presented():
                wait(app, lambda: observe()["ready"] and alternates[0].isVisible(), "alternate motion frame presented")
                if primary.isVisible():
                    raise ValueError("the primary and alternate frame views are both presented")
                if primary.mapToScene(QPointF(0, 0)) != alternates[0].mapToScene(QPointF(0, 0)):
                    raise ValueError("the primary frame view is not placed where the presented alternate frame is")
            verify_alternate_presented()
            wait(app, lambda: primary.isVisible(), "primary motion frame presented again")
            verify_alternate_presented()
        if capture("pressed.png") == hover_image:
            raise ValueError("native pressed feedback did not change the drawing")
        mouse(QEvent.MouseMove, 480, 380)
        if observe()["state"]["hold"]["inside"]:
            raise ValueError("native captured outside movement retained inside state")
        mouse(QEvent.MouseMove)
        mouse(QEvent.MouseButtonRelease)
        effects(2)
        check()
        window.focusCommand()
        wait(app, lambda: observe()["state"]["focused"] and observe()["ready"], "observed native focus")
        key(QEvent.KeyPress, Qt.Key_Space)
        effects(2, {"kind":"key","key":"space"})
        key(QEvent.KeyRelease, Qt.Key_Space)
        effects(3)
        check()
        key(QEvent.KeyPress, Qt.Key_Return)
        effects(4, {"kind":"key","key":"enter"})
        key(QEvent.KeyRelease, Qt.Key_Return)
        check()
        key(QEvent.KeyPress, Qt.Key_Space)
        check()
        if not window.invoke():
            raise ValueError("available semantic invocation rejected")
        effects(5)
        key(QEvent.KeyRelease, Qt.Key_Space)
        effects(5)
        check()
        window.setProperty("revoke", True)
        window.invoke()
        effects(6)
        if check()["state"]["enabled"] or window.invoke():
            raise ValueError("product revocation failed to withdraw available action")
        effects(6)
        window.setProperty("revoke", False)
        window.availability(True)
        window.focusCommand()
        wait(app, lambda: observe()["state"]["focused"] and observe()["ready"], "focus restored after revocation")
        check()
        key(QEvent.KeyPress, Qt.Key_Space)
        effects(6, {"kind":"key","key":"space"})
        replacement = copy.deepcopy(source)
        replacement["description"] = "Reconfigured while held."
        source = replacement
        prepare(source)
        effects(6)
        key(QEvent.KeyRelease, Qt.Key_Space)
        effects(6)
        check()
        # Reject malformed author input before native channel substitution, and
        # withdraw paint, hit region and semantics together. Recovery is explicit.
        invalid = [json.dumps(source)[:-1] + ',"hovered":false}',
                   json.dumps(source)[:-1] + ',"unknown":true}']
        for member, value in (("activation", {"schemaVersion":"0.1.0","enabled":True,"focused":True,"hold":{"kind":"key","key":"escape"}}),
                              ("description", 4), ("componentMinimum", {"width":-1,"height":48})):
            request = copy.deepcopy(source)
            request[member] = value
            invalid.append(request)
        request = copy.deepcopy(source)
        request["body"]["minimumContentContrast"] = 100
        invalid.append(request)
        for request in invalid:
            prepare(request)
            result = observe()
            if not result["error"] or result["snapshot"] or result["target"] or result["ready"]:
                raise ValueError("rejected configuration retained a production channel")
            mouse(QEvent.MouseButtonPress)
            mouse(QEvent.MouseButtonRelease)
            key(QEvent.KeyPress, Qt.Key_Return)
            window.invoke()
            effects(6)
            prepare(source)
            check()
        for samples in (0,9,256):
            control.setProperty("samplesPerAxis", samples)
            wait(app, lambda: bool(observe()["error"]), "invalid raster policy rejection")
            if observe()["ready"] or observe()["target"] or observe()["snapshot"]:
                raise ValueError("invalid raster policy retained a production channel")
            control.setProperty("samplesPerAxis", 4)
            check()
        window.setProperty("disabledFocusable", True)
        wait(app, lambda: bool(observe()["error"]), "unsupported discoverable disabled policy rejection")
        if observe()["ready"] or observe()["target"] or observe()["snapshot"]:
            raise ValueError("unsupported native policy retained publication")
        window.setProperty("disabledFocusable", False)
        check()
        mouse(QEvent.MouseButtonPress)
        held = observe()["state"]["hold"]
        if not held or not held["inside"]:
            raise ValueError("transform cancellation fixture did not hold native input")
        window.rotateCarrier(10)
        wait(app, lambda: bool(observe()["error"]) and not observe()["target"] and not observe()["snapshot"], "transform withdrawal")
        wait(app, lambda: observe()["state"]["hold"] is None, "transform gesture cancellation")
        mouse(QEvent.MouseButtonRelease)
        effects(6)
        window.rotateCarrier(0)
        window.renew()
        check()
        window.moveCommand(90)
        check()
        if observe()["state"]["hold"]:
            raise ValueError("native placement replacement retained held input")
        capture("recovered.png")
        window.showCommand(False)
        wait(app, lambda: not observe()["ready"] and not observe()["target"] and not observe()["snapshot"], "hidden channel withdrawal")
        window.showCommand(True)
        check()
        for theme, text in (("dark", "Verbindung mit gespeicherten Einstellungen wiederherstellen"),
                            ("light", "اتصل Reconnect")):
            source = request_for(theme, text)
            if args.motion:
                source["content"]["environment"]["accessibilityPreferences"]["reducedMotion"] = False
            prepare(source)
            check()
            capture("localized-" + theme + ".png")
        # A consumer can revoke availability during snapshot publication.
        window.setProperty("revokeOnReady", True)
        mouse(QEvent.MouseMove, 120, 80)
        result = check()
        if result["state"]["enabled"] or window.property("revokeOnReady"):
            raise ValueError("reentrant snapshot publication lost product revocation")
        if args.parent_x11_display:
            from qt_nested_input import exercise
            exercise(app,window,args.parent_x11_display,args.parent_x11_window,wait,observe,check)
        window.close()
        profile = "sampled" if args.motion else "static"
        print(f"integrated {profile} Qt command: coherent paint/label/target/semantics, native-window pointer/key routing, exact-once effects and fail-closed recovery passed (injected input)")
        return 0
    except (ImportError, OSError, RuntimeError, ValueError, TypeError, AttributeError) as error:
        print("Qt command control check failed: " + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
