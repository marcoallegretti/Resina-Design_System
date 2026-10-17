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
    property string configurationName: ""
    property string motionChannels: ""
    property real commandX: 40
    property bool windowed: true
    property bool shown: true
    property bool mounted: true
    property bool focusTaken: false
    property string reaction: ""
    property bool disabledFocusable: false
    property int effects: 0
    property int publications: 0
    property var reactionResult: null
    readonly property var command: windowLoader.item && windowLoader.item.mount.item
        ? windowLoader.item.mount.item.command : null

    function log(kind, details) {
        console.log("RESINA_LIFECYCLE " + JSON.stringify(Object.assign({kind:kind, generation:generation}, details || {})));
    }

    function fail(message) {
        log("failure", {message:message});
    }

    function start() {
        if (!requests || font.status !== FontLoader.Ready) return;
        configuration = JSON.stringify(requests.standard);
        configurationName = "standard";
        log("started");
    }

    function nativeFocus() {
        if (!command) return null;
        return {activeFocus:command.activeFocus, windowActive:Boolean(command.Window.window && command.Window.window.active),
            visible:command.visible, enabled:command.enabled};
    }

    function targetCenter() {
        if (!command || !command.targetPublication.length || !command.coordinateFrame) return null;
        const bounds = JSON.parse(command.targetPublication).bounds;
        const point = command.coordinateFrame.mapToItem(null, bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
        return {x:point.x, y:point.y, width:bounds.width, height:bounds.height};
    }

    function observation() {
        return {generation:generation, windowed:windowed, shown:shown, mounted:mounted,
            ready:command ? command.ready : false, error:command ? command.preparationError : "",
            state:command ? JSON.parse(command.publication).state : null,
            hovered:command ? command.hovered : false, nativeFocus:nativeFocus(),
            target:targetCenter(), effects:effects, publications:publications, reactionResult:reactionResult,
            otherFocused:Boolean(windowLoader.item && windowLoader.item.mount.item && windowLoader.item.mount.item.other.activeFocus),
            screen:command && command.Screen.width > 0 ? {width:command.Screen.width, height:command.Screen.height} : null};
    }

    function frame(control) {
        if (!control.snapshot.length || !control.motion.length) return;
        log("frame", {time:Date.now() / 1000, configuration:configurationName, snapshot:JSON.parse(control.snapshot),
            motion:JSON.parse(control.motion), state:JSON.parse(control.publication).state, hovered:control.hovered,
            target:JSON.parse(control.targetPublication || "null"), center:targetCenter()});
    }

    function activated(control) {
        const publication = JSON.parse(control.publication);
        if (!publication.state.enabled || !publication.activate) fail("consumer effect preceded committed activation");
        ++effects;
        log("effect", {effects:effects, state:publication.state, reaction:reaction});
        const mode = reaction;
        reaction = "";
        if (mode === "replace") configuration = JSON.stringify(requests.replacement);
        else if (mode === "unavailable") reactionResult = control.controller.dispatch(JSON.stringify({kind:"availability", enabled:false}));
        else if (mode === "invoke") reactionResult = control.invoke();
        else if (mode === "hide") shown = false;
        else if (mode === "unmount") mounted = false;
        else if (mode === "hideUnmount") { shown = false; mounted = false; }
        else if (mode === "close") windowed = false;
        else if (mode.length) fail("unknown reaction " + mode);
    }

    Connections {
        target: root.command ? root.command.controller : null
        function onPublicationChanged() { ++root.publications; }
    }

    FontLoader {
        id: font
        source: Quickshell.env("RESINA_COMMAND_FONT")
        onStatusChanged: {
            if (status === FontLoader.Error) root.fail("font could not be loaded");
            if (status === FontLoader.Ready) {
                if (name !== "DejaVu Sans") root.fail("requires explicit DejaVu Sans");
                else Qt.callLater(root.start);
            }
        }
    }

    FileView {
        path: Quickshell.env("RESINA_LIFECYCLE_REQUESTS")
        onLoaded: {
            try {
                root.requests = JSON.parse(text());
                if (!root.requests.standard || !root.requests.replacement) throw new Error("incomplete requests");
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
            implicitWidth: 400
            implicitHeight: 200
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.namespace: "resina-lifecycle"
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
            color: "#f4f1ea"

            Loader {
                id: mount
                anchors.fill: parent
                active: root.mounted
                sourceComponent: Item {
                    property alias command: command
                    property alias other: other

                    Content.ResinaCommandControl {
                        id: command
                        x: root.commandX
                        y: 40
                        configuration: root.configuration
                        disabledFocusable: root.disabledFocusable
                        motionChannels: root.motionChannels
                        samplesPerAxis: 4
                        activeFocusOnTab: true
                        onActivated: root.activated(command)
                        onMotionChanged: root.frame(command)
                        onPreparationErrorChanged: if (preparationError.length && root.configuration.length)
                            root.log("rejected", {error:preparationError, shown:root.shown, windowed:root.windowed,
                                mounted:root.mounted, attached:Boolean(command.Window.window && command.Window.window.visible)})
                        Component.onCompleted: controller.dispatch(JSON.stringify({kind:"availability", enabled:true}))
                    }

                    Item {
                        id: other
                        x: 300
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
        function configure(name: string): void {
            root.configurationName = name;
            root.configuration = JSON.stringify(root.requests[name]);
        }
        function setMotion(enabled: bool): void { root.motionChannels = enabled ? JSON.stringify(root.requests.channels) : ""; }
        function setCommandX(value: real): void { root.commandX = value; }
        function availability(enabled: bool): bool {
            return root.command ? root.command.controller.dispatch(JSON.stringify({kind:"availability", enabled:enabled})) : false;
        }
        function focusCommand(): void { if (root.command) root.command.forceActiveFocus(); }
        function focusOther(): void { if (windowLoader.item && windowLoader.item.mount.item) windowLoader.item.mount.item.other.forceActiveFocus(); }
        function setFocusTaken(value: bool): void { root.focusTaken = value; }
        function setShown(value: bool): void { root.shown = value; }
        function setMounted(value: bool): void { root.mounted = value; }
        function setWindowed(value: bool): void { root.windowed = value; }
        function setReaction(mode: string): void { root.reaction = mode; root.reactionResult = null; }
        function setDisabledFocusable(value: bool): void { root.disabledFocusable = value; }
        function reloadShell(hard: bool): void { root.log("reload", {hard:hard}); Quickshell.reload(hard); }
    }
}
