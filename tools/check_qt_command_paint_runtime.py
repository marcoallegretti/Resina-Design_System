import argparse
import copy
import io
import json
import os
from pathlib import Path
import re
import sys
import time
from urllib.parse import unquote_to_bytes

from PIL import Image
from check_qt_command_snapshot_runtime import request_for, verify_snapshot
from check_quickshell_paint_runtime import compare_images, read_backend_output
import conformance_font


def main():
    parser = argparse.ArgumentParser(description="Verify checked Rust command paint in an actual Qt window.")
    parser.add_argument("--module-path", required=True, type=Path)
    parser.add_argument("--font-dir", required=True, type=Path)
    parser.add_argument("--raster", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args()
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    os.environ.setdefault("QT_QUICK_BACKEND", "software")
    try:
        from PySide6.QtCore import QPluginLoader, QUrl, qVersion
        from PySide6.QtGui import QGuiApplication
        from PySide6.QtQml import QQmlComponent, QQmlEngine
        from PySide6.QtQuick import QQuickWindow
        if qVersion() != "6.11.2":
            raise ValueError("requires Qt 6.11.2")
        app = QGuiApplication([])
        plugin = QPluginLoader(str(args.module_path.resolve() / "Resina/Runtime/libResina_Runtime.so"))
        if plugin.instance() is None:
            raise ValueError("native Qt plugin factory failed: " + plugin.errorString())
        conformance_font.register_qt(args.font_dir)
        engine = QQmlEngine()
        engine.addImportPath(str(args.module_path.resolve()))
        component = QQmlComponent(engine)
        qml_dir = (Path(__file__).resolve().parents[1] / "backends/quickshell/resina-qml/qml").as_uri()
        component.setData(('''import QtQuick
import Resina.Runtime 0.1
import "''' + qml_dir + '''" as Content
Window {
    width: 400; height: 220; visible: true; color: "transparent"
    property var prepared: null
    property var body: null
    property int renderer: 0
    CommandSnapshotResolver { id: resolver }
    Content.ResinaLabelMeasure { id: producer }
    function prepare(source, options) {
        if (body) { body.visible = false; body.destroy(); body = null; }
        const result = JSON.parse(resolver.resolvePaintedSnapshot(source, producer, options));
        if (!result.error) {
            body = Qt.createQmlObject(result.paintSource, contentItem, "CheckedCommandPaint");
            body.x = body.paintOriginX; body.y = body.paintOriginY;
        }
        prepared = result;
        return JSON.stringify(result);
    }
    Item { Component.onCompleted: renderer = GraphicsInfo.api }
}
''').encode(), QUrl())
        window = component.create()
        if not isinstance(window, QQuickWindow):
            raise ValueError("native paint fixture failed: " + repr(component.errors()))
        args.output_dir.mkdir(parents=True, exist_ok=False)
        count = 0
        dpr = window.devicePixelRatio()
        for theme in ("light", "dark"):
            for states in (["rest"], ["hover"], ["pressed"], ["disabled"], ["rest", "focused"]):
                request = request_for(theme, "Erneut mit dem Netzwerk verbinden")
                request["activation"].update(enabled="disabled" not in states, focused="focused" in states)
                request["hovered"] = "hover" in states
                if "pressed" in states:
                    request["activation"]["hold"] = {"kind": "pointer", "id": "primary", "inside": True}
                options = {"x": 44.3, "y": 26.1, "deviceScale": dpr, "samplesPerAxis": 4}
                result = json.loads(window.prepare(json.dumps(request), json.dumps(options)))
                if set(result) != {"snapshot", "paintSource"}:
                    raise ValueError("checked native rendering failed: " + str(result))
                verify_snapshot(result["snapshot"], request, {"schemaVersion": "0.1.0", "states": states})
                body = window.property("body")
                deadline = time.monotonic() + 5
                while not body.property("paintReady") and time.monotonic() < deadline:
                    app.processEvents()
                if not body.property("paintReady"):
                    raise ValueError("checked native paint never became ready")
                # Process actual presentation before synchronous window readback.
                deadline = time.monotonic() + .1
                while time.monotonic() < deadline:
                    app.processEvents()
                if window.property("renderer") != 1:
                    raise ValueError("software renderer was not actually selected")
                captured = window.grabWindow()
                path = args.output_dir / f"command-{theme}-{count}.png"
                if captured.isNull() or not captured.save(str(path)):
                    raise ValueError("actual native window readback failed")
                encoded = re.search(r'source: "data:image/png,([^"\n]+)"', result["paintSource"])
                if encoded is None:
                    raise ValueError("native paint source is not the prepared PNG")
                body_request = copy.deepcopy(request["body"])
                command_appearance = body_request.pop("commandAppearance")
                surrounding = body_request.pop("surroundingColor")
                body_request.update(schemaVersion="0.1.0", size=result["snapshot"]["label"]["size"],
                                    theme={"schemaVersion": "0.1.0", **{key: request["content"][key]
                                           for key in ("themeSource", "externalSources", "environment")}})
                body_request["surface"]["states"] = {"schemaVersion": "0.1.0", "states": states}
                portable = {"schemaVersion": "0.1.0", "surface": {"schemaVersion": "0.1.0",
                            "body": body_request, "surroundingColor": surrounding},
                            "commandAppearance": command_appearance}
                with Image.open(io.BytesIO(unquote_to_bytes(encoded.group(1)))) as source:
                    oracle = read_backend_output([str(args.raster.resolve()), "-", str(source.width), str(source.height),
                        str(body.property("paintOriginX") - options["x"]),
                        str(body.property("paintOriginY") - options["y"]), str(dpr), "4"], json.dumps(portable).encode())
                    with Image.open(io.BytesIO(oracle)) as independent:
                        compare_images(independent, source)
                with Image.open(io.BytesIO(unquote_to_bytes(encoded.group(1)))) as source:
                    expected = Image.new("RGBA", (round(400 * dpr), round(220 * dpr)))
                    expected.paste(source.convert("RGBA"), (round(body.property("paintOriginX") * dpr), round(body.property("paintOriginY") * dpr)))
                with Image.open(path) as actual:
                    compare_images(expected, actual)
                count += 1
        baseline = request_for("light", "Reconnect")
        for options in ({"x": 0, "y": 0, "deviceScale": 0, "samplesPerAxis": 4},
                        {"x": 0, "y": 0, "deviceScale": dpr, "samplesPerAxis": 0},
                        [], {"x": 0, "y": 0, "deviceScale": dpr, "samplesPerAxis": 4, "unknown": True}):
            if set(json.loads(window.prepare(json.dumps(baseline), json.dumps(options)))) != {"error"}:
                raise ValueError("invalid rendering options published a partial snapshot")
        for invalid in ('{"x":0,"x":1,"y":0,"deviceScale":1,"samplesPerAxis":4}',
                        " " * (64 * 1024 + 1),
                        '{"x":1e100,"y":0,"deviceScale":1,"samplesPerAxis":4}'):
            if set(json.loads(window.prepare(json.dumps(baseline), invalid))) != {"error"}:
                raise ValueError("invalid rendering transport/range published channels")
        recovered = json.loads(window.prepare(json.dumps(baseline), json.dumps(
            {"x": 44.3, "y": 26.1, "deviceScale": dpr, "samplesPerAxis": 4})))
        if set(recovered) != {"snapshot", "paintSource"}:
            raise ValueError("native rendering failed to recover after rejection")
        window.close()
        window.deleteLater()
        app.processEvents()
    except Exception as error:
        print("FAIL Qt command paint runtime: " + str(error), file=sys.stderr)
        return 1
    print(f"Checked Rust command paint matched actual Qt software readback for {count} Light/Dark state frames at DPR {dpr}; invalid rendering failed without partial channels")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
