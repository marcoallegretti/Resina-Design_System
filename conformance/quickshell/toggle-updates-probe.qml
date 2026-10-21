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
    property bool disabledFocusable: false
    property string preparation: ""
    property string unmount: ""
    property var unmounts: []
    readonly property var toggle: mount.item

    function observeFocus() {
        return {activeFocus: root.toggle.activeFocus, windowActive: root.toggle.Window.active,
            visible: root.toggle.visible, enabled: root.toggle.enabled,
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
        if (!cases || !fonts.ready) return;
        capturing = true;
        ++index;
        if (index === cases.length) {
            console.log("RESINA_TOGGLE_DONE " + cases.length);
            Qt.quit();
            return;
        }
        other.forceActiveFocus();
        publications = [];
        unmounts = [];
        const scenario = cases[index];
        if (typeof scenario.request === "object") {
            const color = scenario.request.background;
            if (color.colorSpace !== "srgb" || color.alpha !== 1) {
                fail("capture requires opaque sRGB surroundings");
                return;
            }
        }
        carrier.rotation = scenario.rotation || 0;
        disabledFocusable = scenario.disabledFocusable === true;
        preparation = scenario.focusDuringPreparation === true ? "focus"
            : scenario.focusDuringPreparation === false ? "blur" : "";
        unmount = scenario.unmount || "";
        configuration = typeof scenario.request === "string" ? scenario.request : JSON.stringify(scenario.request);
        if (!apply(scenario)) return;
        if (scenario.renewPlacement === true) root.toggle.reconfigure();
        deadline.restart();
        capturing = false;
    }

    function apply(scenario) {
        root.toggle.controller.dispatch(JSON.stringify({kind:"availability", enabled:scenario.enabled !== false}));
        if (!root.toggle.setChecked(scenario.checked === true)) {
            fail("checked replacement was rejected");
            return false;
        }
        if ((scenario.focused === true && preparation !== "focus") || preparation === "blur")
            root.toggle.forceActiveFocus();
        return true;
    }

    function prepared() {
        if (preparation.length) Qt.callLater(changeFocus);
        if (unmount === "pending") {
            unmount = "";
            Qt.callLater(unload);
        }
    }

    function changeFocus() {
        if (preparation === "focus") root.toggle.forceActiveFocus();
        else if (preparation === "blur") other.forceActiveFocus();
        preparation = "";
    }

    function published() {
        if (unmount !== "callback") return;
        unmount = "";
        unload();
    }

    function unload() {
        unmounts.push({published: root.toggle.snapshot.length > 0, prepared: root.toggle.targetPublication.length > 0,
            nativeFocused: focused(observeFocus())});
        mount.active = false;
        Qt.callLater(reload);
    }

    function reload() {
        mount.active = true;
    }

    Instantiator {
        id: fonts
        readonly property var sources: JSON.parse(Quickshell.env("RESINA_TOGGLE_FONTS") || "[]")
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
            Loader {
                id: mount
                x: 50
                y: 50
                onLoaded: if (root.index >= 0 && root.index < root.cases.length) root.apply(root.cases[root.index])
                sourceComponent: Content.ResinaToggleControl {
                    configuration: root.configuration
                    disabledFocusable: root.disabledFocusable
                    samplesPerAxis: 4
                    onTargetPublicationChanged: if (targetPublication.length) root.prepared()
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
                        root.published();
                    }
                }
            }
            Item { id: other; width: 1; height: 1 }
        }
    }

    Timer {
        id: deadline
        interval: 5000
        onTriggered: {
            if (!root.toggle) { root.fail("case did not remount: " + root.cases[root.index].name); return; }
            const observation = root.observeFocus();
            root.fail("case did not complete: " + JSON.stringify({
                name:root.cases[root.index].name, capturing:root.capturing,
                ready:root.toggle.ready, error:root.toggle.preparationError, checked:root.toggle.checked,
                snapshotLength:root.toggle.snapshot.length, targetLength:root.toggle.targetPublication.length,
                acceptsNewGestures:root.toggle.acceptsNewGestures, nativeFocus:observation,
                frameReady:root.toggle.currentFrame ? root.toggle.currentFrame.paintReady : false,
                frameError:root.toggle.currentFrame ? root.toggle.currentFrame.paintError : ""
            }));
        }
    }

    Timer {
        interval: 20
        running: true
        repeat: true
        onTriggered: {
            if (root.index < 0 || root.capturing || !root.toggle) return;
            const scenario = root.cases[root.index];
            const observation = root.observeFocus();
            const result = {
                name: scenario.name,
                ready: root.toggle.ready,
                error: root.toggle.preparationError,
                snapshot: root.toggle.snapshot,
                target: root.toggle.targetPublication,
                state: JSON.parse(root.toggle.publication).state,
                checked: root.toggle.checked,
                hovered: root.toggle.hovered,
                nativeFocused: root.focused(observation),
                nativeFocus: observation,
                ignored: root.toggle.Accessible.ignored,
                deviceScale: capture.Window.window ? capture.Window.window.devicePixelRatio : null,
                renderer: capture.GraphicsInfo.api === GraphicsInfo.Software ? "software" : "unsupported",
                publications: root.publications,
                unmounts: root.unmounts,
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
