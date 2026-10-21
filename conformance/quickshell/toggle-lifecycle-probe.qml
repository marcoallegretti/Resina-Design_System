import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import "controls" as Content

ShellRoot {
    id: root
    readonly property string generation: Date.now().toString(36) + Math.floor(Math.random() * 1e9).toString(36)
    property var requests: null
    property string configuration: ""
    property bool windowed: true
    property bool shown: true
    property bool mounted: true
    property bool parented: false
    property bool focusTaken: false
    property string reaction: ""
    property int effects: 0
    property int publications: 0
    property int retractions: 0
    property var reactionResult: null
    readonly property var mountItem: windowLoader.item ? windowLoader.item.mount.item : null
    readonly property var toggle: mountItem ? mountItem.toggle : null

    function log(kind, details) {
        console.log("RESINA_LIFECYCLE " + JSON.stringify(Object.assign({kind:kind, generation:generation}, details || {})));
    }

    function fail(message) {
        log("failure", {message:message});
    }

    function start() {
        if (!requests || !fonts.ready) return;
        configuration = JSON.stringify(requests.standard);
        log("started");
    }

    function nativeFocus() {
        if (!toggle) return null;
        return {activeFocus:toggle.activeFocus, windowActive:Boolean(toggle.Window.window && toggle.Window.window.active),
            visible:toggle.visible, enabled:toggle.enabled};
    }

    function targetCenter() {
        if (!toggle || !toggle.targetPublication.length || !toggle.coordinateFrame) return null;
        const bounds = JSON.parse(toggle.targetPublication).bounds;
        const point = toggle.coordinateFrame.mapToItem(null, bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
        return {x:point.x, y:point.y, width:bounds.width, height:bounds.height};
    }

    function observation() {
        return {generation:generation, windowed:windowed, shown:shown, mounted:mounted, parented:parented,
            ready:toggle ? toggle.ready : false, error:toggle ? toggle.preparationError : "",
            state:toggle ? JSON.parse(toggle.publication).state : null, checked:toggle ? toggle.checked : null,
            hovered:toggle ? toggle.hovered : false, nativeFocus:nativeFocus(),
            target:targetCenter(), effects:effects, publications:publications, retractions:retractions, reactionResult:reactionResult,
            otherFocused:Boolean(mountItem && mountItem.other.activeFocus),
            screen:toggle && toggle.Screen.width > 0 ? {width:toggle.Screen.width, height:toggle.Screen.height} : null};
    }

    function published(control) {
        if (!control.snapshot.length) return;
        const snapshot = JSON.parse(control.snapshot);
        log("publication", {snapshotFocused:snapshot.accessibility.state.focused,
            nativeFocused:control.activeFocus && Boolean(control.Window.window && control.Window.window.active),
            snapshotChecked:snapshot.accessibility.state.checked, layoutChecked:snapshot.layout.checked,
            checked:control.checked});
    }

    function activated(control, checked) {
        const publication = JSON.parse(control.togglePublication);
        if (!publication.activation.state.enabled || !publication.activation.activate || publication.checked !== checked
            || control.checked !== checked)
            fail("consumer effect preceded committed activation");
        ++effects;
        log("effect", {effects:effects, state:publication.activation.state, checked:checked, reaction:reaction});
        const mode = reaction;
        reaction = "";
        if (mode === "check") reactionResult = control.setChecked(!checked);
        else if (mode === "unavailable") reactionResult = control.controller.dispatch(JSON.stringify({kind:"availability", enabled:false}));
        else if (mode === "invoke") reactionResult = control.invoke();
        else if (mode === "hide") shown = false;
        else if (mode === "unmount") mounted = false;
        else if (mode === "hideUnmount") { shown = false; mounted = false; }
        else if (mode === "close") windowed = false;
        else if (mode.length) fail("unknown reaction " + mode);
    }

    Connections {
        target: root.toggle ? root.toggle.controller : null
        function onPublicationChanged() { ++root.publications; }
    }
    Connections {
        target: root.toggle
        function onSnapshotChanged() { if (!root.toggle.snapshot.length) ++root.retractions; }
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
                    else if (++fonts.loaded === fonts.sources.length) Qt.callLater(root.start);
                }
            }
        }
    }

    FileView {
        path: Quickshell.env("RESINA_LIFECYCLE_REQUESTS")
        onLoaded: {
            try {
                root.requests = JSON.parse(text());
                if (!root.requests.standard) throw new Error("incomplete requests");
                Qt.callLater(root.start);
            } catch (error) { root.fail(error.message); }
        }
        onLoadFailed: error => root.fail("requests could not be loaded: " + error)
    }

    Loader {
        id: windowLoader
        active: root.windowed

        sourceComponent: PanelWindow {
            property alias mount: mount
            visible: root.shown
            anchors.top: true
            anchors.left: true
            implicitWidth: 800
            implicitHeight: 400
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.namespace: "resina-lifecycle"
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
            color: "#f4f1ea"

            Loader {
                id: mount
                anchors.fill: parent
                active: root.mounted
                sourceComponent: Item {
                    id: holder
                    property alias toggle: toggle
                    property alias other: other

                    Item {
                        id: primary
                        anchors.fill: parent

                        Content.ResinaToggleControl {
                            id: toggle
                            x: 40
                            y: 40
                            parent: root.parented ? alternate : primary
                            configuration: root.configuration
                            disabledFocusable: false
                            samplesPerAxis: 4
                            activeFocusOnTab: true
                            onActivated: checked => root.activated(toggle, checked)
                            onSnapshotChanged: root.published(toggle)
                            onPreparationErrorChanged: if (preparationError.length && root.configuration.length)
                                root.log("rejected", {error:preparationError, shown:root.shown, windowed:root.windowed,
                                    mounted:root.mounted, attached:Boolean(toggle.Window.window && toggle.Window.window.visible)})
                            Component.onCompleted: controller.dispatch(JSON.stringify({kind:"availability", enabled:true}))
                        }
                    }

                    Item {
                        id: alternate
                        anchors.fill: parent
                    }

                    Item {
                        id: other
                        x: 740
                        y: 40
                        width: 40
                        height: 40
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Pane
                        Accessible.name: "Focus target"
                        Accessible.focusable: true
                    }
                }
            }
        }
    }

    Loader {
        active: root.focusTaken

        sourceComponent: PanelWindow {
            anchors.bottom: true
            anchors.right: true
            implicitWidth: 40
            implicitHeight: 40
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.namespace: "resina-focus-peer"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
            color: "#3b4a4e"
        }
    }

    IpcHandler {
        target: "resina"

        function observe(): string {
            try {
                return JSON.stringify(root.observation());
            } catch (error) {
                return JSON.stringify({exception:String(error)});
            }
        }
        function availability(enabled: bool): bool {
            return root.toggle ? root.toggle.controller.dispatch(JSON.stringify({kind:"availability", enabled:enabled})) : false;
        }
        function setChecked(value: bool): bool { return root.toggle ? root.toggle.setChecked(value) : false; }
        function invoke(): bool { return root.toggle ? root.toggle.invoke() : false; }
        function focusToggle(): void { if (root.toggle) root.toggle.forceActiveFocus(); }
        function focusOther(): void { if (root.mountItem) root.mountItem.other.forceActiveFocus(); }
        function setFocusTaken(value: bool): void { root.focusTaken = value; }
        function setShown(value: bool): void { root.shown = value; }
        function setMounted(value: bool): void { root.mounted = value; }
        function setWindowed(value: bool): void { root.windowed = value; }
        function setParented(value: bool): void { root.parented = value; }
        function setReaction(mode: string): void { root.reaction = mode; root.reactionResult = null; }
        function reloadShell(hard: bool): void { root.log("reload", {hard:hard}); Quickshell.reload(hard); }
        function changePending(change: string, then: string): bool {
            const control = root.toggle;
            if (!control) return false;
            let accepted = false;
            if (change === "checked") accepted = control.setChecked(!control.checked);
            else if (change === "availability")
                accepted = control.controller.dispatch(JSON.stringify({kind:"availability", enabled:false}));
            else root.fail("unknown pending change " + change);
            root.log("pending", {change:change, then:then, accepted:accepted, published:control.snapshot.length > 0,
                ready:control.ready});
            if (then === "unmount") root.mounted = false;
            else if (then === "focus") control.forceActiveFocus();
            else if (then === "blur") root.mountItem.other.forceActiveFocus();
            else root.fail("unknown change after pending frame " + then);
            return accepted;
        }
    }
}
