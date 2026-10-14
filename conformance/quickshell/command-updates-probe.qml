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
    readonly property bool nativeFocused: command.activeFocus && command.Window.active
        && command.visible && command.enabled && root.backingWindowVisible

    function start() {
        if (index < 0) next();
    }

    function fail(message) {
        console.error("RESINA_COMMAND_FAIL " + message);
        Qt.quit();
    }

    function next() {
        if (!cases || font.status !== FontLoader.Ready || alternateFont.status !== FontLoader.Ready) return;
        capturing = true;
        ++index;
        if (index === cases.length) {
            console.log("RESINA_COMMAND_DONE " + cases.length);
            Qt.quit();
            return;
        }
        other.forceActiveFocus();
        publications = [];
        const scenario = cases[index];
        if (scenario.focused === true) {
            const color = scenario.request.body.surroundingColor;
            if (color.colorSpace !== "srgb" || color.alpha !== 1) {
                fail("focused capture requires opaque sRGB surroundings");
                return;
            }
        }
        carrier.rotation = scenario.rotation || 0;
        command.disabledFocusable = scenario.disabledFocusable === true;
        configuration = typeof scenario.request === "string" ? scenario.request : JSON.stringify(scenario.request);
        command.controller.dispatch(JSON.stringify({kind:"availability", enabled:scenario.enabled !== false}));
        if (scenario.renewPlacement === true) command.reconfigure();
        if (scenario.focused === true) command.forceActiveFocus();
        deadline.restart();
        capturing = false;
    }

    FontLoader {
        id: font
        source: Quickshell.env("RESINA_COMMAND_FONT")
        onStatusChanged: {
            if (status === FontLoader.Error) root.fail("font could not be loaded");
            if (status === FontLoader.Ready) {
                if (name !== "DejaVu Sans") root.fail("requires explicit DejaVu Sans");
                else if (root.index < 0) Qt.callLater(root.start);
            }
        }
    }

    FontLoader {
        id: alternateFont
        source: Quickshell.env("RESINA_COMMAND_ALTERNATE_FONT")
        onStatusChanged: {
            if (status === FontLoader.Error) root.fail("alternate font could not be loaded");
            if (status === FontLoader.Ready) {
                if (name !== "DejaVu Sans Mono") root.fail("requires explicit DejaVu Sans Mono");
                else if (root.index < 0) Qt.callLater(root.start);
            }
        }
    }

    FileView {
        path: Quickshell.env("RESINA_COMMAND_CASES")
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
                    || root.cases[root.index].focused !== true) return "transparent";
                const components = root.cases[root.index].request.body.surroundingColor.components;
                return Qt.rgba(components[0], components[1], components[2], 1);
            }
        }
        Item {
            id: carrier
            anchors.fill: parent
            Content.ResinaCommandControl {
                id: command
                x: 50
                y: 50
                configuration: root.configuration
                disabledFocusable: false
                samplesPerAxis: 4
                onSnapshotChanged: {
                    if (snapshot.length) root.publications.push({
                        configuration: root.configuration,
                        snapshot: JSON.parse(snapshot),
                        state: JSON.parse(publication).state,
                        hovered: hovered,
                        nativeFocused: root.nativeFocused
                    });
                }
            }
            Item { id: other; width: 1; height: 1 }
        }
    }

    Timer {
        id: deadline
        interval: 5000
        onTriggered: root.fail("case did not complete: " + JSON.stringify({
            name:root.cases[root.index].name, capturing:root.capturing,
            ready:command.ready, error:command.preparationError,
            snapshotLength:command.snapshot.length, targetLength:command.targetPublication.length,
            acceptsNewGestures:command.acceptsNewGestures, windowVisible:root.visible,
            nativeFocused:root.nativeFocused, windowActive:command.Window.active,
            frameReady:command.currentFrame ? command.currentFrame.paintReady : false,
            frameError:command.currentFrame ? command.currentFrame.paintError : ""
        }))
    }

    Timer {
        interval: 20
        running: true
        repeat: true
        onTriggered: {
            if (root.index < 0 || root.capturing) return;
            const scenario = root.cases[root.index];
            const result = {
                name: scenario.name,
                ready: command.ready,
                error: command.preparationError,
                snapshot: command.snapshot,
                target: command.targetPublication,
                state: JSON.parse(command.publication).state,
                hovered: command.hovered,
                nativeFocused: root.nativeFocused,
                ignored: command.Accessible.ignored,
                deviceScale: capture.Screen.devicePixelRatio,
                renderer: capture.GraphicsInfo.api === GraphicsInfo.Software ? "software" : "unsupported",
                publications: root.publications,
                measurements: []
            };
            if (scenario.reject) {
                if (!result.error.length) return;
                root.capturing = true;
                deadline.stop();
                console.log("RESINA_COMMAND_CASE " + JSON.stringify(result));
                Qt.callLater(root.next);
                return;
            }
            if (result.error.length) { root.fail(result.error); return; }
            if (!result.ready) return;
            if (result.nativeFocused !== (scenario.focused === true)) return;
            try { result.measurements = root.measureComplete(result.snapshot, scenario); }
            catch (error) { root.fail(error.message); return; }
            root.capturing = true;
            const path = Quickshell.env("RESINA_COMMAND_OUTPUT") + "/" + scenario.name + ".png";
            if (!capture.grabToImage(image => {
                if (!image.saveToFile(path)) { root.fail("capture could not be saved"); return; }
                deadline.stop();
                console.log("RESINA_COMMAND_CASE " + JSON.stringify(result));
                Qt.callLater(root.next);
            })) root.fail("capture could not be requested");
        }
    }
}
