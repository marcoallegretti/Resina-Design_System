import argparse
import json
import os
from pathlib import Path
import sys
import time

from check_qt_command_snapshot_runtime import ROOT, request_for, verify_snapshot


def wait(app, predicate, description):
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        app.processEvents()
        if predicate():
            return
    raise ValueError("pointer fixture did not reach " + description)


def main():
    parser = argparse.ArgumentParser(description="Verify native Qt pointer ownership against Rust hit membership and activation.")
    parser.add_argument("--module-path", required=True, type=Path)
    parser.add_argument("--font", required=True, type=Path)
    parser.add_argument("--parent-x11-display")
    parser.add_argument("--parent-x11-window", type=lambda value: int(value, 0))
    args = parser.parse_args()
    if bool(args.parent_x11_display) != (args.parent_x11_window is not None):
        parser.error("supply both parent X11 display and Weston output window")
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    os.environ.setdefault("QT_QUICK_BACKEND", "software")
    try:
        from PySide6.QtCore import QCoreApplication, QEvent, QObject, QPointF, QPluginLoader, Qt, QUrl, qVersion
        from PySide6.QtGui import QFontDatabase, QGuiApplication, QInputDevice, QKeyEvent, QMouseEvent, QPointingDevice
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
import Resina.Runtime 0.1
import "''' + (ROOT / "backends/quickshell/resina-qml/qml").as_uri() + '''" as Content
Window {
    width:400; height:300; visible:true
    property int intents:0
    property bool hideOnHold:false
    property bool revoke:false
    readonly property var controller: owner.controller
    Item { id: other; width:1; height:1 }
    Item {
    id:clipHost; width:parent.width; height:parent.height
    Content.ResinaCommandInput {
        id:owner; width:400; height:300; coordinateFrame:view
        Content.ResinaCommandView { id:view; x:44.3; y:26.1 }
        Component.onCompleted: pointerOwner.objectName="pointer"
    }
    }
    Connections {
        target:owner.controller
        function onPublicationChanged() {
            view.invalidate();
            if (hideOnHold && JSON.parse(owner.controller.publication).state.hold) {
                hideOnHold = false;
                owner.visible = false;
            }
        }
        function onActivated() {
            const state = JSON.parse(owner.controller.publication).state;
            if (!state.enabled || state.hold) throw new Error("pointer intent preceded committed release");
            ++intents;
            if (revoke) owner.controller.dispatch('{"kind":"availability","enabled":false}');
        }
    }
    function prepareView(source) {
        const request=JSON.parse(source);
        request.activation=JSON.parse(owner.controller.publication).state;
        view.prepare(JSON.stringify(request), JSON.stringify({x:44.3,y:26.1,
            deviceScale:view.nativeDeviceScale,samplesPerAxis:4}));
        return JSON.stringify(request);
    }
    function observation() { return JSON.stringify({ready:view.paintReady,snapshot:view.snapshot}); }
    function prepareTarget(source) { return owner.prepareTarget(source); }
    function focusOther() { other.forceActiveFocus(); }
    function focusOwner() { owner.forceActiveFocus(); }
    function hideOwner() { owner.visible=false; }
    function showOwner() { owner.visible=true; }
    function shiftFrame() { view.x += 1; }
    function setOwnerClip(value) { owner.clip=value; }
    function setAncestorClip(width,value) { clipHost.width=width; clipHost.clip=value; }
}
''').encode(), QUrl())
        window = component.create()
        if not isinstance(window, QQuickWindow):
            raise ValueError("pointer fixture failed: " + repr(component.errors()))
        controller = window.property("controller")
        pointer = window.findChild(QObject, "pointer")
        last_event=[None]

        def state():
            return json.loads(controller.property("publication"))["state"]

        def dispatch(event):
            if not controller.dispatch(json.dumps(event)):
                raise ValueError(controller.property("lastError"))

        def prepare():
            request = json.loads(window.prepareView(json.dumps(request_for("light", "Reconnect"))))
            wait(app, lambda: json.loads(window.observation())["ready"], "checked command drawing")
            snapshot = json.loads(json.loads(window.observation())["snapshot"])
            region = snapshot["hitRegion"]
            source = {"schemaVersion":"0.1.0", "environment":request["content"]["environment"],
                      "visualBounds":region["bounds"], "availableBounds":request["availableBounds"],
                      "componentMinimum":region["minimumSize"], "occupiedRegions":request["occupiedRegions"]}
            if not window.prepareTarget(json.dumps(source)):
                raise ValueError(pointer.property("lastError"))
            if json.loads(pointer.property("targetPublication")) != region:
                raise ValueError("native target changed the checked stable reservation")
            return request, source

        def mouse(x, y, kind, modifiers=Qt.NoModifier, button=Qt.LeftButton, device=None):
            down = kind != QEvent.MouseButtonRelease
            event = QMouseEvent(kind, QPointF(x,y), QPointF(x,y), QPointF(x,y),
                                Qt.NoButton if kind == QEvent.MouseMove else button,
                                button if down else Qt.NoButton, modifiers,
                                device or QPointingDevice.primaryPointingDevice())
            QCoreApplication.sendEvent(window, event)
            app.processEvents()
            last_event[0]=event

        def assert_result(intents, inside=None):
            from shiboken6 import getCppPointer
            event=last_event[0]
            held = None if inside is None else {"kind":"pointer",
                "id":f"qt-mouse-{getCppPointer(event.pointingDevice())[0]:x}-{event.point(0).id()}","inside":inside}
            if window.property("intents") != intents or state()["hold"] != held:
                raise ValueError("pointer activation timing or held membership differs")
            grabber=event.exclusiveGrabber(event.point(0)) if event is not None else None
            if held and grabber != pointer:
                raise ValueError("Rust pointer hold has no actual Qt exclusive grab")
            if not held and grabber == pointer:
                raise ValueError("released lifecycle retained its native Qt grab")

        dispatch({"kind":"availability","enabled":True})
        window.requestActivate()
        wait(app, lambda: window.isExposed() and window.isActive(), "active exposed native window")
        window.focusOther()
        wait(app, lambda: not state()["focused"], "actual unfocused pointer operation")
        request, source = prepare()
        from fractions import Fraction
        bounds=json.loads(pointer.property("targetPublication"))["bounds"]
        frame=pointer.property("coordinateFrame")
        for x,y in ((bounds["x"],bounds["y"]),(bounds["x"]+bounds["width"],bounds["y"]),
                    (bounds["x"],bounds["y"]+bounds["height"]),(10,10),(-50,-50)):
            native=pointer.mapFromItem(frame,QPointF(x,y))
            decoded=frame.mapFromItem(pointer,native)
            expected=(Fraction(bounds["x"]) <= Fraction(decoded.x()) < Fraction(bounds["x"])+Fraction(bounds["width"])
                      and Fraction(bounds["y"]) <= Fraction(decoded.y()) < Fraction(bounds["y"])+Fraction(bounds["height"]))
            if pointer.contains(native) != expected:
                raise ValueError("native mapped membership differs from independent exact arithmetic")
        # A competing primary mouse press cannot cancel a held keyboard gesture.
        window.focusOwner()
        wait(app,lambda:state()["focused"],"successful native keyboard focus")
        QCoreApplication.sendEvent(window,QKeyEvent(QEvent.KeyPress,Qt.Key_Space,Qt.NoModifier))
        mouse(70,50,QEvent.MouseButtonPress)
        mouse(70,50,QEvent.MouseButtonRelease)
        if state()["hold"] != {"kind":"key","key":"space"}:
            raise ValueError("pointer ownership displaced the first held keyboard gesture")
        dispatch({"kind":"cancel"})
        window.focusOther()
        wait(app,lambda:not state()["focused"],"restored unfocused pointer operation")
        invalid=dict(source,occupiedRegions=[source["visualBounds"]])
        if window.prepareTarget(json.dumps(invalid)) or pointer.property("targetPublication") or not pointer.property("lastError"):
            raise ValueError("conflicting target preparation did not withdraw with a diagnostic")
        request,source=prepare()
        window.setAncestorClip(100,True)
        app.processEvents()
        if window.prepareTarget(json.dumps(source)) or pointer.property("targetPublication") or not pointer.property("lastError"):
            raise ValueError("actual ancestor clipping admitted an incomplete native target")
        window.setAncestorClip(400,False)
        request,source=prepare()
        frame.setScale(1.01)
        if window.prepareTarget(json.dumps(source)) or pointer.property("targetPublication"):
            raise ValueError("unsupported native scale admitted a target")
        frame.setScale(1)
        request,source=prepare()
        corner=pointer.mapFromItem(frame,QPointF(bounds["x"]+0.01,bounds["y"]+0.01))
        mouse(corner.x(),corner.y(),QEvent.MouseButtonPress)
        assert_result(0,True)
        mouse(390,280,QEvent.MouseButtonRelease)
        assert_result(0)
        mouse(70,50,QEvent.MouseButtonPress)
        assert_result(0,True)
        other_device=QPointingDevice("injected second mouse",100001,QInputDevice.DeviceType.Mouse,
            QPointingDevice.PointerType.Generic,QInputDevice.Capability.Position,1,3)
        first_hold=state()["hold"]
        mouse(70,50,QEvent.MouseButtonPress,device=other_device)
        mouse(70,50,QEvent.MouseButtonRelease,device=other_device)
        if state()["hold"] != first_hold or window.property("intents"):
            raise ValueError("another native device identity stole or released the first gesture")
        mouse(390,280,QEvent.MouseMove)
        assert_result(0,False)
        mouse(70,50,QEvent.MouseMove)
        assert_result(0,True)
        # Actual focus loss is independent of the pointer hold.
        window.focusOwner()
        app.processEvents()
        window.focusOther()
        app.processEvents()
        assert_result(0,True)
        pressed_request = json.loads(window.prepareView(json.dumps(request)))
        wait(app, lambda: json.loads(window.observation())["ready"], "held pointer drawing")
        snapshot = json.loads(json.loads(window.observation())["snapshot"])
        if snapshot["hitRegion"] != json.loads(pointer.property("targetPublication")):
            raise ValueError("held paint preparation moved the stable native target")
        cases = json.loads((ROOT / "conformance/interaction/command-states-cases.json").read_text())
        expected = next(case["expected"] for case in cases if case["name"] == "unfocused pointer press")
        verify_snapshot(snapshot, pressed_request, expected)
        if window.grabWindow().isNull():
            raise ValueError("held native pointer drawing did not render")
        mouse(70,50,QEvent.MouseButtonRelease,Qt.ControlModifier)
        assert_result(1)
        mouse(70,50,QEvent.MouseButtonPress)
        mouse(-20,-20,QEvent.MouseMove)
        assert_result(1,False)
        mouse(-20,-20,QEvent.MouseButtonRelease)
        assert_result(1)
        for button, modifiers in ((Qt.RightButton,Qt.NoModifier),(Qt.LeftButton,Qt.ControlModifier)):
            mouse(70,50,QEvent.MouseButtonPress,modifiers,button)
            mouse(70,50,QEvent.MouseButtonRelease,modifiers,button)
            assert_result(1)
        mouse(70,50,QEvent.MouseButtonPress)
        pointer.ungrabMouse()
        app.processEvents()
        assert_result(1)
        mouse(70,50,QEvent.MouseButtonRelease)
        assert_result(1)
        mouse(70,50,QEvent.MouseButtonPress)
        dispatch({"kind":"invoke"})
        assert_result(2)
        mouse(70,50,QEvent.MouseButtonRelease)
        assert_result(2)
        mouse(70,50,QEvent.MouseButtonPress)
        window.shiftFrame()
        app.processEvents()
        mouse(70,50,QEvent.MouseButtonRelease)
        assert_result(2)
        if pointer.property("targetPublication"):
            raise ValueError("changed native mapping retained target publication")
        # Restore the fixture's explicit body/grid origin before re-preparation.
        pointer.property("coordinateFrame").setX(44.3)
        request, source = prepare()
        mouse(70,50,QEvent.MouseButtonPress)
        window.setOwnerClip(True)
        app.processEvents()
        mouse(70,50,QEvent.MouseButtonRelease)
        assert_result(2)
        if pointer.property("targetPublication"):
            raise ValueError("changed native clipping retained a target")
        window.setOwnerClip(False)
        request,source=prepare()
        window.setProperty("hideOnHold",True)
        mouse(70,50,QEvent.MouseButtonPress)
        wait(app, lambda: state()["hold"] is None, "publication-time hiding cancellation")
        assert_result(2)
        window.showOwner()
        app.processEvents()
        request, source = prepare()
        mouse(70,50,QEvent.MouseButtonPress)
        dispatch({"kind":"availability","enabled":False})
        assert_result(2)
        mouse(70,50,QEvent.MouseButtonRelease)
        assert_result(2)
        dispatch({"kind":"availability","enabled":True})
        request,source=prepare()
        mouse(70,50,QEvent.MouseButtonPress)
        window.hide()
        wait(app,lambda:state()["hold"] is None,"hidden native window cancellation")
        assert_result(2)
        window.show()
        window.requestActivate()
        wait(app,lambda:window.isExposed() and window.isActive(),"restored native window exposure")
        request,source=prepare()
        mouse(70,50,QEvent.MouseButtonRelease)
        assert_result(2)
        window.setProperty("revoke",True)
        mouse(70,50,QEvent.MouseButtonPress)
        mouse(70,50,QEvent.MouseButtonRelease)
        assert_result(3)
        if state()["enabled"]:
            raise ValueError("product availability revocation was not committed")
        dispatch({"kind":"availability","enabled":True})
        window.setProperty("revoke",False)
        request, source = prepare()
        if args.parent_x11_display:
            if app.platformName() != "wayland" or window.devicePixelRatio() != 1:
                raise ValueError("parent input requires an actual Wayland Qt client at DPR 1")
            import ctypes
            import ctypes.util
            xlib = ctypes.CDLL(ctypes.util.find_library("X11"))
            xtest = ctypes.CDLL(ctypes.util.find_library("Xtst"))
            xlib.XOpenDisplay.argtypes=[ctypes.c_char_p]
            xlib.XOpenDisplay.restype=ctypes.c_void_p
            xlib.XSetInputFocus.argtypes=[ctypes.c_void_p,ctypes.c_ulong,ctypes.c_int,ctypes.c_ulong]
            xlib.XSync.argtypes=[ctypes.c_void_p,ctypes.c_int]
            xlib.XCloseDisplay.argtypes=[ctypes.c_void_p]
            xtest.XTestFakeMotionEvent.argtypes=[ctypes.c_void_p,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_ulong]
            xtest.XTestFakeButtonEvent.argtypes=[ctypes.c_void_p,ctypes.c_uint,ctypes.c_int,ctypes.c_ulong]
            display=xlib.XOpenDisplay(args.parent_x11_display.encode())
            if not display:
                raise ValueError("cannot open parent X11 display")
            records=[]

            class NativePointer(QObject):
                def eventFilter(self, watched, event):
                    if event.type() in (QEvent.MouseMove,QEvent.MouseButtonPress,QEvent.MouseButtonRelease):
                        last_event[0]=event.clone()
                        records.append({"kind":event.type(), "x":event.position().x(),
                                        "y":event.position().y(), "spontaneous":event.spontaneous()})
                    return False

            observer=NativePointer(window)
            window.installEventFilter(observer)
            held_button=False

            def parent_move(x,y):
                if not xtest.XTestFakeMotionEvent(display,-1,round(x),round(y),0):
                    raise ValueError("parent pointer motion injection failed")
                xlib.XSync(display,False)

            def native_move(x,y):
                start=len(records)
                parent_move(origin[0]+x,origin[1]+y)
                wait(app,lambda:any(record["kind"] == QEvent.MouseMove for record in records[start:]),
                     "compositor pointer motion")
                record=next(record for record in records[start:] if record["kind"] == QEvent.MouseMove)
                if not record["spontaneous"] or (record["x"],record["y"]) != (x,y):
                    raise ValueError("parent motion did not traverse the observed native Wayland coordinates")

            def native_button(down):
                nonlocal held_button
                start=len(records)
                if not xtest.XTestFakeButtonEvent(display,1,down,0):
                    raise ValueError("parent pointer button injection failed")
                held_button=down
                xlib.XSync(display,False)
                kind=QEvent.MouseButtonPress if down else QEvent.MouseButtonRelease
                wait(app,lambda:any(record["kind"] == kind for record in records[start:]),
                     "compositor pointer button delivery")
                if not next(record for record in records[start:] if record["kind"] == kind)["spontaneous"]:
                    raise ValueError("parent button input bypassed native Wayland delivery")

            try:
                xlib.XSetInputFocus(display,args.parent_x11_window,2,0)
                xlib.XSync(display,False)
                # Calibrate from observed native client coordinates. Wayland
                # does not promise applications a global surface position.
                origin=None
                for parent_y in range(100,601,100):
                    for parent_x in range(100,801,100):
                        start=len(records)
                        parent_move(parent_x,parent_y)
                        deadline=time.monotonic()+0.1
                        while time.monotonic()<deadline:
                            app.processEvents()
                        fresh=[record for record in records[start:] if record["kind"] == QEvent.MouseMove
                               and record["spontaneous"]]
                        if fresh:
                            origin=(parent_x-fresh[-1]["x"],parent_y-fresh[-1]["y"])
                            break
                    if origin is not None:
                        break
                if origin is None:
                    raise ValueError("no native pointer coordinates observed in the nested output")
                native_move(70,50)
                native_button(True)
                assert_result(3,True)
                native_move(390,280)
                assert_result(3,False)
                native_move(70,50)
                assert_result(3,True)
                native_button(False)
                assert_result(4)
                native_move(71,50)
                native_button(True)
                dispatch({"kind":"invoke"})
                assert_result(5)
                native_button(False)
                assert_result(5)
                native_move(72,50)
                native_button(True)
                pointer.ungrabMouse()
                app.processEvents()
                assert_result(5)
                native_button(False)
                assert_result(5)
                native_move(71,50)
                native_button(True)
                native_move(-20,-20)
                assert_result(5,False)
                native_button(False)
                assert_result(5)
                print("Parent XTest input traversed Weston/wl_pointer into spontaneous Qt delivery; exclusive capture retained motion/release outside the client. This is injected input, not physical-device qualification.")
            finally:
                if held_button:
                    xtest.XTestFakeButtonEvent(display,1,False,0)
                    xlib.XSync(display,False)
                xlib.XCloseDisplay(display)
        final_intents=window.property("intents")
        mouse(70,50,QEvent.MouseButtonPress)
        retired=[]
        def observe_retirement():
            retired.append(state())
        controller.publicationChanged.connect(observe_retirement)
        pointer.deleteLater()
        QCoreApplication.sendPostedEvents(None,QEvent.DeferredDelete)
        app.processEvents()
        if not retired or retired[-1]["hold"] is not None or last_event[0].exclusiveGrabber(last_event[0].point(0)):
            raise ValueError("actual pointer-owner destruction left an armed gesture or native grab")
        mouse(70,50,QEvent.MouseButtonRelease)
        if window.property("intents") != final_intents:
            raise ValueError("destroyed pointer owner activated on stale release")
        controller.publicationChanged.disconnect(observe_retirement)
        window.close()
        window.deleteLater()
        app.processEvents()
    except Exception as error:
        print("FAIL Qt command pointer runtime: " + str(error),file=sys.stderr)
        if "controller" in locals():
            from shiboken6 import isValid
            if isValid(controller):
                print(controller.property("publication"),file=sys.stderr)
            if pointer is not None and isValid(pointer):
                print("pointer error=" + str(pointer.property("lastError")),file=sys.stderr)
        return 1
    print("Injected Qt pointers matched exact Rust target membership, live activation, actual exclusive grabs, capture-loss cancellation and checked drawing on " + app.platformName())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
