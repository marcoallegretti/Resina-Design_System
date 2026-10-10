import QtQuick
import Resina.Runtime 0.1

Item {
    id: root
    readonly property string snapshot: state.publication
    readonly property bool paintReady: snapshot.length > 0 && state.body !== null
        && state.body.paintReady && label.paintReady
    readonly property string paintError: state.error
    readonly property real nativeDeviceScale: Screen.devicePixelRatio
    readonly property var nativeWindow: Window.window
    implicitWidth: state.size ? state.size.width : 0
    implicitHeight: state.size ? state.size.height : 0
    visible: paintReady
    enabled: false
    clip: false
    focus: false
    activeFocusOnTab: false
    Accessible.ignored: true

    function retire() {
        const executing = state.executing;
        state.executing = true;
        try {
            timeout.stop();
            state.pending = null;
            state.context = null;
            state.publication = "";
            if (state.body) {
                state.body.visible = false;
                state.body.destroy();
                state.body = null;
            }
        } finally {
            state.executing = executing;
        }
    }

    function invalidate() {
        if (state.executing)
            state.cancelled = true;
        else
            state.preparing = false;
        retire();
        state.error = "Qt command view requires preparation after its native context changes";
    }

    function checkContext(options) {
        const origin = mapToItem(null, 0, 0);
        const horizontal = mapToItem(null, 1, 0);
        const vertical = mapToItem(null, 0, 1);
        if (!nativeWindow || options.deviceScale !== nativeDeviceScale
            || Math.abs(origin.x - options.x) > 1 / 1024
            || Math.abs(origin.y - options.y) > 1 / 1024
            || horizontal.x - origin.x !== 1
            || horizontal.y - origin.y !== 0
            || vertical.x - origin.x !== 0
            || vertical.y - origin.y !== 1)
            throw new Error("Qt command view requires matching native translation and pixel density");
    }

    function prepare(source, optionsSource) {
        if (state.preparing || state.executing)
            throw new Error("Qt command view preparation is in progress");
        state.preparing = true;
        state.executing = true;
        state.cancelled = false;
        retire();
        state.error = "";
        let candidate = null;
        try {
            const result = JSON.parse(resolver.resolvePaintedSnapshot(source, measurement, optionsSource));
            if (result.error)
                throw new Error(result.error);
            const options = JSON.parse(optionsSource);
            checkContext(options);
            candidate = Qt.createQmlObject(result.paintSource, root, "ResinaCheckedCommandPaint");
            if (typeof candidate.paintReady !== "boolean" || typeof candidate.paintError !== "string")
                throw new Error("Qt command view requires native paint readiness and error reporting");
            candidate.visible = false;
            candidate.x = candidate.paintOriginX - options.x;
            candidate.y = candidate.paintOriginY - options.y;
            const complete = result.snapshot;
            const content = complete.label;
            const foreground = complete.paint.paint.body.foreground;
            if (foreground.colorSpace !== "srgb")
                throw new Error("Qt command view requires the checked sRGB foreground");
            label.prepare({text:content.text, family:JSON.parse(source).content.fontFamily.name,
                pixelSize:content.typography.fontSize.value, weight:content.typography.fontWeight,
                spacing:content.typography.letterSpacing.value, lineHeight:content.typography.lineHeight,
                wrappingWidth:content.labelBounds.width, height:content.labelBounds.height,
                x:content.labelBounds.x, y:content.labelBounds.y,
                color:foreground.components.concat([foreground.alpha])});
            if (!label.paintReady)
                throw new Error("Qt command view complete glyph preparation is not ready");
            state.body = candidate;
            candidate = null;
            state.body.z = -1;
            state.pending = {complete:complete, options:options};
            if (state.cancelled)
                throw new Error("Qt command view preparation was invalidated");
            timeout.start();
            Qt.callLater(completePreparation);
        } catch (error) {
            if (candidate) {
                candidate.visible = false;
                candidate.destroy();
            }
            retire();
            state.preparing = false;
            state.error = error.message;
            throw error;
        } finally {
            state.executing = false;
        }
    }

    function completePreparation() {
        if (state.executing || !state.pending || !state.body)
            return;
        if (state.body.paintError.length) {
            const cause = state.body.paintError;
            retire();
            state.preparing = false;
            state.error = cause;
            return;
        }
        if (!state.body.paintReady)
            return;
        state.executing = true;
        timeout.stop();
        try {
            const pending = state.pending;
            state.size = pending.complete.label.size;
            checkContext(pending.options);
            if (width !== state.size.width || height !== state.size.height || state.cancelled)
                throw new Error("Qt command view layout changed during preparation");
            state.body.visible = true;
            state.context = pending.options;
            state.publication = JSON.stringify(pending.complete);
            if (state.cancelled || !paintReady)
                throw new Error("Qt command view publication was invalidated");
            state.pending = null;
        } catch (error) {
            retire();
            state.error = error.message;
        } finally {
            state.preparing = false;
            state.executing = false;
        }
    }

    Connections {
        target: root.nativeWindow
        function onAfterAnimating() {
            if (!state.context)
                return;
            try { root.checkContext(state.context); }
            catch (error) { root.invalidate(); }
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
            state.preparing = false;
            state.error = "Qt command view native paint preparation timed out";
        }
    }

    onXChanged: invalidate()
    onYChanged: invalidate()
    onScaleChanged: invalidate()
    onRotationChanged: invalidate()
    onParentChanged: invalidate()
    onNativeDeviceScaleChanged: invalidate()
    onNativeWindowChanged: invalidate()
    onWidthChanged: { if (!state.preparing && snapshot.length) invalidate(); }
    onHeightChanged: { if (!state.preparing && snapshot.length) invalidate(); }

    QtObject {
        id: state
        property string publication: ""
        property string error: ""
        property var body: null
        property var size: null
        property bool preparing: false
        property bool cancelled: false
        property bool executing: false
        property var pending: null
        property var context: null
    }
    CommandSnapshotResolver { id: resolver }
    ResinaLabelMeasure { id: measurement }
    ResinaLabel { id: label }
}
