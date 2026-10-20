import argparse
import copy
import json
import os
from pathlib import Path
import sys

from command_label_arithmetic import layout
import conformance_font


ROOT = Path(__file__).resolve().parents[1]


def resolved_master(masters, calls):
    """Identify the one registered master that resolved every native measurement call."""
    weights = {call["request"]["weight"] for call in calls}
    faces = {(call["face"]["family"], call["face"]["style"]) for call in calls}
    if len(weights) != 1 or len(faces) != 1:
        raise ValueError("native measurement calls resolved different weights or masters")
    (family, style), = faces
    return conformance_font.resolve(masters, family, style, weights.pop())


def verify_layout(label, calls, request):
    if len(calls) != 2 or calls[0]["request"]["maximumWidth"] is not None:
        raise ValueError("Rust layout did not request natural then fitted native measurement")
    natural, fitted = (call["result"] for call in calls)
    if any(result.get("kind") != "extent" for result in (natural, fitted)):
        raise ValueError("native measurement failed")
    expected = layout({**request, "natural": natural, "fitted": fitted,
                       "direction": label["layoutDirection"]})
    width, height = expected["size"]["width"], expected["size"]["height"]
    expected_bounds = expected["labelBounds"]
    offered = expected_bounds["width"]
    if label["text"] != request["text"] or label["size"] != {"width": width, "height": height} \
            or label["labelBounds"] != expected_bounds or calls[1]["request"]["maximumWidth"] != offered \
            or fitted["width"] > offered:
        raise ValueError("native extents did not reach the complete Rust layout equations")
    style = label["typography"]
    for call in calls:
        native = call["request"]
        if native["text"] != label["text"] or native["family"] != request["fontFamily"]["name"] \
                or native["pixelSize"] != style["fontSize"]["value"] \
                or native["weight"] != style["fontWeight"] or native["spacing"] != style["letterSpacing"]["value"] \
                or native["lineHeight"] != style["lineHeight"]:
            raise ValueError("resolved typography or text changed at the native producer boundary")


def main():
    parser = argparse.ArgumentParser(description="Verify Rust command layout through actual Qt text measurement and preparation.")
    parser.add_argument("--module-path", required=True, type=Path)
    parser.add_argument("--font-dir", required=True, type=Path)
    arguments = parser.parse_args()
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    try:
        from PySide6.QtCore import QPluginLoader, QUrl, qVersion
        from PySide6.QtGui import QGuiApplication
        from PySide6.QtQml import QQmlComponent, QQmlEngine
        from PySide6 import QtQuick  # Load the pinned host's native plugin dependency.
        if qVersion() != "6.11.2":
            raise ValueError("requires Qt 6.11.2")
        application = QGuiApplication([])
        loader = QPluginLoader(str(arguments.module_path.resolve() / "Resina/Runtime/libResina_Runtime.so"))
        if loader.instance() is None:
            raise ValueError("Qt plugin factory failed: " + loader.errorString())
        masters = conformance_font.register_qt(arguments.font_dir)
        family = conformance_font.FAMILY
        engine = QQmlEngine()
        engine.addImportPath(str(arguments.module_path.resolve()))
        component = QQmlComponent(engine)
        content_uri = (ROOT / "backends/quickshell/resina-qml/qml").as_uri()
        component.setData(('''import QtQuick
import Resina.Runtime 0.1
import "''' + content_uri + '''" as Content
Item {
    property var readiness: []
    property bool recursive: false
    property string recursiveError: ""
    CommandLabelResolver { id: resolver }
    Content.ResinaLabelMeasure { id: measurement }
    Content.ResinaCommandLabel { id: drawing }
    Connections {
        target: drawing
        function onPaintReadyChanged() { readiness.push(drawing.paintReady); }
        function onLayoutChanged() {
            if (recursive && drawing.layout.length) {
                try { drawing.prepare("[]", [0,0,0,1]); }
                catch (error) { recursiveError = error.message; }
            }
        }
    }
    QtObject {
        id: observer
        property var calls: []
        function measureJson(source) {
            const result = measurement.measureJson(source);
            const request = JSON.parse(source);
            const face = JSON.parse(result).kind === "extent" ? measurement.measure(request.text, request.family,
                request.pixelSize, request.weight, request.spacing, request.lineHeight, request.maximumWidth).face : null;
            calls.push({request:request, result:JSON.parse(result), face:face});
            return result;
        }
    }
    QtObject { id: malformed; function measureJson(source) { return '{"kind":"extent","width":1,"width":2,"height":1}'; } }
    function resolve(source) {
        observer.calls = [];
        return JSON.stringify({result:JSON.parse(resolver.resolveLabel(source, observer)), calls:observer.calls});
    }
    function rejectProducer(source, missing) { return resolver.resolveLabel(source, missing ? null : malformed); }
    function draw(source) {
        readiness = [];
        try { drawing.prepare(source, [0,0,0,1]); }
        catch (error) {}
        return JSON.stringify({layout:drawing.layout, ready:drawing.paintReady,
            error:drawing.paintError, visible:drawing.visible, ignored:drawing.Accessible.ignored,
            enabled:drawing.enabled, width:drawing.width, height:drawing.height, readiness:readiness});
    }
    function recursiveDraw(source) { recursive = true; const result = draw(source); recursive = false; return result; }
}
''').encode(), QUrl())
        root = component.create()
        if root is None:
            raise ValueError("native content fixture failed: " + repr(component.errors()))
        baseline_environment = json.loads((ROOT / "conformance/headless/valid-request.json").read_text())["environment"]
        count = 0
        resolved = set()
        for theme in ("light", "dark"):
            for scale in (1, 1.0006, 1.5, 2):
                for direction in ("ltr", "rtl"):
                    for text in ("Reconnect", "Erneut mit dem Netzwerk verbinden", "مرحبا بالعالم",
                                 "مرحبا بالعالم\u2029! 123\u2029Reconnect", "\u2029Reconnect\u2029\u2029"):
                        request = {"themeSource": (ROOT / f"tokens/themes/{theme}.json").read_text(),
                                   "externalSources": {"foundation.json": (ROOT / "tokens/foundation.json").read_text()},
                                   "environment": {**baseline_environment, "textScale": scale, "layoutDirection": direction},
                                   "text": text, "fontFamily": {"role": "sans", "name": family},
                                   "minimumSize": {"width": 64, "height": 80},
                                   "maximumSize": {"width": 180, "height": 600},
                                   "padding": {"start": 12, "end": 8, "top": 6, "bottom": 10}}
                        source = json.dumps(request)
                        observed = json.loads(root.resolve(source))
                        if "error" in observed["result"]:
                            raise ValueError(observed["result"]["error"])
                        label = observed["result"]["label"]
                        verify_layout(label, observed["calls"], request)
                        master = resolved_master(masters, observed["calls"])
                        resolved.add(tuple(master.items()))
                        expected_style = {"familyRole": "sans", "fontSize": {"value": 14 * scale, "unit": "px"},
                                          "fontWeight": 500, "lineHeight": 1.34,
                                          "letterSpacing": {"value": 0.15 * scale, "unit": "px"}}
                        if label["typography"] != expected_style:
                            raise ValueError("authored Label typography was changed or scaled more than once")
                        if label["layoutDirection"] != direction:
                            raise ValueError("environment direction was lost")
                        drawn = json.loads(root.draw(source))
                        if not drawn["ready"] or drawn["error"] or not drawn["visible"] \
                                or not drawn["ignored"] or drawn["enabled"] or json.loads(drawn["layout"]) != label \
                                or drawn["width"] != label["size"]["width"] or drawn["height"] != label["size"]["height"]:
                            raise ValueError("complete Rust label did not reach native drawing preparation")
                        if drawn["readiness"] != ([True] if count == 0 else [False, True]):
                            raise ValueError("replacement stayed ready during native drawing preparation")
                        count += 1
        valid = source
        invalid = []
        for changes in ({"text": " "}, {"fontFamily": {"role": "numeric", "name": family}},
                        {"fontFamily": {"role": "sans", "name": "Missing Resina Native Family"}},
                        {"maximumSize": {"width": 180, "height": 1}},
                        {"minimumSize": {"width": 64, "height": 1}, "maximumSize": {"width": 180, "height": 20}},
                        {"fontFamily": ["sans", family]}, {"padding": [12, 8, 6, 10]},
                        {"unexpected": True}):
            invalid.append(json.dumps({**request, **changes}))
        invalid.extend((source[:-1] + ',"text":"duplicate"}', json.dumps(list(request.values())),
                        "[]", " " * (8 * 1024 * 1024 + 1)))
        for path in (("environment", "geometry"), ("environment", "geometry", "safeArea"),
                     ("environment", "accessibilityPreferences"), ("environment", "rendererCapabilities")):
            malformed = copy.deepcopy(request)
            owner = malformed
            for part in path[:-1]:
                owner = owner[part]
            owner[path[-1]] = list(owner[path[-1]].values())
            invalid.append(json.dumps(malformed))
        for source in invalid:
            result = json.loads(root.resolve(source))["result"]
            if set(result) != {"error"} or not result["error"]:
                raise ValueError("invalid native layout published a label")
            drawn = json.loads(root.draw(source))
            if drawn["layout"] or drawn["ready"] or drawn["visible"] or not drawn["error"]:
                raise ValueError("failed preparation retained visible/stale content")
        for missing in (True, False):
            result = json.loads(root.rejectProducer(valid, missing))
            if set(result) != {"error"}:
                raise ValueError("missing or malformed native producer published an extent")
        if not json.loads(root.draw(valid))["ready"]:
            raise ValueError("native content did not recover after rejected preparation")
        drawn = json.loads(root.recursiveDraw(valid))
        if not drawn["ready"] or drawn["error"] or not root.property("recursiveError"):
            raise ValueError("recursive preparation changed the outer content publication")
        root.deleteLater()
        application.processEvents()
        if len(resolved) != 1:
            raise ValueError("Command labels resolved more than one master")
    except (ImportError, OSError, ValueError, KeyError, TypeError) as error:
        print("FAIL Qt command label runtime: " + str(error), file=sys.stderr)
        return 1
    print(json.dumps({"resolvedMaster": dict(resolved.pop())}, indent=2))
    print(f"Rust/Qt command content passed {count} actual native layout/drawing preparations, strict rejection and recovery")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
