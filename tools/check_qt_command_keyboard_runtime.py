import argparse
import json
import os
from pathlib import Path
import sys
import time

from check_qt_command_snapshot_runtime import ROOT, request_for, verify_snapshot


def wait(application, predicate, description="required state"):
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        application.processEvents()
        if predicate():
            return
    raise ValueError("native keyboard fixture did not reach " + description)


def main():
    parser = argparse.ArgumentParser(description="Verify injected Qt key routing through native focus and Rust activation.")
    parser.add_argument("--module-path", required=True, type=Path)
    parser.add_argument("--font", required=True, type=Path)
    parser.add_argument("--parent-x11-display", help="XTest input through an explicitly identified nested Weston X11 output")
    parser.add_argument("--parent-x11-window", type=lambda value: int(value, 0))
    args = parser.parse_args()
    if bool(args.parent_x11_display) != (args.parent_x11_window is not None):
        parser.error("supply both parent X11 display and Weston output window")
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    os.environ.setdefault("QT_QUICK_BACKEND", "software")
    try:
        from PySide6.QtCore import QCoreApplication, QEvent, QObject, QPluginLoader, Qt, QUrl, qVersion
        from PySide6.QtGui import QFontDatabase, QGuiApplication, QKeyEvent
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
        component = QQmlComponent(engine)
        component.setData(('''import QtQuick
import "''' + (ROOT / "backends/quickshell/resina-qml/qml").as_uri() + '''" as Content
Window {
    width: 400; height: 300; visible: true
    property int intents: 0
    property int propagated: 0
    property int shortcutIntents: 0
    property bool revoke: false
    property bool cycleFocus: false
    property bool hideOnFocusPublication: false
    readonly property var controller: command.controller
    property var temporaryController: null
    readonly property bool nativeFocused: command.activeFocus && active && visible
    Shortcut { sequence: "Space"; context: Qt.WindowShortcut; onActivated: ++shortcutIntents }
    Item {
        anchors.fill: parent
        Keys.onPressed: (event) => { ++propagated; event.accepted = true; }
        Keys.onReleased: (event) => { ++propagated; event.accepted = true; }
        Content.ResinaCommandKeyboard {
            id: command
            objectName: "commandKeyboard"
            width: 300; height: 200
            Content.ResinaCommandView { id: view; x:44.3; y:26.1 }
        }
        Item { id: other; width:1; height:1 }
    }
    Connections {
        target: command.controller
        function onActivated() {
            const current = JSON.parse(command.controller.publication);
            if (!current.state.enabled)
                throw new Error("intent preceded committed availability");
            ++intents;
            if (revoke)
                command.controller.dispatch('{"kind":"availability","enabled":false}');
        }
        function onPublicationChanged() {
            view.invalidate();
            if (hideOnFocusPublication && JSON.parse(command.controller.publication).state.focused) {
                hideOnFocusPublication = false;
                command.visible = false;
            }
            if (cycleFocus) {
                cycleFocus = false;
                other.forceActiveFocus();
                command.forceActiveFocus();
            }
        }
    }
    function focusCommand() { command.forceActiveFocus(); }
    function focusOther() { other.forceActiveFocus(); }
    function hideCommand() { command.visible = false; }
    function showCommand() { command.visible = true; }
    function prepareView(source) {
        const request = JSON.parse(source);
        request.activation = JSON.parse(command.controller.publication).state;
        view.prepare(JSON.stringify(request), JSON.stringify({x:44.3,y:26.1,
            deviceScale:view.nativeDeviceScale,samplesPerAxis:4}));
        return JSON.stringify(request);
    }
    function observation() { return JSON.stringify({ready:view.paintReady,
        snapshot:view.snapshot,error:command.routingError}); }
    function makeTemporary() {
        const component = Qt.createComponent(@@KEYBOARD_SOURCE@@);
        if (component.status !== Component.Ready) throw new Error(component.errorString());
        const item = component.createObject(contentItem, {width:10,height:10});
        temporaryController = item.controller;
        item.controller.dispatch('{"kind":"availability","enabled":true}');
        item.forceActiveFocus();
        return item;
    }
}
''').replace("@@KEYBOARD_SOURCE@@", json.dumps((ROOT / "backends/quickshell/resina-qml/qml/ResinaCommandKeyboard.qml").as_uri())).encode(), QUrl())
        window = component.create()
        if not isinstance(window, QQuickWindow):
            raise ValueError("keyboard fixture failed: " + repr(component.errors()))
        controller = window.property("controller")

        def dispatch(event):
            if not controller.dispatch(json.dumps(event)):
                raise ValueError(controller.property("lastError"))

        def current():
            return json.loads(controller.property("publication"))

        def focus():
            window.requestActivate()
            window.focusCommand()
            wait(app, lambda: window.isExposed() and window.property("nativeFocused")
                 and current()["state"]["focused"], "actual exposed native focus")

        def key(code, down, modifiers=Qt.NoModifier, repeat=False):
            event = QKeyEvent(QEvent.KeyPress if down else QEvent.KeyRelease, code, modifiers, "", repeat)
            QCoreApplication.sendEvent(window, event)
            app.processEvents()

        def assert_result(intents, held=None):
            if window.property("intents") != intents or current()["state"]["hold"] != held:
                raise ValueError("native key routing violated activation timing or held ownership")
            if json.loads(window.observation())["error"]:
                raise ValueError("native key routing failed diagnostically")
            if window.property("shortcutIntents"):
                raise ValueError("a command gesture also reached a product shortcut")

        dispatch({"kind":"availability","enabled":True})
        focus()
        key(Qt.Key_Space, True)
        assert_result(0, {"kind":"key","key":"space"})
        for _ in range(3):
            key(Qt.Key_Space, False, repeat=True)
            key(Qt.Key_Space, True, repeat=True)
            assert_result(0, {"kind":"key","key":"space"})
        key(Qt.Key_Space, False, Qt.ControlModifier)
        assert_result(1)
        key(Qt.Key_Return, True)
        assert_result(2, {"kind":"key","key":"enter"})
        key(Qt.Key_Return, True, repeat=True)
        key(Qt.Key_Return, False, Qt.ShiftModifier)
        assert_result(2)
        key(Qt.Key_Enter, True, Qt.KeypadModifier)
        key(Qt.Key_Enter, False, Qt.KeypadModifier)
        assert_result(3)
        before = window.property("propagated")
        for code, modifiers in ((Qt.Key_Space, Qt.ControlModifier), (Qt.Key_Return, Qt.ShiftModifier),
                                (Qt.Key_Left, Qt.NoModifier)):
            key(code, True, modifiers)
            key(code, False, modifiers)
        assert_result(3)
        if window.property("propagated") != before + 6:
            raise ValueError("router swallowed non-command keys or modified new gestures")
        key(Qt.Key_Space, True)
        window.focusOther()
        wait(app, lambda: not current()["state"]["focused"] and current()["state"]["hold"] is None, "focus-loss cancellation")
        key(Qt.Key_Space, False)
        assert_result(3)
        focus()
        window.setProperty("cycleFocus", True)
        key(Qt.Key_Space, True)
        wait(app, lambda: current()["state"]["hold"] is None, "transient focus cancellation")
        key(Qt.Key_Space, False)
        assert_result(3)
        key(Qt.Key_Space, True)
        window.hideCommand()
        wait(app, lambda: not current()["state"]["focused"] and current()["state"]["hold"] is None,
             "hidden-item cancellation")
        key(Qt.Key_Return, True)
        key(Qt.Key_Return, False)
        assert_result(3)
        window.showCommand()
        focus()
        key(Qt.Key_Space, False)
        assert_result(3)
        key(Qt.Key_Space, True)
        window.hide()
        wait(app, lambda: not current()["state"]["focused"] and current()["state"]["hold"] is None, "hidden-window cancellation")
        window.show()
        focus()
        key(Qt.Key_Space, False)
        assert_result(3)
        dispatch({"kind":"focus","focused":False})
        window.setProperty("hideOnFocusPublication", True)
        key(Qt.Key_Return, True)
        wait(app, lambda: not current()["state"]["focused"] and current()["state"]["hold"] is None,
             "publication-time visibility cancellation")
        assert_result(3)
        window.showCommand()
        focus()
        key(Qt.Key_Return, False)
        assert_result(3)
        window.setProperty("revoke", True)
        key(Qt.Key_Return, True)
        wait(app, lambda: not current()["state"]["enabled"] and current()["state"]["hold"] is None, "product revocation")
        key(Qt.Key_Return, False)
        assert_result(4)
        dispatch({"kind":"availability","enabled":True})
        window.setProperty("revoke", False)
        focus()
        key(Qt.Key_Space, True)
        request = json.loads(window.prepareView(json.dumps(request_for("light", "Reconnect"))))
        wait(app, lambda: json.loads(window.observation())["ready"], "checked native pressed drawing")
        snapshot = json.loads(json.loads(window.observation())["snapshot"])
        cases = json.loads((ROOT / "conformance/interaction/command-states-cases.json").read_text())
        expected = next(case["expected"] for case in cases if case["name"] == "space hold")
        verify_snapshot(snapshot, request, expected)
        if window.grabWindow().isNull():
            raise ValueError("native keyboard-held composed drawing did not render")
        key(Qt.Key_Space, False)
        assert_result(5)
        wait(app, lambda: not json.loads(window.observation())["ready"])
        if args.parent_x11_display:
            if app.platformName() != "wayland":
                raise ValueError("parent X11 injection requires an actual Wayland Qt client")
            import ctypes
            import ctypes.util
            xlib = ctypes.CDLL(ctypes.util.find_library("X11"))
            xtest = ctypes.CDLL(ctypes.util.find_library("Xtst"))
            xlib.XOpenDisplay.argtypes = [ctypes.c_char_p]
            xlib.XOpenDisplay.restype = ctypes.c_void_p
            xlib.XSetInputFocus.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
            xlib.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
            xlib.XKeysymToKeycode.restype = ctypes.c_ubyte
            xlib.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
            xlib.XCloseDisplay.argtypes = [ctypes.c_void_p]
            xtest.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
            display = xlib.XOpenDisplay(args.parent_x11_display.encode())
            if not display:
                raise ValueError("cannot open the parent X11 display")
            held_codes = set()
            records = []

            class NativeKeys(QObject):
                def eventFilter(self, watched, event):
                    if event.type() in (QEvent.KeyPress, QEvent.KeyRelease):
                        records.append({"key":event.key(), "down":event.type() == QEvent.KeyPress,
                                        "repeat":event.isAutoRepeat(), "spontaneous":event.spontaneous(),
                                        "scan":event.nativeScanCode()})
                    return False

            observer = NativeKeys(window)
            window.installEventFilter(observer)

            def native_key(symbol, code, down):
                start = len(records)
                native_code = xlib.XKeysymToKeycode(display, symbol)
                if not native_code or not xtest.XTestFakeKeyEvent(display, native_code, down, 0):
                    raise ValueError("parent XTest key injection failed")
                if down:
                    held_codes.add(native_code)
                else:
                    held_codes.discard(native_code)
                xlib.XSync(display, False)
                wait(app, lambda: any(record["key"] == code and record["down"] == down
                                     and not record["repeat"] for record in records[start:]), "compositor key delivery")
                delivered = next(record for record in records[start:]
                                 if record["key"] == code and record["down"] == down and not record["repeat"])
                if not delivered["spontaneous"] or not delivered["scan"]:
                    raise ValueError("parent injection did not traverse native Wayland key delivery")

            try:
                xlib.XSetInputFocus(display, args.parent_x11_window, 2, 0)
                xlib.XSync(display, False)
                focus()
                native_key(0x20, Qt.Key_Space, True)
                assert_result(5, {"kind":"key","key":"space"})
                wait(app, lambda: any(record["key"] == Qt.Key_Space and record["repeat"]
                                     for record in records), "native platform repeat metadata")
                assert_result(5, {"kind":"key","key":"space"})
                native_key(0xffe3, Qt.Key_Control, True)
                native_key(0x20, Qt.Key_Space, False)
                assert_result(6)
                native_key(0xffe3, Qt.Key_Control, False)
                native_key(0xff0d, Qt.Key_Return, True)
                assert_result(7, {"kind":"key","key":"enter"})
                native_key(0xff0d, Qt.Key_Return, False)
                assert_result(7)
                native_key(0xffe3, Qt.Key_Control, True)
                native_key(0x20, Qt.Key_Space, True)
                native_key(0x20, Qt.Key_Space, False)
                native_key(0xffe3, Qt.Key_Control, False)
                assert_result(7)
                print("Parent XTest input traversed Weston/wl_keyboard into spontaneous Qt delivery with native scan codes and repeat metadata; this is injected input, not physical-device qualification.")
            finally:
                for code in held_codes:
                    xtest.XTestFakeKeyEvent(display, code, False, 0)
                xlib.XSync(display, False)
                xlib.XCloseDisplay(display)
        # Closing the new-gesture gate must preserve an existing matching
        # release and must not claim a different held-key shortcut override.
        owner = window.findChild(QObject, "commandKeyboard")
        focus()
        baseline = window.property("intents")
        owner.setProperty("acceptsNewGestures", False)
        key(Qt.Key_Return, True)
        key(Qt.Key_Return, False)
        assert_result(baseline)
        owner.setProperty("acceptsNewGestures", True)
        key(Qt.Key_Space, True)
        owner.setProperty("acceptsNewGestures", False)
        key(Qt.Key_Return, True)
        key(Qt.Key_Return, False)
        assert_result(baseline, {"kind":"key","key":"space"})
        key(Qt.Key_Space, False)
        assert_result(baseline + 1)
        owner.setProperty("acceptsNewGestures", True)
        temporary = window.makeTemporary()
        temporary_controller = window.property("temporaryController")
        wait(app, lambda: json.loads(temporary_controller.property("publication"))["state"]["focused"],
             "temporary native keyboard focus")
        key(Qt.Key_Space, True)
        if json.loads(temporary_controller.property("publication"))["state"]["hold"] != {"kind":"key","key":"space"}:
            raise ValueError("temporary keyboard owner did not arm its native gesture")
        retired, destroyed, retired_intents = [], [], []
        temporary_controller.publicationChanged.connect(
            lambda: retired.append(json.loads(temporary_controller.property("publication"))))
        temporary_controller.activated.connect(lambda: retired_intents.append(True))
        temporary.destroyed.connect(lambda: destroyed.append(True))
        temporary.deleteLater()
        QCoreApplication.sendPostedEvents(None, QEvent.DeferredDelete)
        app.processEvents()
        if not destroyed or not retired or retired_intents or retired[-1]["state"]["hold"] is not None:
            raise ValueError("actual keyboard-owner destruction failed to cancel its armed gesture")
        window.close()
        window.deleteLater()
        app.processEvents()
    except Exception as error:
        print("FAIL Qt command keyboard runtime: " + str(error), file=sys.stderr)
        if "controller" in locals():
            print(controller.property("publication"), file=sys.stderr)
            print("nativeFocused=" + str(window.property("nativeFocused"))
                  + " active=" + str(window.isActive()) + " exposed=" + str(window.isExposed())
                  + " intents=" + str(window.property("intents")), file=sys.stderr)
        return 1
    print("Injected Qt keys matched live Rust activation, actual native focus, repeat filtering, interruption cancellation and checked pressed drawing on " + app.platformName())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
