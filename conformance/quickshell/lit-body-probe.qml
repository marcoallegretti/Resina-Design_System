import QtQuick
import Quickshell
import Quickshell.Io
import "controls" as Content

FloatingWindow {
    id: root
    implicitWidth: 500
    implicitHeight: 400
    color: "transparent"
    property var cases: null
    property int index: -1
    property bool capturing: false

    function fail(message) {
        console.error("RESINA_LIT_BODY_FAIL " + message);
        Qt.quit();
    }

    function start() {
        if (index < 0) next();
    }

    function next() {
        if (!cases || !fonts.ready) return;
        if (!command.presentsNativeWindow() || !toggle.presentsNativeWindow()) { exposure.restart(); return; }
        capturing = true;
        ++index;
        if (index === cases.length) {
            console.log("RESINA_LIT_BODY_DONE " + cases.length);
            Qt.quit();
            return;
        }
        const scenario = cases[index];
        const window = capture.Window.window;
        const background = scenario.background;
        backdrop.color = Qt.rgba(background[0], background[1], background[2], 1);
        const options = JSON.stringify({x: view.x, y: view.y, deviceScale: window.devicePixelRatio, samplesPerAxis: 4});
        try {
            command.retire();
            toggle.retire();
            (scenario.kind === "command" ? command : toggle).prepare(JSON.stringify(scenario.request), options);
        } catch (error) {
            fail(scenario.name + ": " + error.message);
            return;
        }
        deadline.restart();
        capturing = false;
    }

    Instantiator {
        id: fonts
        readonly property var sources: JSON.parse(Quickshell.env("RESINA_LIT_BODY_FONTS") || "[]")
        readonly property bool ready: sources.length > 0 && loaded === sources.length
        property int loaded: 0
        model: sources
        delegate: FontLoader {
            required property string modelData
            source: modelData
            onStatusChanged: {
                if (status === FontLoader.Error) root.fail("font could not be loaded");
                if (status === FontLoader.Ready) {
                    if (name !== "Inter") root.fail("requires the explicit Inter masters");
                    else if (++fonts.loaded === fonts.sources.length && root.index < 0) Qt.callLater(root.start);
                }
            }
        }
    }

    FileView {
        path: Quickshell.env("RESINA_LIT_BODY_CASES")
        onLoaded: {
            try {
                root.cases = JSON.parse(text());
                if (!Array.isArray(root.cases) || !root.cases.length) throw new Error("empty case list");
                if (root.index < 0) Qt.callLater(root.start);
            } catch (error) { root.fail(error.message); }
        }
        onLoadFailed: error => root.fail("case file could not be loaded: " + error)
    }

    Item {
        id: capture
        width: 500
        height: 400
        Rectangle {
            id: backdrop
            anchors.fill: parent
            Accessible.ignored: true
            color: "transparent"
        }
        Item {
            id: view
            x: 50
            y: 50
            Content.ResinaCommandView { id: command }
            Content.ResinaToggleView { id: toggle }
        }
    }

    Timer {
        id: exposure
        interval: 20
        onTriggered: root.next()
    }

    Timer {
        id: deadline
        interval: 5000
        onTriggered: {
            const active = root.cases[root.index].kind === "command" ? command : toggle;
            root.fail("case did not complete: " + JSON.stringify({
                name: root.cases[root.index].name, ready: active.paintReady, error: active.paintError}));
        }
    }

    Timer {
        interval: 20
        running: true
        repeat: true
        onTriggered: {
            if (root.index < 0 || root.capturing) return;
            const scenario = root.cases[root.index];
            const active = scenario.kind === "command" ? command : toggle;
            if (active.paintError.length) { root.fail(scenario.name + ": " + active.paintError); return; }
            if (!active.paintReady) return;
            root.capturing = true;
            const report = {
                name: scenario.name,
                snapshot: active.snapshot,
                deviceScale: capture.Window.window.devicePixelRatio,
                renderer: capture.GraphicsInfo.api === GraphicsInfo.Software ? "software" : "unsupported"
            };
            const path = Quickshell.env("RESINA_LIT_BODY_OUTPUT") + "/" + scenario.name + ".png";
            if (!capture.grabToImage(image => {
                if (!image.saveToFile(path)) { root.fail("capture could not be saved"); return; }
                deadline.stop();
                console.log("RESINA_LIT_BODY_CASE " + JSON.stringify(report));
                Qt.callLater(root.next);
            })) root.fail("capture could not be requested");
        }
    }
}
