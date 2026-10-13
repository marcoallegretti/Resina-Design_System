import argparse
import copy
import json
import os
from pathlib import Path
import sys

from check_schemas import validator_for
from check_qt_command_label_runtime import verify_layout
from opaque_request_source import pigment_coefficient_failures


ROOT = Path(__file__).resolve().parents[1]


def verify_snapshot(snapshot, request, expected_states, motion=None):
    if set(snapshot) != {"label", "paint", "hitRegion", "accessibility"}:
        raise ValueError("native preparation did not publish exactly the complete channels")
    if motion is not None:
        validator_for("schemas/command-motion-ir.schema.json").validate(motion)
        if snapshot["paint"] != motion["command"]:
            raise ValueError("sampled snapshot paint differs from the guarded motion frame")
    else:
        validator_for("schemas/command-paint-ir.schema.json").validate(snapshot["paint"])
    for member, schema in (("label", "command-label-ir"),
                           ("hitRegion", "hit-region-ir"), ("accessibility", "command-accessibility-ir")):
        validator_for(f"schemas/{schema}.schema.json").validate(snapshot[member])
    body = snapshot["paint"]["paint"]["body"]
    label = snapshot["label"]
    front = body["geometry"]["front"]["bounds"]
    if front != {"x": 0, "y": 0, **label["size"]}:
        raise ValueError("native body and complete label differ in front size/origin")
    target = snapshot["hitRegion"]["bounds"]
    silhouette = body["geometry"]["silhouette"]["bounds"]
    available = request["availableBounds"]
    def contains(outer, inner):
        return all(outer[axis] <= inner[axis]
                   and inner[axis] + inner[extent] <= outer[axis] + outer[extent]
                   for axis, extent in (("x", "width"), ("y", "height")))
    if not contains(target, silhouette) or not contains(available, target):
        raise ValueError("native target does not cover paint within available bounds")
    if any(all(target[axis] < occupied[axis] + occupied[extent]
               and occupied[axis] < target[axis] + target[extent]
               for axis, extent in (("x", "width"), ("y", "height")))
           for occupied in request["occupiedRegions"]):
        raise ValueError("native target overlaps an occupied region")
    if any(target[axis] < request["componentMinimum"][axis] for axis in ("width", "height")):
        raise ValueError("native target loses the explicit component minimum")
    activation = request["activation"]
    if body["states"] != expected_states:
        raise ValueError("body signals differ from the public command-state case")
    phase = next((phase for phase in ("disabled", "pressed", "hover") if phase in expected_states["states"]), "rest")
    if snapshot["paint"]["phase"] != phase:
        raise ValueError("body phase differs from its public state precedence")
    expected_accessibility = {"schemaVersion": "0.1.0", "role": "button", "name": request["content"]["text"],
                              "description": request["description"], "value": None,
                              "state": {"enabled": activation["enabled"], "focused": activation["focused"]},
                              "actions": [{"kind": "invoke", "available": activation["enabled"]}],
                              "focusable": request["focusable"], "relationships": []}
    if snapshot["accessibility"] != expected_accessibility:
        raise ValueError("complete semantic dimensions disagree with native content/current state")
    if ("focus" in snapshot["paint"]["paint"]) != activation["focused"]:
        raise ValueError("navigation paint disagrees with actual focus supplied by the fixture")
    if snapshot["label"]["text"] != request["content"]["text"] \
            or snapshot["label"]["layoutDirection"] != request["content"]["environment"]["layoutDirection"]:
        raise ValueError("snapshot changed content or direction")


def request_for(theme, text):
    environment = json.loads((ROOT / "conformance/headless/valid-request.json").read_text())["environment"]
    environment.update(textScale=1.0006)
    definition = json.loads((ROOT / "definitions/components/command.json").read_text())["variants"]["standard"]["body"]
    background = {"colorSpace": "srgb", "components": [1, 1, 1] if theme == "light" else [0, 0, 0], "alpha": 1}
    return {"content": {"themeSource": (ROOT / f"tokens/themes/{theme}.json").read_text(),
                        "externalSources": {"foundation.json": (ROOT / "tokens/foundation.json").read_text()},
                        "environment": environment, "text": text,
                        "fontFamily": {"role": "sans", "name": "DejaVu Sans"},
                        "minimumSize": {"width": 160, "height": 80},
                        "maximumSize": {"width": 240, "height": 600},
                        "padding": {"start": 32, "end": 32, "top": 26, "bottom": 26}},
            "body": {"surface": {"schemaVersion": "0.2.0", "materialRole": definition["materialRole"],
                                  "colorRole": definition["colorRole"],
                                  "form": {"schemaVersion": "0.1.0", "shape": definition["shape"], "elevation": definition["elevation"]},
                                  "states": {"schemaVersion": "0.1.0", "states": ["rest"]},
                                  "treatmentStack": {"schemaVersion": "0.1.0", "treatments": ["none"]}},
                     "appearance": json.loads((ROOT / "definitions/tier0-surface-appearance.json").read_text()),
                     "commandAppearance": json.loads((ROOT / f"definitions/command-appearance-{theme}.json").read_text()),
                     "foregroundRole": definition["contentRole"], "adjacentColor": background,
                     "postTreatmentBackdrop": background, "surroundingColor": background,
                     "minimumContentContrast": 4.5, "minimumEdgeContrast": 3},
            "activation": {"schemaVersion": "0.1.0", "enabled": True, "focused": False, "hold": None},
            "hovered": False, "description": "Connect using saved settings.", "focusable": True,
            "availableBounds": {"x": -100, "y": -100, "width": 1000, "height": 1000},
            "componentMinimum": {"width": 48, "height": 48}, "occupiedRegions": []}


def main():
    parser = argparse.ArgumentParser(description="Verify complete Qt/Rust command preparation with actual native font extents.")
    parser.add_argument("--module-path", required=True, type=Path)
    parser.add_argument("--font", required=True, type=Path)
    arguments = parser.parse_args()
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    try:
        from PySide6.QtCore import QPluginLoader, QUrl, qVersion
        from PySide6.QtGui import QFontDatabase, QGuiApplication
        from PySide6.QtQml import QQmlComponent, QQmlEngine
        from check_qt_toggle_control_runtime import request_for as toggle_request_for
        from PySide6 import QtQuick  # Load the pinned host's native plugin dependency.
        if qVersion() != "6.11.2":
            raise ValueError("requires Qt 6.11.2")
        application = QGuiApplication([])
        plugin = QPluginLoader(str(arguments.module_path.resolve() / "Resina/Runtime/libResina_Runtime.so"))
        if plugin.instance() is None:
            raise ValueError("Qt plugin factory failed: " + plugin.errorString())
        font_id = QFontDatabase.addApplicationFont(str(arguments.font.resolve()))
        if font_id < 0 or QFontDatabase.applicationFontFamilies(font_id) != ["DejaVu Sans"]:
            raise ValueError("requires explicitly loaded DejaVu Sans")
        engine = QQmlEngine()
        engine.addImportPath(str(arguments.module_path.resolve()))
        component = QQmlComponent(engine)
        component.setData(('''import QtQuick
import Resina.Runtime 0.1
import "''' + (ROOT / "backends/quickshell/resina-qml/qml").as_uri() + '''" as Content
Item {
    property string measurements: "[]"
    CommandSnapshotResolver { id: resolver }
    ToggleSnapshotResolver { id: toggleResolver }
    Content.ResinaLabelMeasure { id: measurement }
    QtObject {
        id: producer
        property var calls: []
        function measureJson(source) {
            const result = measurement.measureJson(source);
            calls.push({request:JSON.parse(source), result:JSON.parse(result)});
            return result;
        }
    }
    function resolve(source) {
        producer.calls = [];
        const result = resolver.resolveSnapshot(source, producer);
        measurements = JSON.stringify(producer.calls);
        return result;
    }
    function composeCommand(source, activation) {
        return resolver.composeRequest(source, activation, false, true);
    }
    function resolveToggle(source) {
        return toggleResolver.resolveToggleSnapshot(source, producer,
            '{"x":0,"y":0,"deviceScale":1,"samplesPerAxis":4}', false);
    }
    function composeToggle(source, activation) {
        return toggleResolver.composeToggleRequest(source, activation, false, false, true);
    }
}
''').encode(), QUrl())
        root = component.create()
        if root is None:
            raise ValueError("native snapshot fixture failed: " + repr(component.errors()))
        cases = [case for case in json.loads((ROOT / "conformance/interaction/command-states-cases.json").read_text()) if "expected" in case]
        count = 0
        for theme in ("light", "dark"):
            for text in ("Reconnect", "Erneut mit dem Netzwerk verbinden", "مرحبا بالعالم\u2029! 123\u2029Reconnect"):
                target = None
                for case in cases:
                    request = request_for(theme, text)
                    request.update(activation=case["request"]["activation"], hovered=case["request"]["hovered"])
                    result = json.loads(root.resolve(json.dumps(request)))
                    if set(result) != {"snapshot"}:
                        raise ValueError(theme + " / " + case["name"] + ": " + str(result)
                                         + " native measurements: " + root.property("measurements"))
                    verify_snapshot(result["snapshot"], request, case["expected"])
                    verify_layout(result["snapshot"]["label"], json.loads(root.property("measurements")), request["content"])
                    current_target = result["snapshot"]["hitRegion"]
                    if target is not None and target != current_target:
                        raise ValueError("interaction feedback moved the reserved native command target")
                    target = current_target
                    count += 1
        baseline = request_for("light", "Reconnect")
        toggle = toggle_request_for()
        for request, path, resolve, compose in (
            (baseline, "/body/appearance/pigmentProfiles", root.resolve, root.composeCommand),
            (toggle, "/track/appearance/pigmentProfiles", root.resolveToggle, root.composeToggle),
            (toggle, "/thumb/appearance/pigmentProfiles", root.resolveToggle, root.composeToggle),
        ):
            activation = json.dumps(request["activation"])
            for name, source in pigment_coefficient_failures(request, path):
                for result in (resolve(source), compose(source, activation)):
                    failure = json.loads(result)
                    if set(failure) != {"error"} or not failure["error"]:
                        raise ValueError(path + " / " + name + ": invalid pigment source published channels")
                for literal in ("0e-400", "-0.000e-400", "1e-400"):
                    valid = source.replace("-1e-400", literal)
                    prepared = json.loads(resolve(valid))
                    if set(prepared) != {"snapshot"}:
                        raise ValueError(path + " / " + name + " / " + literal
                                         + ": valid pigment source failed preparation: " + str(prepared))
                    if set(json.loads(compose(valid, activation))) != {"request"}:
                        raise ValueError(path + ": valid pigment source failed composition")
        bad_requests = []
        for changes in ({"focusable": False}, {"description": " "},
                        {"availableBounds": {"x": 0, "y": 0, "width": 1, "height": 1}},
                        {"occupiedRegions": [{"x": 0, "y": 0, "width": 20, "height": 20}]},
                        {"availableBounds": [-100, -100, 1000, 1000]}, {"unexpected": True}):
            bad_requests.append({**baseline, **changes})
        bad = copy.deepcopy(baseline)
        bad["content"]["padding"] = {"start": 0, "end": 0, "top": 0, "bottom": 0}
        bad_requests.append(bad)
        bad = copy.deepcopy(baseline)
        bad["body"]["surface"]["states"]["states"] = ["hover"]
        bad_requests.append(bad)
        bad = copy.deepcopy(baseline)
        bad["body"]["minimumContentContrast"] = 22
        bad_requests.append(bad)
        for request in bad_requests:
            result = json.loads(root.resolve(json.dumps(request)))
            if set(result) != {"error"} or not result["error"]:
                raise ValueError("failed complete preparation published partial channels")
        for field in ("surface", "appearance", "commandAppearance"):
            invalid = copy.deepcopy(baseline)
            invalid["body"][field] = list(invalid["body"][field].values())
            if set(json.loads(root.resolve(json.dumps(invalid)))) != {"error"}:
                raise ValueError("positional body record published channels")
        for field in ("description", "occupiedRegions", "focusable"):
            invalid = copy.deepcopy(baseline)
            del invalid[field]
            if set(json.loads(root.resolve(json.dumps(invalid)))) != {"error"}:
                raise ValueError("missing explicit snapshot field published channels")
        source = json.dumps(baseline)
        for invalid in ("[]", source[:-1] + ',"hovered":false}', " " * (8 * 1024 * 1024 + 1)):
            if set(json.loads(root.resolve(invalid))) != {"error"}:
                raise ValueError("invalid native snapshot transport published channels")
        if set(json.loads(root.resolve(source))) != {"snapshot"}:
            raise ValueError("complete preparation did not recover after failures")
        root.deleteLater()
        application.processEvents()
    except Exception as error:
        print("FAIL Qt command snapshot runtime: " + str(error), file=sys.stderr)
        return 1
    print(f"Qt/Rust command preparation passed native content, guarded paint, stable targets and semantics across {count} shared-state scenarios; rejection and recovery passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
