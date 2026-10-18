import argparse
import json
import os
from pathlib import Path
import sys
import threading
import time

from PIL import Image
from check_qt_command_snapshot_runtime import ROOT, request_for, verify_snapshot
from check_quickshell_paint_runtime import compare_images


def present(application):
    deadline = time.monotonic() + .1
    while time.monotonic() < deadline:
        application.processEvents()


def prepare(window, application, source, options):
    result = json.loads(window.prepare(source, options))
    if "error" in result:
        return result
    deadline = time.monotonic() + 6
    while time.monotonic() < deadline:
        application.processEvents()
        observation = json.loads(window.observations())
        if observation["ready"]:
            return json.loads(observation["snapshot"])
        if observation["error"]:
            return {"error":observation["error"]}
    raise ValueError("native view did not resolve preparation")


def main():
    parser = argparse.ArgumentParser(description="Verify coherent command body/glyph publication in native Qt.")
    parser.add_argument("--module-path", required=True, type=Path)
    parser.add_argument("--font", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args()
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    os.environ.setdefault("QT_QUICK_BACKEND", "software")
    try:
        from PySide6.QtCore import QCoreApplication, QEvent, QObject, QPluginLoader, QUrl, qVersion
        from PySide6.QtGui import QFontDatabase, QGuiApplication
        from PySide6.QtQml import QQmlComponent, QQmlEngine
        from PySide6.QtQuick import QQuickWindow
        if qVersion() != "6.11.2":
            raise ValueError("requires Qt 6.11.2")
        app = QGuiApplication([])
        plugin = QPluginLoader(str(args.module_path.resolve() / "Resina/Runtime/libResina_Runtime.so"))
        if plugin.instance() is None:
            raise ValueError("native Qt plugin factory failed: " + plugin.errorString())
        if QFontDatabase.applicationFontFamilies(QFontDatabase.addApplicationFont(str(args.font.resolve()))) != ["DejaVu Sans"]:
            raise ValueError("requires explicitly loaded DejaVu Sans")
        engine = QQmlEngine()
        engine.addImportPath(str(args.module_path.resolve()))
        component = QQmlComponent(engine)
        component.setData(('''import QtQuick
import Resina.Runtime 0.1
import "''' + (ROOT / "backends/quickshell/resina-qml/qml").as_uri() + '''" as Content
Window {
    width: 400; height: 300; visible: true; color: "white"
    property var readiness: []
    property bool oracle: false
    property bool recursive: false
    property string recursiveError: ""
    property bool recursiveWithdrawal: false
    property string withdrawalError: ""
    property string lastSource: ""
    property string lastOptions: ""
    property bool incoherentNotification: false
    property int renderer: 0
    Item {
        id: host
        transform: Scale { id: distortion; xScale: 1; yScale: 1 }
        Content.ResinaCommandView {
            id: view; objectName: "commandView"; x:44.3; y:26.1; opacity: oracle ? 0 : 1
            transform: Matrix4x4 { id: projection; matrix: Qt.matrix4x4() }
        }
    }
    Connections {
        target: view
        function onPaintReadyChanged() { readiness.push(view.paintReady); }
        function onSnapshotChanged() {
            if (view.snapshot.length && (!view.paintReady || !view.visible))
                incoherentNotification = true;
            if (recursiveWithdrawal && !view.snapshot.length) {
                try { view.prepare(lastSource, lastOptions); }
                catch(error) { withdrawalError = error.message; }
            }
            if (recursive && view.snapshot.length) {
                try { view.prepare("[]", "[]"); }
                catch(error) { recursiveError = error.message; }
            }
        }
    }
    Item {
        id: reference; x:44.3; y:26.1; visible: oracle
        property var body: null
        Content.ResinaLabel { id: glyphs }
    }
    CommandSnapshotResolver { id: resolver; objectName:"snapshotResolver" }
    Content.ResinaLabelMeasure { id: measurement }
    Item { Component.onCompleted: renderer = GraphicsInfo.api }
    function prepare(source, options) {
        lastSource = source; lastOptions = options;
        readiness = [];
        try { view.prepare(source, options); return JSON.stringify({pending:true}); }
        catch(error) { return JSON.stringify({error:error.message}); }
    }
    function prepareReference(source, options) {
        if (reference.body) { reference.body.visible=false; reference.body.destroy(); }
        const result = JSON.parse(resolver.resolvePaintedSnapshot(source, measurement, options));
        if (result.error) throw new Error(result.error);
        const placement = JSON.parse(options);
        reference.body = Qt.createQmlObject(result.paintSource, reference, "IndependentCommandPaint");
        reference.body.z = -1;
        reference.body.x = reference.body.paintOriginX-placement.x;
        reference.body.y = reference.body.paintOriginY-placement.y;
        const label = result.snapshot.label;
        const color = result.snapshot.paint.paint.body.foreground;
        glyphs.prepare({text:label.text, family:"DejaVu Sans", pixelSize:label.typography.fontSize.value,
            weight:label.typography.fontWeight, spacing:label.typography.letterSpacing.value,
            lineHeight:label.typography.lineHeight, wrappingWidth:label.labelBounds.width,
            height:label.labelBounds.height, x:label.labelBounds.x,y:label.labelBounds.y,
            color:color.components.concat([color.alpha])});
    }
    function observations() { return JSON.stringify({ready:view.paintReady, snapshot:view.snapshot,
        error:view.paintError, transitions:readiness, visible:view.visible}); }
    function probeImageError(source, options) {
        if (reference.body) { reference.body.visible=false; reference.body.destroy(); }
        const result = JSON.parse(resolver.resolvePaintedSnapshot(source, measurement, options));
        const marker = 'source: "data:image/png,';
        const begin = result.paintSource.indexOf(marker);
        const end = result.paintSource.indexOf('"', begin+marker.length);
        const corrupt = result.paintSource.substring(0, begin) + 'source: "data:image/png,invalid"'
            + result.paintSource.substring(end+1);
        reference.body = Qt.createQmlObject(corrupt, reference, "NativePaintDecodeFailure");
    }
    function imageErrorObservation() {
        return JSON.stringify({ready:reference.body.paintReady, error:reference.body.paintError});
    }
    function makeTemporary(source, options) {
        const component = Qt.createComponent(@@VIEW_SOURCE@@);
        if (component.status !== Component.Ready) throw new Error(component.errorString());
        const item = component.createObject(host, {x:44.3, y:26.1});
        item.prepare(source, options);
        return item;
    }
    function move(value) { view.x = value; }
    function moveParent(value) { host.x = value; }
    function distort(value) { distortion.xScale = value; }
    function projective(value) {
        projection.matrix = value ? Qt.matrix4x4(2,0,0,0, 0,2,0,0, 0,0,1,0, 1,1,0,1) : Qt.matrix4x4();
    }
    function contextProbes() {
        const scale=view.nativeDeviceScale;
        return JSON.stringify([
            scale === view.Window.window.devicePixelRatio,
            resolver.presentsNativeWindow(view),
            resolver.presentsNativeWindow(resolver),
            resolver.matchesNativeContext(view,44.3,26.1,scale),
            resolver.matchesNativeContext(view,44.3+1/2048,26.1,scale),
            resolver.matchesNativeContext(view,44.3+1/512,26.1,scale),
            resolver.matchesNativeContext(view,44.3,26.1,scale+1),
            resolver.matchesNativeContext(null,44.3,26.1,scale),
            resolver.matchesNativeContext(resolver,44.3,26.1,scale),
            resolver.matchesNativeContext(view,NaN,26.1,scale),
            resolver.matchesNativeContext(view,44.3,Infinity,scale),
            resolver.matchesNativeContext(view,44.3,26.1,0)]);
    }
    function mappedBasis() { return JSON.stringify([view.mapToItem(null,0,0),view.mapToItem(null,1,0),
        view.mapToItem(null,0,1),view.mapToItem(null,50,50)]); }
}
''').replace("@@VIEW_SOURCE@@", json.dumps((ROOT / "backends/quickshell/resina-qml/qml/ResinaCommandView.qml").as_uri())).encode(), QUrl())
        window = component.create()
        if not isinstance(window, QQuickWindow):
            raise ValueError("native view fixture failed: " + repr(component.errors()))
        args.output_dir.mkdir(parents=True, exist_ok=False)
        present(app)
        if not window.isExposed():
            raise ValueError("native view fixture window was not exposed")
        dpr = window.devicePixelRatio()
        options = json.dumps({"x":44.3, "y":26.1, "deviceScale":dpr, "samplesPerAxis":4})
        window.projective(True)
        origin,horizontal,vertical,distant=json.loads(window.mappedBasis())
        if ((horizontal["x"]-origin["x"],horizontal["y"]-origin["y"],
             vertical["x"]-origin["x"],vertical["y"]-origin["y"]) != (1,0,0,1)
            or (distant["x"]-origin["x"],distant["y"]-origin["y"]) == (50,50)):
            raise ValueError("projective fixture does not disguise its nontranslation as a unit basis")
        rejected=prepare(window,app,json.dumps(request_for("light","Reconnect")),options)
        if "matching native translation" not in rejected.get("error", ""):
            raise ValueError("projective native view did not reject its deceptively valid unit basis diagnostically")
        observation=json.loads(window.observations())
        if observation["ready"] or observation["snapshot"] or observation["visible"] or not observation["error"]:
            raise ValueError("projective rejection retained native publication")
        window.projective(False)
        if json.loads(window.contextProbes()) != [True,True,False,True,True,False,False,False,False,False,False,False]:
            raise ValueError("native view density is not its window's, or the native context guard lost exposure, "
                             "placement precision, type, density or finite-input checks")
        native_resolver=window.findChild(QObject,"snapshotResolver")
        native_view=window.findChild(QObject,"commandView")
        if not native_resolver.matchesNativeContext(native_view,44.3,26.1,dpr):
            raise ValueError("valid UI-thread context unexpectedly rejected")
        thread_results=[]
        worker=threading.Thread(target=lambda:thread_results.append(
            native_resolver.matchesNativeContext(native_view,44.3,26.1,dpr)),daemon=True)
        worker.start()
        worker.join(5)
        if worker.is_alive() or thread_results != [False]:
            raise ValueError("foreign calling thread inspected or accepted a native command context")
        count = 0
        scenarios = []
        for theme in ("light", "dark"):
            for scale in (1.0006, 1.5, 2):
                for text in ("Reconnect", "Erneut mit dem Netzwerk verbinden", "مرحبا بالعالم\u2029! 123\u2029Reconnect", "Reconnect\u2029\u2029! 123"):
                    for direction in ("ltr", "rtl"):
                        request = request_for(theme, text)
                        request["content"]["environment"].update(layoutDirection=direction, textScale=scale)
                        request["content"]["padding"]["end"] = 40
                        scenarios.append((theme, request, ["rest"]))
            for states in (["hover"], ["pressed"], ["disabled"], ["rest", "focused"]):
                request = request_for(theme, "Erneut mit dem Netzwerk verbinden")
                request["hovered"] = "hover" in states
                request["activation"].update(enabled="disabled" not in states, focused="focused" in states)
                if "pressed" in states:
                    request["activation"]["hold"] = {"kind":"pointer", "id":"primary", "inside":True}
                scenarios.append((theme, request, states))
        for theme, request, states in scenarios:
            window.setProperty("color", "white" if theme == "light" else "black")
            source = json.dumps(request)
            snapshot = prepare(window, app, source, options)
            if "error" in snapshot:
                raise ValueError(snapshot["error"])
            verify_snapshot(snapshot, request, {"schemaVersion":"0.1.0", "states":states})
            observations = json.loads(window.observations())
            if not observations["ready"] or not observations["visible"] or observations["error"]:
                raise ValueError("native channels were not committed ready together")
            if count and observations["transitions"] != [False, True]:
                raise ValueError("replacement did not withdraw readiness before publication")
            present(app)
            if window.property("renderer") != 1:
                raise ValueError("requires actual Qt software rendering")
            path = args.output_dir / f"view-{count}.png"
            if not window.grabWindow().save(str(path)):
                raise ValueError("native view readback failed")
            window.prepareReference(source, options)
            window.setProperty("oracle", True)
            present(app)
            reference = args.output_dir / f"reference-{count}.png"
            if not window.grabWindow().save(str(reference)):
                raise ValueError("native reference readback failed")
            with Image.open(path) as actual, Image.open(reference) as expected:
                compare_images(expected, actual)
            window.setProperty("oracle", False)
            count += 1
        source = json.dumps(request_for("light", "Reconnect"))
        window.setProperty("recursive", True)
        if "error" in prepare(window, app, source, options) or not window.property("recursiveError"):
            raise ValueError("publication reentrancy did not reject the recursive preparation")
        window.setProperty("recursive", False)
        window.setProperty("recursiveWithdrawal", True)
        if "error" in prepare(window, app, source, options) or "in progress" not in window.property("withdrawalError"):
            raise ValueError("withdrawal permitted reentrant native preparation")
        window.setProperty("recursiveWithdrawal", False)
        if window.property("incoherentNotification"):
            raise ValueError("snapshot notification preceded complete native readiness")
        for invalid in ("[]", "{}"):
            if "error" not in prepare(window, app, invalid, options):
                raise ValueError("invalid request published a native view")
            observations = json.loads(window.observations())
            if observations["ready"] or observations["visible"] or observations["snapshot"] or not observations["error"]:
                raise ValueError("failed native view retained stale channels")
        if "error" in prepare(window, app, source, options):
            raise ValueError("native view failed to recover")
        window.move(45.3)
        if json.loads(window.observations())["ready"]:
            raise ValueError("native placement change retained prepared paint")
        if "error" not in prepare(window, app, source, options):
            raise ValueError("mismatched placement was accepted")
        window.move(44.3)
        if "error" in prepare(window, app, source, options):
            raise ValueError("native placement recovery failed")
        node = window.findChild(QObject, "commandView")
        if node is None or node.metaObject().property(node.metaObject().indexOfProperty("snapshot")).isWritable():
            raise ValueError("native snapshot publication is not read-only")
        for index, (change, restore) in enumerate(((lambda: window.moveParent(1), lambda: window.moveParent(0)),
                                (lambda: window.distort(1.0005), lambda: window.distort(1)),
                                (lambda: window.projective(True), lambda: window.projective(False)))):
            change()
            present(app)
            observation = json.loads(window.observations())
            if observation["ready"] or observation["snapshot"]:
                raise ValueError("changed ancestor context reached native rendering with stale publication")
            blank = args.output_dir / f"withdrawn-{index}.png"
            window.grabWindow().save(str(blank))
            with Image.open(blank) as actual:
                if len(actual.convert("RGBA").getcolors(actual.width * actual.height)) != 1:
                    raise ValueError("withdrawn native channels remain visible")
            restore()
            if "error" in prepare(window, app, source, options):
                raise ValueError("ancestor context recovery failed")
        # Cancel an actual pending decode before Qt processes its completion.
        window.prepare(source, options)
        window.move(45.3)
        present(app)
        if json.loads(window.observations())["ready"]:
            raise ValueError("cancelled pending preparation published after native context loss")
        window.move(44.3)
        if "error" in prepare(window, app, source, options):
            raise ValueError("pending cancellation recovery failed")
        temporary = window.makeTemporary(source, options)
        destroyed = []
        temporary.destroyed.connect(lambda: destroyed.append(True))
        temporary.deleteLater()
        QCoreApplication.sendPostedEvents(None, QEvent.DeferredDelete)
        present(app)
        if not destroyed:
            raise ValueError("pending native view was not actually destroyed")
        window.probeImageError(source, options)
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            app.processEvents()
            observed = json.loads(window.imageErrorObservation())
            if observed["error"]:
                break
        if observed["ready"] or not observed["error"]:
            raise ValueError("actual native paint decode failure was not exposed")
        window.close()
        window.deleteLater()
        app.processEvents()
    except Exception as error:
        print("FAIL Qt command view runtime: " + str(error), file=sys.stderr)
        return 1
    print(f"Coherent Qt body/glyph publication matched native reference captures on {app.platformName()} at DPR {dpr}; rejection, replacement, reentrancy and placement recovery passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
