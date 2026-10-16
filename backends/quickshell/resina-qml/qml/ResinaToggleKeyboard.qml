import QtQuick
import Resina.Runtime 0.1

Item {
    id: root
    readonly property ToggleController controller: lifecycle
    readonly property string togglePublication: lifecycle.publication
    readonly property string publication: JSON.stringify(JSON.parse(togglePublication).activation)
    readonly property string routingError: nativeState.error
    readonly property var nativeWindow: Window.window
    property bool acceptsNewGestures: true
    enabled: JSON.parse(publication).state.enabled
    activeFocusOnTab: true
    Accessible.ignored: true

    function deliver(event) {
        const accepted = lifecycle.dispatch(JSON.stringify(event));
        nativeState.error = accepted ? "" : lifecycle.lastError;
        return accepted;
    }

    function hasNativeFocus() {
        return enabled && visible && Boolean(nativeWindow) && nativeWindow.visible && observedFocus();
    }

    function observedFocus() {
        return activeFocus && Boolean(nativeWindow) && nativeWindow.active;
    }

    function synchronize() {
        if (nativeState.destroyed)
            return false;
        if (nativeState.interrupted) {
            if (!deliver({kind:"cancel"})) {
                Qt.callLater(synchronize);
                return false;
            }
            nativeState.interrupted = false;
        }
        // Retain a brief focus loss across a publication callback even when the
        // item regains focus before that callback returns.
        if (nativeState.focusLost) {
            if (!deliver({kind:"focus",focused:false})) {
                Qt.callLater(synchronize);
                return false;
            }
            nativeState.focusLost = false;
        }
        const focused = observedFocus();
        if (JSON.parse(publication).state.focused !== focused && !deliver({kind:"focus",focused:focused})) {
            Qt.callLater(synchronize);
            return false;
        }
        return true;
    }

    function interrupt() {
        nativeState.interrupted = true;
        Qt.callLater(synchronize);
    }

    function route(event, down) {
        event.accepted = false;
        const key = event.key === Qt.Key_Space ? "space"
            : (event.key === Qt.Key_Return || event.key === Qt.Key_Enter ? "enter" : null);
        if (!key || !hasNativeFocus())
            return;
        const held = JSON.parse(publication).state.hold;
        if (!acceptsNewGestures && !(held && held.kind === "key" && held.key === key))
            return;
        const terminating = !down && held && held.kind === "key" && held.key === key;
        const unmodified = (event.modifiers & ~Qt.KeypadModifier) === 0;
        if (!unmodified && !terminating)
            return;
        event.accepted = true;
        if (!synchronize() || !hasNativeFocus() || nativeState.interrupted || nativeState.focusLost) {
            interrupt();
            return;
        }
        // Qt exposes synthetic repeat releases as well as repeat presses.
        // Only the actual release terminates a held command gesture.
        if (!down && event.isAutoRepeat)
            return;
        if (!deliver(down ? {kind:"keyDown",key:key,repeat:event.isAutoRepeat}
                          : {kind:"keyUp",key:key}))
            interrupt();
    }

    Keys.onPressed: (event) => route(event, true)
    Keys.onReleased: (event) => route(event, false)
    Keys.onShortcutOverride: (event) => {
        const held = JSON.parse(publication).state.hold;
        const key = event.key === Qt.Key_Space ? "space"
            : (event.key === Qt.Key_Return || event.key === Qt.Key_Enter ? "enter" : null);
        event.accepted = hasNativeFocus()
            && (acceptsNewGestures || (held && held.kind === "key" && held.key === key))
            && (event.modifiers & ~Qt.KeypadModifier) === 0
            && (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter);
    }
    onActiveFocusChanged: {
        if (!activeFocus)
            nativeState.focusLost = true;
        Qt.callLater(synchronize);
    }
    onEnabledChanged: { if (!enabled) interrupt(); else Qt.callLater(synchronize); }
    onVisibleChanged: {
        if (!visible) {
            focus = false;
            interrupt();
        } else {
            Qt.callLater(synchronize);
        }
    }
    onParentChanged: interrupt()
    onNativeWindowChanged: interrupt()
    Component.onCompleted: Qt.callLater(synchronize)
    Component.onDestruction: {
        nativeState.destroyed = true;
        lifecycle.dispatch('{"kind":"cancel"}');
    }

    Connections {
        target: root.nativeWindow
        function onActiveChanged() {
            if (!root.nativeWindow.active)
                root.interrupt();
            Qt.callLater(root.synchronize);
        }
        function onVisibleChanged() {
            if (!root.nativeWindow.visible) {
                root.focus = false;
                root.interrupt();
            }
            Qt.callLater(root.synchronize);
        }
    }
    QtObject {
        id: nativeState
        property bool interrupted: false
        property bool focusLost: false
        property bool destroyed: false
        property string error: ""
    }
    ToggleController { id: lifecycle }
}
