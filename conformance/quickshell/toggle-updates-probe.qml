import QtQuick
import Quickshell
import Quickshell.Io
import "controls" as Content

FloatingWindow {
    id: root
    implicitWidth: 500
    implicitHeight: 800
    color: "transparent"
    property var cases: null
    property int index: -1
    property string configuration: ""
    property var publications: []
    property bool capturing: false

    function observeFocus() {
        return {activeFocus: toggle.activeFocus, windowActive: toggle.Window.active,
            visible: toggle.visible, enabled: toggle.enabled,
            windowVisible: root.backingWindowVisible};
    }

    function focused(observation) {
        return observation.activeFocus && observation.windowActive && observation.visible
            && observation.enabled && observation.windowVisible;
    }

    function start() {
        if (index < 0) next();
    }

    function fail(message) {
        console.error("RESINA_TOGGLE_FAIL " + message);
        Qt.quit();
    }

    function next() {
        if (!cases || font.status !== FontLoader.Ready) return;
        capturing = true;
        ++index;
        if (index === cases.length) {
            console.log("RESINA_TOGGLE_DONE " + cases.length);
            Qt.quit();
            return;
        }
        other.forceActiveFocus();
        publications = [];
        const scenario = cases[index];
        if (typeof scenario.request === "object") {
            const color = scenario.request.background;
            if (color.colorSpace !== "srgb" || color.alpha !== 1) {
                fail("capture requires opaque sRGB surroundings");
                return;
            }
        }
        carrier.rotation = scenario.rotation || 0;
        toggle.disabledFocusable = scenario.disabledFocusable === true;
        configuration = typeof scenario.request === "string" ? scenario.request : JSON.stringify(scenario.request);
        toggle.controller.dispatch(JSON.stringify({kind:"availability", enabled:scenario.enabled !== false}));
        if (!toggle.setChecked(scenario.checked === true)) {
            fail("checked replacement was rejected");
            return;
        }
        if (scenario.renewPlacement === true) toggle.reconfigure();
        if (scenario.focused === true) toggle.forceActiveFocus();
        deadline.restart();
        capturing = false;
    }

    FontLoader {
        id: font
        source: Quickshell.env("RESINA_TOGGLE_FONT")
        onStatusChanged: {
            if (status === FontLoader.Error) root.fail("font could not be loaded");
            if (status === FontLoader.Ready) {
                if (name !== "DejaVu Sans") root.fail("requires explicit DejaVu Sans");
                else if (root.index < 0) Qt.callLater(root.start);
            }
        }
    }

    FileView {
        path: Quickshell.env("RESINA_TOGGLE_CASES")
        onLoaded: {
            try {
                root.cases = JSON.parse(text());
                if (!Array.isArray(root.cases) || !root.cases.length) throw new Error("empty case list");
                if (root.index < 0) Qt.callLater(root.start);
            } catch (error) { root.fail(error.message); }
        }
        onLoadFailed: error => root.fail("case file could not be loaded: " + error)
    }

    Content.ResinaLabelMeasure { id: measurement }

    function measureComplete(snapshot, scenario) {
        const label = JSON.parse(snapshot).label;
        const style = label.typography;
        const request = {text:scenario.request.content.text, family:scenario.request.content.fontFamily.name,
            pixelSize:style.fontSize.value, weight:style.fontWeight, spacing:style.letterSpacing.value,
            lineHeight:style.lineHeight, maximumWidth:null};
        const natural = JSON.parse(measurement.measureJson(JSON.stringify(request)));
        const fittedRequest = Object.assign({}, request, {maximumWidth:label.labelBounds.width});
        const fitted = JSON.parse(measurement.measureJson(JSON.stringify(fittedRequest)));
        return [{request:request,result:natural}, {request:fittedRequest,result:fitted}];
    }

    Item {
        id: capture
        width: 500
        height: 800
        Rectangle {
            anchors.fill: parent
            Accessible.ignored: true
            color: {
                if (!root.cases || root.index < 0 || root.index >= root.cases.length
                    || typeof root.cases[root.index].request !== "object") return "transparent";
                const components = root.cases[root.index].request.background.components;
                return Qt.rgba(components[0], components[1], components[2], 1);
            }
        }
        Item {
            id: carrier
            anchors.fill: parent
            Content.ResinaToggleControl {
                id: toggle
                x: 50
                y: 50
                configuration: root.configuration
                disabledFocusable: false
                samplesPerAxis: 4
                onSnapshotChanged: {
                    if (!snapshot.length) return;
                    const observation = root.observeFocus();
                    root.publications.push({
                        configuration: root.configuration,
                        snapshot: JSON.parse(snapshot),
                        state: JSON.parse(publication).state,
                        checked: checked,
                        hovered: hovered,
                        nativeFocused: root.focused(observation),
                        nativeFocus: observation
                    });
                }
            }
            Item { id: other; width: 1; height: 1 }
        }
    }

    Timer {
        id: deadline
        interval: 5000
        onTriggered: {
            const observation = root.observeFocus();
            root.fail("case did not complete: " + JSON.stringify({
                name:root.cases[root.index].name, capturing:root.capturing,
                ready:toggle.ready, error:toggle.preparationError, checked:toggle.checked,
                snapshotLength:toggle.snapshot.length, targetLength:toggle.targetPublication.length,
                acceptsNewGestures:toggle.acceptsNewGestures, nativeFocus:observation,
                frameReady:toggle.currentFrame ? toggle.currentFrame.paintReady : false,
                frameError:toggle.currentFrame ? toggle.currentFrame.paintError : ""
            }));
        }
    }

    Timer {
        interval: 20
        running: true
        repeat: true
        onTriggered: {
            if (root.index < 0 || root.capturing) return;
            const scenario = root.cases[root.index];
            const observation = root.observeFocus();
            const result = {
                name: scenario.name,
                ready: toggle.ready,
                error: toggle.preparationError,
                snapshot: toggle.snapshot,
                target: toggle.targetPublication,
                state: JSON.parse(toggle.publication).state,
                checked: toggle.checked,
                hovered: toggle.hovered,
                nativeFocused: root.focused(observation),
                nativeFocus: observation,
                ignored: toggle.Accessible.ignored,
                deviceScale: capture.Screen.devicePixelRatio,
                renderer: capture.GraphicsInfo.api === GraphicsInfo.Software ? "software" : "unsupported",
                publications: root.publications,
                measurements: []
            };
            if (scenario.reject) {
                if (!result.error.length) return;
                root.capturing = true;
                deadline.stop();
                console.log("RESINA_TOGGLE_CASE " + JSON.stringify(result));
                Qt.callLater(root.next);
                return;
            }
            if (result.error.length) { root.fail(result.error); return; }
            if (!result.ready) return;
            if (result.nativeFocused !== (scenario.focused === true)) return;
            try { result.measurements = root.measureComplete(result.snapshot, scenario); }
            catch (error) { root.fail(error.message); return; }
            root.capturing = true;
            const path = Quickshell.env("RESINA_TOGGLE_OUTPUT") + "/" + scenario.name + ".png";
            if (!capture.grabToImage(image => {
                if (!image.saveToFile(path)) { root.fail("capture could not be saved"); return; }
                deadline.stop();
                console.log("RESINA_TOGGLE_CASE " + JSON.stringify(result));
                Qt.callLater(root.next);
            })) root.fail("capture could not be requested");
        }
    }
}
