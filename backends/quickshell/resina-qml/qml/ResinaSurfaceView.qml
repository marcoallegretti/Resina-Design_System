import QtQuick
import Resina.Runtime 0.1

Item {
    id: root
    property string request: ""
    required property int samplesPerAxis
    readonly property string publication: state.publication
    readonly property bool paintReady: publication.length > 0 && state.body !== null && state.body.paintReady
    readonly property string paintError: state.error
    readonly property color foreground: state.foreground
    readonly property rect contentBounds: state.content
    readonly property real nativeDeviceScale: Screen.devicePixelRatio
    readonly property var nativeWindow: Window.window
    implicitWidth: state.front.width
    implicitHeight: state.front.height
    visible: paintReady
    enabled: false
    clip: false
    focus: false
    activeFocusOnTab: false
    Accessible.ignored: true

    function retire() {
        timeout.stop();
        state.pending = null;
        state.context = null;
        state.publication = "";
        if (state.body) {
            state.body.visible = false;
            state.body.destroy();
            state.body = null;
        }
    }

    function invalidate() {
        if (state.executing) {
            state.cancelled = true;
            return;
        }
        retire();
        state.error = "";
        if (request.length)
            Qt.callLater(prepare);
    }

    function context() {
        const origin = mapToItem(null, 0, 0);
        const options = {x:origin.x, y:origin.y, deviceScale:nativeDeviceScale, samplesPerAxis:samplesPerAxis};
        if (!nativeWindow || !resolver.matchesNativeContext(root, options.x, options.y, options.deviceScale)) {
            state.awaitingContext = true;
            throw new Error("Qt surface view requires a translation-only native placement and its window pixel density");
        }
        return options;
    }

    function prepare() {
        if (state.executing)
            return;
        retire();
        state.awaitingContext = false;
        state.error = "";
        if (!request.length)
            return;
        state.executing = true;
        state.cancelled = false;
        let candidate = null;
        try {
            const options = context();
            const result = JSON.parse(resolver.resolveSurfaceView(request, JSON.stringify(options)));
            if (result.error)
                throw new Error(result.error);
            const body = result.paint.body;
            if (body.foreground.colorSpace !== "srgb")
                throw new Error("Qt surface view requires the checked sRGB foreground");
            candidate = Qt.createQmlObject(result.paintSource, root, "ResinaCheckedSurfacePaint");
            if (typeof candidate.paintReady !== "boolean" || typeof candidate.paintError !== "string")
                throw new Error("Qt surface view requires native paint readiness and error reporting");
            candidate.visible = false;
            candidate.x = candidate.paintOriginX - options.x;
            candidate.y = candidate.paintOriginY - options.y;
            candidate.z = -1;
            state.body = candidate;
            candidate = null;
            state.pending = {paint:result.paint, options:options};
            if (state.cancelled)
                throw new Error("Qt surface view preparation was invalidated");
            timeout.start();
            Qt.callLater(completePreparation);
        } catch (error) {
            if (candidate) {
                candidate.visible = false;
                candidate.destroy();
            }
            retire();
            state.error = error.message;
        } finally {
            state.executing = false;
        }
        resume();
    }

    function resume() {
        if (!state.cancelled)
            return;
        state.cancelled = false;
        invalidate();
    }

    function completePreparation() {
        if (state.executing || !state.pending || !state.body)
            return;
        if (state.body.paintError.length) {
            const cause = state.body.paintError;
            retire();
            state.error = cause;
            return;
        }
        if (!state.body.paintReady)
            return;
        state.executing = true;
        timeout.stop();
        try {
            const pending = state.pending;
            const options = context();
            if (options.x !== pending.options.x || options.y !== pending.options.y
                || options.deviceScale !== pending.options.deviceScale || state.cancelled)
                throw new Error("Qt surface view native context changed during preparation");
            const geometry = pending.paint.body.geometry;
            const content = geometry.content;
            const color = pending.paint.body.foreground;
            state.front = Qt.size(geometry.front.bounds.width, geometry.front.bounds.height);
            state.content = Qt.rect(content.offset.x + content.contour.bounds.x,
                content.offset.y + content.contour.bounds.y,
                content.contour.bounds.width, content.contour.bounds.height);
            state.foreground = Qt.rgba(color.components[0], color.components[1], color.components[2], color.alpha);
            state.body.visible = true;
            state.context = pending.options;
            state.pending = null;
            state.publication = JSON.stringify(pending.paint);
        } catch (error) {
            retire();
            state.error = error.message;
        } finally {
            state.executing = false;
        }
        resume();
    }

    Connections {
        target: root.nativeWindow
        function onAfterAnimating() {
            if (state.awaitingContext) {
                try { root.context(); }
                catch (error) { return; }
                root.invalidate();
            } else if (state.context) {
                try {
                    const current = root.context();
                    if (current.x !== state.context.x || current.y !== state.context.y
                        || current.deviceScale !== state.context.deviceScale)
                        root.invalidate();
                } catch (error) {
                    root.invalidate();
                }
            }
        }
    }
    Connections {
        target: state.body
        function onPaintReadyChanged() { Qt.callLater(root.completePreparation); }
        function onPaintErrorChanged() { Qt.callLater(root.completePreparation); }
    }
    Timer {
        id: timeout
        interval: 5000
        onTriggered: {
            root.retire();
            state.error = "Qt surface view native paint preparation timed out";
        }
    }

    onRequestChanged: invalidate()
    onSamplesPerAxisChanged: invalidate()
    onXChanged: invalidate()
    onYChanged: invalidate()
    onScaleChanged: invalidate()
    onRotationChanged: invalidate()
    onParentChanged: invalidate()
    onNativeDeviceScaleChanged: invalidate()
    onNativeWindowChanged: invalidate()
    Component.onDestruction: retire()

    QtObject {
        id: state
        property string publication: ""
        property string error: ""
        property var body: null
        property size front: Qt.size(0, 0)
        property rect content: Qt.rect(0, 0, 0, 0)
        property color foreground: "transparent"
        property bool cancelled: false
        property bool executing: false
        property var pending: null
        property var context: null
        property bool awaitingContext: false
    }
    SurfacePaintResolver { id: resolver }
}
