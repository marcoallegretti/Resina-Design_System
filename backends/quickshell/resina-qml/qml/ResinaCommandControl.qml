import QtQuick
import Resina.Runtime 0.1

ResinaCommandInput {
    id: root
    required property string configuration
    required property bool disabledFocusable
    required property int samplesPerAxis
    // Alias the native read-only property: a consumer can reenter publication
    // without recursively mutating a QML string binding.
    property alias snapshot: resolver.publication
    readonly property bool ready: visible && nativeWindow && nativeWindow.visible
        && snapshot.length > 0 && view.paintReady
        && targetPublication.length > 0
    readonly property string preparationError: state.error
    signal activated()

    coordinateFrame: view
    // Feedback decoding is not gesture authorization. Keep a qualified native
    // owner armed across feedback refreshes after its first complete publication.
    acceptsNewGestures: visible && nativeWindow && nativeWindow.visible
        && state.armed && state.qualified && targetPublication.length > 0
    activeFocusOnTab: activeFocus || (acceptsNewGestures && enabled)
    implicitWidth: state.reservation ? state.reservation.width : 0
    implicitHeight: state.reservation ? state.reservation.height : 0
    Accessible.ignored: !ready
    Accessible.role: Accessible.Button
    Accessible.name: state.semantics ? state.semantics.name : ""
    Accessible.description: state.semantics && state.semantics.description !== null ? state.semantics.description : ""
    Accessible.focusable: state.semantics ? state.semantics.focusable : false
    Accessible.pressed: false
    function accessiblePressAction() { invoke(); }

    function invoke() {
        if (!acceptsNewGestures || !enabled) return false;
        return controller.dispatch('{"kind":"invoke"}');
    }

    function fail(cause) {
        state.revision = {};
        state.qualified = false;
        state.armed = false;
        resolver.publishSnapshot("");
        state.semantics = null;
        state.pending = "";
        state.error = cause;
        root.focus = false;
        const running = state.running;
        state.running = true;
        try {
            view.invalidate();
            invalidateTarget();
            interrupt();
        } finally { state.running = running; }
    }

    function requestFor(activation, hovered) {
        const result = JSON.parse(resolver.composeRequest(configuration, JSON.stringify(activation),
            hovered, activation.enabled || disabledFocusable));
        if (result.error) throw new Error(result.error);
        return result.request;
    }

    function qualify() {
        let reservation = null;
        const endpoints = [];
        // Validate every static feedback endpoint before exposing this profile.
        for (const enabled of [false,true]) {
            for (const focused of (enabled ? [false,true] : [false])) {
                for (const hovered of [false,true]) {
                    for (const pressed of (enabled ? [false,true] : [false])) {
                        const activation = {schemaVersion:"0.1.0",enabled:enabled,focused:focused,
                            hold:pressed ? {kind:"pointer",id:"qualification",inside:true} : null};
                        const source = requestFor(activation,hovered);
                        endpoints.push(source);
                        const result = JSON.parse(resolver.resolveSnapshot(source,measurement));
                        if (result.error) throw new Error(result.error);
                        const target = canonical(result.snapshot.hitRegion);
                        if (reservation !== null && reservation !== target)
                            throw new Error("command feedback changed its stable reservation");
                        reservation = target;
                    }
                }
            }
        }
        state.target = reservation;
        state.reservation = JSON.parse(reservation).bounds;
        view.x = -state.reservation.x;
        view.y = -state.reservation.y;
        const origin = view.mapToItem(null,0,0);
        const options = JSON.stringify({x:origin.x,y:origin.y,
            deviceScale:view.nativeDeviceScale,samplesPerAxis:samplesPerAxis});
        for (const source of endpoints) {
            const prepared = JSON.parse(resolver.qualifySnapshot(source,measurement,options));
            if (prepared.error) throw new Error(prepared.error);
        }
        state.qualified = true;
    }

    function refresh() {
        if (!state.started) return;
        if (state.running) { state.dirty = true; return; }
        state.running = true;
        state.revision = {};
        state.dirty = false;
        try {
            resolver.publishSnapshot("");
            state.semantics = null;
            view.invalidate();
            if (disabledFocusable)
                throw new Error("Qt command control does not support discoverable disabled focus; choose an explicit excluding policy");
            if (!state.qualified) qualify();
            const activation = JSON.parse(publication).state;
            if (!activation.enabled && activation.focused) {
                // Excluding disabled controls relinquish actual Qt focus. Do
                // not invent an unfocused semantic state before Qt confirms it.
                root.focus = false;
                state.pending = "";
                Qt.callLater(synchronize);
                return;
            }
            const request = requestFor(activation,hovered);
            const current = JSON.parse(request);
            if (!targetPublication.length) {
                const region = JSON.parse(state.target);
                const hitRequest = {schemaVersion:"0.1.0",environment:current.content.environment,
                    visualBounds:region.bounds,availableBounds:current.availableBounds,
                    componentMinimum:region.minimumSize,occupiedRegions:current.occupiedRegions};
                if (!prepareTarget(JSON.stringify(hitRequest))) throw new Error(pointerError);
            }
            // Target preparation can expose a retained native hover observation.
            state.pending = requestFor(JSON.parse(publication).state,root.hovered);
            const origin = view.mapToItem(null,0,0);
            view.prepare(state.pending,JSON.stringify({x:origin.x,y:origin.y,
                deviceScale:view.nativeDeviceScale,samplesPerAxis:samplesPerAxis}));
            // Local PNGs may already be decoded. Preserve the asynchronous path otherwise.
            view.completePreparation();
            state.error = "";
        } catch (error) {
            fail(error.message);
        } finally {
            state.running = false;
        }
        finish();
        if (state.dirty) Qt.callLater(refresh);
    }

    function canonical(value) {
        if (value === null || typeof value !== "object") return JSON.stringify(value);
        if (Array.isArray(value)) return "[" + value.map(canonical).join(",") + "]";
        return "{" + Object.keys(value).sort().map(key => JSON.stringify(key)+":"+canonical(value[key])).join(",") + "}";
    }

    function finish() {
        if (state.running || !state.pending.length || !view.paintReady) return;
        const pending = JSON.parse(state.pending);
        const current = JSON.parse(publication).state;
        if (canonical(pending.activation) !== canonical(current) || pending.hovered !== root.hovered) {
            Qt.callLater(refresh);
            return;
        }
        if (canonical(JSON.parse(view.snapshot).hitRegion) !== canonical(JSON.parse(targetPublication || "null"))
            || canonical(JSON.parse(targetPublication || "null")) !== canonical(JSON.parse(state.target))) {
            fail("native command publication lost its stable target");
            return;
        }
        const revision = state.revision;
        state.pending = "";
        state.semantics = JSON.parse(view.snapshot).accessibility;
        if (state.revision !== revision) return;
        resolver.publishSnapshot(view.snapshot);
        if (state.revision !== revision) return;
        state.armed = true;
        if (state.revision !== revision) return;
        if (!ready) fail("native command publication was invalidated");
    }

    function reconfigure() {
        if (!state.started) return;
        state.revision = {};
        const running = state.running;
        state.running = true;
        try {
            state.qualified = false;
            state.armed = false;
            resolver.publishSnapshot("");
            state.pending = "";
            state.semantics = null;
            state.error = "";
            invalidateTarget();
            view.invalidate();
            interrupt();
        } finally { state.running = running; }
        Qt.callLater(refresh);
    }

    onConfigurationChanged: reconfigure()
    onDisabledFocusableChanged: reconfigure()
    onSamplesPerAxisChanged: reconfigure()
    onXChanged: reconfigure()
    onYChanged: reconfigure()
    onVisibleChanged: reconfigure()
    onParentChanged: reconfigure()
    onNativeWindowChanged: reconfigure()
    Component.onCompleted: { state.started = true; Qt.callLater(refresh); }
    onHoveredChanged: { if (state.qualified || !state.error.length) refresh(); }

    Connections {
        target: root.nativeWindow
        function onVisibleChanged() { root.reconfigure(); }
    }
    Connections {
        target: root.controller
        function onPublicationChanged() {
            if (state.qualified || !state.error.length) root.refresh();
        }
        function onActivated() { root.activated(); }
    }
    Connections {
        target: root.pointerOwner
        function onTargetPublicationChanged() {
            if (state.started && state.qualified && !root.targetPublication.length && !state.running)
                root.reconfigure();
        }
    }
    Connections {
        target: view
        function onSnapshotChanged() {
            if (view.snapshot.length) Qt.callLater(root.finish);
            else if (!state.running && resolver.publication.length) root.reconfigure();
        }
        function onPaintErrorChanged() {
            if (!state.running && state.pending.length && view.paintError.length)
                root.fail(view.paintError);
        }
        function onNativeDeviceScaleChanged() { root.reconfigure(); }
    }
    ResinaCommandView { id: view }
    ResinaLabelMeasure { id: measurement }
    CommandSnapshotResolver { id: resolver }
    QtObject {
        id: state
        property var revision: ({})
        property bool started: false
        property bool qualified: false
        property bool armed: false
        property bool running: false
        property bool dirty: false
        property string target: ""
        property var reservation: null
        property string pending: ""
        property var semantics: null
        property string error: ""
    }
}
