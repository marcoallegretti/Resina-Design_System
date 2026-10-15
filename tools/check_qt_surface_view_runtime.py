"""Verify live Resina surface paint publication in native Qt against the reference resolver and CPU raster."""
import argparse
import copy
import io
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import threading
import time

from PIL import Image
from check_quickshell_paint_runtime import compare_images

ROOT = Path(__file__).resolve().parent.parent
PLACEMENT = (44.3, 26.1)
WINDOW = (400, 300)


def run(command, source):
    result = subprocess.run(command, input=source, capture_output=True, timeout=30)
    if result.returncode or result.stderr:
        raise ValueError(f"{command[0]} failed: {result.stderr.decode(errors='replace')[-2000:]}")
    return result.stdout


def paint_bounds(ir):
    if "focus" in ir:
        outer = ir["focus"]["geometry"]["outer"]
        bounds = dict(outer["contour"]["bounds"])
        bounds["x"] += outer["offset"]["x"]
        bounds["y"] += outer["offset"]["y"]
        return bounds
    return ir["body"]["geometry"]["silhouette"]["bounds"]


def expected_image(raster, source, ir, placement, scale):
    bounds = paint_bounds(ir)
    left = math.floor((bounds["x"] + placement[0]) * scale)
    top = math.floor((bounds["y"] + placement[1]) * scale)
    width = math.ceil((bounds["x"] + bounds["width"] + placement[0]) * scale) - left
    height = math.ceil((bounds["y"] + bounds["height"] + placement[1]) * scale) - top
    prepared = run([raster, "-", str(width), str(height), str(left / scale - placement[0]),
                    str(top / scale - placement[1]), str(scale), "4"], source)
    expected = Image.new("RGBA", (round(WINDOW[0] * scale), round(WINDOW[1] * scale)))
    with Image.open(io.BytesIO(prepared)) as image:
        expected.paste(image, (left, top))
    return expected


def expected_content(ir):
    content = ir["body"]["geometry"]["content"]
    bounds = content["contour"]["bounds"]
    return [content["offset"]["x"] + bounds["x"], content["offset"]["y"] + bounds["y"], bounds["width"], bounds["height"]]


FIXTURE = '''import QtQuick
import Resina.Runtime 0.1
import "@@QML@@" as Content
Window {
    width: @@WIDTH@@; height: @@HEIGHT@@; visible: true; color: "transparent"
    property var readiness: []
    property bool incoherent: false
    property string captured: ""
    property int renderer: 0
    Item {
        id: capture; width: @@WIDTH@@; height: @@HEIGHT@@
        Item {
            id: host
            transform: Scale { id: distortion; xScale: 1; yScale: 1 }
            Content.ResinaSurfaceView { id: view; objectName: "surfaceView"; x: @@X@@; y: @@Y@@; samplesPerAxis: 4 }
        }
    }
    Connections {
        target: view
        function onPaintReadyChanged() { readiness.push(view.paintReady); }
        function onPublicationChanged() { if (view.publication.length && (!view.paintReady || !view.visible)) incoherent = true; }
    }
    SurfacePaintResolver { id: resolver; objectName: "surfaceResolver" }
    Item { Component.onCompleted: renderer = GraphicsInfo.api }
    function setRequest(source) { readiness = []; view.request = source; }
    function observations() {
        return JSON.stringify({ready:view.paintReady, publication:view.publication, error:view.paintError,
            visible:view.visible, transitions:readiness, foreground:[view.foreground.r, view.foreground.g,
            view.foreground.b, view.foreground.a], content:[view.contentBounds.x, view.contentBounds.y,
            view.contentBounds.width, view.contentBounds.height], size:[view.implicitWidth, view.implicitHeight]});
    }
    function grab(path) {
        captured = "";
        if (!capture.grabToImage(function(result) { captured = result.saveToFile(path) ? "saved" : "failed"; }))
            captured = "failed";
    }
    function move(value) { view.x = value; }
    function moveParent(value) { host.x = value; }
    function distort(value) { distortion.xScale = value; }
    function makeTemporary(source) {
        const component = Qt.createComponent(@@VIEW@@);
        if (component.status !== Component.Ready) throw new Error(component.errorString());
        return component.createObject(host, {x:@@X@@, y:@@Y@@, samplesPerAxis:4, request:source});
    }
    function startPreparation(item) {
        item.prepare();
        return JSON.stringify({published:item.publication.length > 0, ready:item.paintReady, error:item.paintError});
    }
    function contextProbes() {
        const scale = view.nativeDeviceScale;
        return JSON.stringify([
            resolver.matchesNativeContext(view, @@X@@, @@Y@@, scale),
            resolver.matchesNativeContext(view, @@X@@ + 1/2048, @@Y@@, scale),
            resolver.matchesNativeContext(view, @@X@@ + 1/512, @@Y@@, scale),
            resolver.matchesNativeContext(view, @@X@@, @@Y@@, scale + 1),
            resolver.matchesNativeContext(null, @@X@@, @@Y@@, scale),
            resolver.matchesNativeContext(resolver, @@X@@, @@Y@@, scale),
            resolver.matchesNativeContext(view, NaN, @@Y@@, scale),
            resolver.matchesNativeContext(view, @@X@@, Infinity, scale),
            resolver.matchesNativeContext(view, @@X@@, @@Y@@, 0)]);
    }
}
'''


class Fixture:
    def __init__(self, app, window, output):
        self.app, self.window, self.output, self.count = app, window, output, 0

    def observe(self):
        return json.loads(self.window.observations())

    def settle(self, predicate, description, timeout=8):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self.app.processEvents()
            observation = self.observe()
            if predicate(observation):
                return observation
        observation = self.observe()
        observation["publication"] = bool(observation["publication"])
        raise ValueError(f"{description} was not reached: {observation}")

    def present(self):
        deadline = time.monotonic() + .1
        while time.monotonic() < deadline:
            self.app.processEvents()

    def capture(self):
        self.present()
        path = self.output / f"surface-{self.count}.png"
        self.count += 1
        self.window.grab(str(path))
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline and not self.window.property("captured"):
            self.app.processEvents()
        if self.window.property("captured") != "saved":
            raise ValueError("native surface capture failed")
        return Image.open(path)

    def withdrawn(self, observation):
        return not observation["ready"] and not observation["visible"] and not observation["publication"]


def verify_publication(fixture, observation, ir, scale, name):
    if json.loads(observation["publication"]) != ir:
        raise ValueError(f"{name}: native publication differs from the reference surface paint IR")
    color = ir["body"]["foreground"]
    if any(abs(actual - expected) > 1 / 512 for actual, expected
           in zip(observation["foreground"], color["components"] + [color["alpha"]])):
        raise ValueError(f"{name}: foreground {observation['foreground']} differs from the guarded IR foreground")
    if any(abs(actual - expected) > 1e-6 for actual, expected in zip(observation["content"], expected_content(ir))):
        raise ValueError(f"{name}: content bounds {observation['content']} differ from the IR content region")
    front = ir["body"]["geometry"]["front"]["bounds"]
    if observation["size"] != [front["width"], front["height"]]:
        raise ValueError(f"{name}: implicit size differs from the IR front")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--module-path", required=True, type=Path)
    parser.add_argument("--raster", required=True)
    parser.add_argument("--resolver", required=True)
    parser.add_argument("--scenes", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args()
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    os.environ.setdefault("QT_QUICK_BACKEND", "software")
    try:
        from PySide6.QtCore import (QCoreApplication, QEvent, QObject, QPluginLoader, QUrl, QtMsgType,
                                    qInstallMessageHandler, qVersion)
        from PySide6.QtGui import QGuiApplication
        from PySide6.QtQml import QQmlComponent, QQmlEngine
        from PySide6.QtQuick import QQuickWindow
        if qVersion() != "6.11.2":
            raise ValueError("requires Qt 6.11.2")
        catalog = json.loads(args.scenes.read_text(encoding="utf-8"))
        scenes = [scene for scene in catalog["scenarios"] if scene["kind"] == "surfacePaint"]
        if len(scenes) != 16 or len({scene["name"] for scene in scenes}) != 16:
            raise ValueError("expected the 16 distinct public complete material paint scenes")
        warnings = []
        qInstallMessageHandler(lambda kind, context, message: warnings.append(message)
                               if kind != QtMsgType.QtDebugMsg and kind != QtMsgType.QtInfoMsg else None)
        app = QGuiApplication([])
        plugin = QPluginLoader(str(args.module_path.resolve() / "Resina/Runtime/libResina_Runtime.so"))
        if plugin.instance() is None:
            raise ValueError("native Qt plugin factory failed: " + plugin.errorString())
        engine = QQmlEngine()
        engine.addImportPath(str(args.module_path.resolve()))
        component = QQmlComponent(engine)
        source = FIXTURE
        for key, value in (("@@QML@@", (ROOT / "backends/quickshell/resina-qml/qml").as_uri()),
                           ("@@VIEW@@", json.dumps((ROOT / "backends/quickshell/resina-qml/qml/ResinaSurfaceView.qml").as_uri())),
                           ("@@WIDTH@@", str(WINDOW[0])), ("@@HEIGHT@@", str(WINDOW[1])),
                           ("@@X@@", str(PLACEMENT[0])), ("@@Y@@", str(PLACEMENT[1]))):
            source = source.replace(key, value)
        component.setData(source.encode(), QUrl())
        window = component.create()
        if not isinstance(window, QQuickWindow):
            raise ValueError("native surface fixture failed: " + repr(component.errors()))
        args.output_dir.mkdir(parents=True, exist_ok=False)
        fixture = Fixture(app, window, args.output_dir)
        scale = window.devicePixelRatio()
        fixture.present()
        if window.property("renderer") != 1:
            raise ValueError("requires actual Qt software rendering")

        if json.loads(window.contextProbes()) != [True, True, False, False, False, False, False, False, False]:
            raise ValueError("native context guard lost placement precision, type, density or finite-input checks")
        native_resolver = window.findChild(QObject, "surfaceResolver")
        native_view = window.findChild(QObject, "surfaceView")
        results = []
        worker = threading.Thread(target=lambda: results.append(
            native_resolver.matchesNativeContext(native_view, *PLACEMENT, scale)), daemon=True)
        worker.start()
        worker.join(5)
        if worker.is_alive() or results != [False]:
            raise ValueError("a foreign calling thread inspected or accepted a native surface context")

        for index, scene in enumerate(scenes):
            request = json.dumps(scene["request"])
            ir = json.loads(run([args.resolver, "-"], request.encode()))
            window.setRequest(request)
            observation = fixture.settle(lambda current: current["ready"] or current["error"], scene["name"])
            if observation["error"]:
                raise ValueError(f"{scene['name']}: {observation['error']}")
            if index and observation["transitions"] != [False, True]:
                raise ValueError(f"{scene['name']}: replacement did not withdraw readiness before publication")
            verify_publication(fixture, observation, ir, scale, scene["name"])
            with fixture.capture() as actual:
                compare_images(expected_image(args.raster, request.encode(), ir, PLACEMENT, scale), actual)
            print(f"PASS {scene['name']}", flush=True)
        if window.property("incoherent"):
            raise ValueError("publication notification preceded complete native readiness")

        base = copy.deepcopy(scenes[0]["request"])
        valid = json.dumps(base)
        focused = copy.deepcopy(base)
        focused["body"]["surface"]["states"]["states"] = ["rest", "focused"]
        focused.pop("surroundingColor", None)
        for invalid in ("[]", "{}", json.dumps(focused)):
            window.setRequest(invalid)
            observation = fixture.settle(lambda current: current["error"], "rejection of an invalid request")
            if not fixture.withdrawn(observation):
                raise ValueError("an invalid request retained native publication")
            window.setRequest(valid)
            fixture.settle(lambda current: current["ready"], "recovery after rejection")
        window.setRequest("")
        observation = fixture.settle(lambda current: not current["ready"], "withdrawal of a cleared request")
        if not fixture.withdrawn(observation) or observation["error"]:
            raise ValueError("a cleared request retained publication or reported an error")
        with fixture.capture() as blank:
            if blank.convert("RGBA").getbbox() is not None:
                raise ValueError("a cleared surface remains visible")

        ir = json.loads(run([args.resolver, "-"], valid.encode()))
        window.setRequest(valid)
        fixture.settle(lambda current: current["ready"], "publication before the placement change")
        window.move(PLACEMENT[0] + 1.7)
        fixture.settle(lambda current: current["ready"] and current["transitions"][-2:] == [False, True],
                       "re-preparation at the moved placement")
        with fixture.capture() as actual:
            compare_images(expected_image(args.raster, valid.encode(), ir, (PLACEMENT[0] + 1.7, PLACEMENT[1]), scale),
                           actual)
        window.move(PLACEMENT[0])
        fixture.settle(lambda current: current["ready"], "re-preparation at the original placement")

        window.setRequest(valid)
        fixture.settle(lambda current: current["ready"], "publication before the ancestor change")
        window.moveParent(2.5)
        fixture.settle(lambda current: current["ready"] and current["transitions"][-2:] == [False, True],
                       "re-preparation after an ancestor translation")
        with fixture.capture() as actual:
            compare_images(expected_image(args.raster, valid.encode(), ir, (PLACEMENT[0] + 2.5, PLACEMENT[1]), scale),
                           actual)
        window.moveParent(0)
        fixture.settle(lambda current: current["ready"] and current["transitions"][-2:] == [False, True],
                       "re-preparation after restoring the ancestor")

        window.distort(1.0005)
        observation = fixture.settle(lambda current: current["error"] and not current["ready"],
                                     "rejection of a non-translation ancestor")
        if not fixture.withdrawn(observation) or "translation-only" not in observation["error"]:
            raise ValueError("a non-translation ancestor did not withdraw publication diagnostically")
        if observation["foreground"][3] != 0 or observation["content"] != [0, 0, 0, 0]:
            raise ValueError("withdrawn surface retained its guarded foreground or content region")
        with fixture.capture() as blank:
            if blank.convert("RGBA").getbbox() is not None:
                raise ValueError("withdrawn surface paint remains visible")
        window.distort(1)
        fixture.settle(lambda current: current["ready"] and not current["error"], "recovery of the native context")
        with fixture.capture() as actual:
            compare_images(expected_image(args.raster, valid.encode(), ir, PLACEMENT, scale), actual)

        initial = window.makeTemporary(valid)
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline and not initial.property("paintReady"):
            app.processEvents()
        if not initial.property("paintReady") or json.loads(initial.property("publication")) != ir:
            raise ValueError("a request supplied at creation was not published")
        initial.deleteLater()
        QCoreApplication.sendPostedEvents(None, QEvent.DeferredDelete)

        destroyed = []
        queued = window.makeTemporary(valid)
        queued.destroyed.connect(lambda: destroyed.append("queued"))
        queued.deleteLater()
        QCoreApplication.sendPostedEvents(None, QEvent.DeferredDelete)
        started = window.makeTemporary("")
        started.destroyed.connect(lambda: destroyed.append("started"))
        started.setProperty("request", valid)
        pending = json.loads(window.startPreparation(started))
        if pending["published"] or pending["ready"] or pending["error"]:
            raise ValueError(f"preparation was not pending before destruction: {pending}")
        started.deleteLater()
        QCoreApplication.sendPostedEvents(None, QEvent.DeferredDelete)
        deadline = time.monotonic() + 6
        while time.monotonic() < deadline:
            app.processEvents()
        if sorted(destroyed) != ["queued", "started"]:
            raise ValueError("pending surface views were not destroyed")
        window.close()
        window.deleteLater()
        app.processEvents()
        if warnings:
            raise ValueError("Qt reported warnings: " + " | ".join(warnings[:5]))
    except Exception as error:
        print("FAIL Qt surface view runtime: " + str(error), file=sys.stderr)
        return 1
    print(f"Live surface paint matched reference IR and CPU raster for 16 scenes on {app.platformName()} at DPR {scale}; "
          "rejection, clearing, placement, ancestor and context recovery passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
