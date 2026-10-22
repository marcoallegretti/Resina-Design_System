import QtQuick
import Resina.Runtime 0.1

ResinaToggleInput {
    id: root
    required property string configuration
    required property bool disabledFocusable
    required property int samplesPerAxis
    property string motionChannels: ""
    property alias motion: resolver.motion
    property alias currentFrame: resolver.frame
    // Alias the native read-only property: a consumer can reenter publication
    // without recursively mutating a QML string binding.
    property alias snapshot: resolver.publication
    readonly property bool ready: acceptsNewGestures && pointerOwner && pointerOwner.acceptsNewGestures
        && snapshot.length > 0 && currentFrame && currentFrame.paintReady
    readonly property string preparationError: state.error
    readonly property bool checked: JSON.parse(togglePublication).checked
    signal activated(bool checked)

    coordinateFrame: view
    // Feedback decoding is not gesture authorization. Keep a qualified native
    // owner armed across feedback refreshes after its first complete publication.
    acceptsNewGestures: visible && nativeWindow && nativeWindow.visible
        && state.armed && state.qualified && targetPublication.length > 0
    activeFocusOnTab: activeFocus || (acceptsNewGestures && enabled)
    implicitWidth: state.reservation ? state.reservation.width : 0
    implicitHeight: state.reservation ? state.reservation.height : 0
    Accessible.ignored: !ready
    Accessible.role: Accessible.Switch
    Accessible.name: state.semantics ? state.semantics.name : ""
    Accessible.description: state.semantics && state.semantics.description !== null ? state.semantics.description : ""
    Accessible.focusable: state.semantics ? state.semantics.focusable : false
    Accessible.pressed: false
    Accessible.checkable: true
    Accessible.checked: state.semantics ? state.semantics.state.checked : false
    Accessible.checkStateMixed: false
    function accessiblePressAction() { invoke(); }
    function accessibleToggleAction() { invoke(); }
    function setChecked(value) { return controller.setChecked(JSON.stringify({checked:value})); }

    function invoke() {
        if (!acceptsNewGestures || !enabled) return false;
        return controller.dispatch('{"kind":"invoke"}');
    }

    function retireViews() {
        view.invalidate();
        if (state.alternate) state.alternate.invalidate();
        state.pendingView = null;
    }

    function clockValue(source) {
        const value = JSON.parse(source);
        if (value.error) throw new Error(value.error);
        return value;
    }

    function captureMotion() {
        if (!state.channels || !state.motionTarget) return null;
        const time = clockValue(clock.sample()).time;
        return clockValue(resolver.retargetMotion(JSON.stringify({channels:state.channels,
            targets:state.motionTarget,time:time}))).channels;
    }

    function motionSource(request, options) {
        // Resolve policy/target through Rust; no material policy is implemented in QML.
        // Targets and policies follow the request alone, so frames of one request resolve them once.
        if (state.probeRequest !== request) {
            const retained = state.motionTarget ? captureMotion() : state.channels;
            state.probe = clockValue(resolver.resolveMotionSnapshot(request,measurement,options,
                JSON.stringify({channels:retained,time:0}),false)).motion;
            state.probeRequest = request;
        }
        const probe = state.probe;
        const targets = {track:{target:probe.track.target,policy:probe.track.policy},
            thumb:{target:probe.thumb.target,policy:probe.thumb.policy},
            travel:{target:probe.travel.target,policy:probe.travel.policy}};
        if (!state.motionTarget) {
            clockValue(clock.restart());
        } else if (canonical(targets) !== canonical(state.motionTarget)) {
            const previousTime = clockValue(clock.restart()).previousTime;
            const previous = clockValue(resolver.retargetMotion(JSON.stringify({channels:state.channels,
                targets:state.motionTarget,time:previousTime})));
            state.channels = previous.channels;
        }
        state.motionTarget = targets;
        return JSON.stringify({channels:state.channels,time:clockValue(clock.sample()).time});
    }

    function fail(cause) {
        state.awaitingWindow = false;
        state.revision = {};
        state.prepared = "";
        state.probeRequest = "";
        state.qualified = false;
        state.armed = false;
        resolver.publishSnapshot("");
        state.semantics = null;
        state.pending = "";
        state.error = cause;
        state.motionActive = false;
        state.channels = null;
        state.motionTarget = null;
        clock.invalidate();
        root.focus = false;
        const running = state.running;
        state.running = true;
        try {
            retireViews();
            invalidateTarget();
            interrupt();
        } finally { state.running = running; }
    }

    function requestFor(activation, hovered, checked) {
        const result = JSON.parse(resolver.composeToggleRequest(configuration, JSON.stringify(activation),
            hovered, checked, activation.enabled || disabledFocusable));
        if (result.error) throw new Error(result.error);
        return result.request;
    }

    function qualify() {
        let reservation = null;
        const endpoints = [];
        // Validate every static feedback endpoint before exposing this profile.
        for (const checked of [false,true]) {
        for (const enabled of [false,true]) {
            for (const focused of (enabled ? [false,true] : [false])) {
                for (const hovered of [false,true]) {
                    for (const pressed of (enabled ? [false,true] : [false])) {
                        const activation = {schemaVersion:"0.1.0",enabled:enabled,focused:focused,
                            hold:pressed ? {kind:"pointer",id:"qualification",inside:true} : null};
                        const source = requestFor(activation,hovered,checked);
                        endpoints.push(source);
                        const result = JSON.parse(resolver.resolveToggleSnapshot(source,measurement,JSON.stringify({x:0,y:0,deviceScale:view.nativeDeviceScale,samplesPerAxis:samplesPerAxis}),false));
                        if (result.error) throw new Error(result.error);
                        const target = canonical(result.snapshot.hitRegion);
                        if (reservation !== null && reservation !== target)
                            throw new Error("toggle feedback changed its stable reservation");
                        reservation = target;
                    }
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
            const prepared = JSON.parse(resolver.resolveToggleSnapshot(source,measurement,options,false));
            if (prepared.error) throw new Error(prepared.error);
        }
        if (motionChannels.length) {
            const channels = clockValue(resolver.validateMotionChannels(motionChannels)).channels;
            const profile = canonical(channels);
            state.channels = state.resumeChannels && profile === state.profile ? state.resumeChannels : channels;
            state.profile = profile;
            state.resumeChannels = null;
            state.motionTarget = null;
            if (!state.alternate) {
                const component = Qt.createComponent("ResinaToggleView.qml");
                if (component.status !== Component.Ready) throw new Error(component.errorString());
                const candidate = component.createObject(root);
                if (!candidate) throw new Error("Qt motion frame view could not be created");
                state.alternate = candidate;
                candidate.visible = Qt.binding(() => root.ready && root.currentFrame === candidate && candidate.paintReady);
            }
            state.alternate.x = view.x;
            state.alternate.y = view.y;
        }
        state.qualified = true;
    }

    function refresh(frameOnly) {
        if (!state.started) return;
        if (state.running) { state.dirty = true; return; }
        state.running = true;
        state.revision = {};
        state.dirty = false;
        state.awaitingWindow = false;
        try {
            if (frameOnly !== true || !motionChannels.length) {
                resolver.publishSnapshot("");
                state.semantics = null;
                retireViews();
            }
            if (disabledFocusable)
                throw new Error("Qt toggle control does not support discoverable disabled focus; choose an explicit excluding policy");
            if (!view.presentsNativeWindow()) {
                state.pending = "";
                state.awaitingWindow = true;
                return;
            }
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
            const request = requestFor(activation,hovered,root.checked);
            const current = JSON.parse(request);
            if (!targetPublication.length) {
                const region = JSON.parse(state.target);
                const hitRequest = {schemaVersion:"0.1.0",environment:current.content.environment,
                    visualBounds:region.bounds,availableBounds:current.availableBounds,
                    componentMinimum:region.minimumSize,occupiedRegions:current.occupiedRegions};
                if (!prepareTarget(JSON.stringify(hitRequest))) throw new Error(pointerError);
            }
            // Target preparation can expose a retained native hover observation.
            state.pending = requestFor(JSON.parse(publication).state,root.hovered,root.checked);
            state.prepared = JSON.stringify([JSON.parse(controller.publication).activation.state, JSON.parse(controller.publication).checked]);
            const origin = view.mapToItem(null,0,0);
            const options = JSON.stringify({x:origin.x,y:origin.y,
                deviceScale:view.nativeDeviceScale,samplesPerAxis:samplesPerAxis});
            const candidate = motionChannels.length && root.currentFrame === view ? state.alternate : view;
            state.pendingView = candidate;
            candidate.invalidate();
            if (motionChannels.length)
                candidate.prepare(state.pending,options,motionSource(state.pending,options));
            else
                candidate.prepare(state.pending,options);
            // Local PNGs may already be decoded. Preserve the asynchronous path otherwise.
            candidate.completePreparation();
            state.error = "";
        } catch (error) {
            fail(error.message);
        } finally {
            state.running = false;
        }
        // Cached image decode can finish during activation publication. Notify
        // component consumers after that controller commit guard has returned.
        Qt.callLater(finish);
        if (state.dirty) Qt.callLater(refresh);
    }

    function canonical(value) {
        if (value === null || typeof value !== "object") return JSON.stringify(value);
        if (Array.isArray(value)) return "[" + value.map(canonical).join(",") + "]";
        return "{" + Object.keys(value).sort().map(key => JSON.stringify(key)+":"+canonical(value[key])).join(",") + "}";
    }

    function finish() {
        if (state.running || !state.pending.length) return;
        if (!synchronize()) {
            Qt.callLater(finish);
            return;
        }
        const candidate = state.pendingView;
        if (!state.pending.length || !candidate || !candidate.paintReady) return;
        const pending = JSON.parse(state.pending);
        const current = JSON.parse(publication).state;
        if (canonical(pending.activation) !== canonical(current) || pending.hovered !== root.hovered || pending.checked !== root.checked) {
            Qt.callLater(refresh);
            return;
        }
        if (canonical(JSON.parse(candidate.snapshot).hitRegion) !== canonical(JSON.parse(targetPublication || "null"))
            || canonical(JSON.parse(targetPublication || "null")) !== canonical(JSON.parse(state.target))) {
            fail("native toggle publication lost its stable target");
            return;
        }
        const revision = state.revision;
        state.pending = "";
        state.pendingView = null;
        state.semantics = JSON.parse(candidate.snapshot).accessibility;
        if (state.revision !== revision) return;
        resolver.publishFrame(candidate.snapshot,candidate.motion,candidate);
        if (state.revision !== revision) return;
        if (candidate.motion.length) {
            const sample = JSON.parse(candidate.motion).motion;
            state.motionActive = !(sample.track.bodyMix.settled && sample.track.depthScale.settled
                && sample.thumb.bodyMix.settled && sample.thumb.depthScale.settled && sample.travel.trajectory.settled);
        } else state.motionActive = false;
        if (state.revision !== revision) return;
        state.armed = true;
        if (state.revision !== revision) return;
        if (!ready) fail("native toggle publication was invalidated");
    }

    function reconfigure() {
        if (!state.started) return;
        state.revision = {};
        state.prepared = "";
        state.probeRequest = "";
        const running = state.running;
        state.running = true;
        try {
            const next = motionChannels.length ? JSON.parse(resolver.validateMotionChannels(motionChannels)) : null;
            state.resumeChannels = next && !next.error && canonical(next.channels) === state.profile ? captureMotion() : null;
            state.motionActive = false;
            state.motionTarget = null;
            clock.invalidate();
            state.qualified = false;
            state.armed = false;
            resolver.publishSnapshot("");
            state.pending = "";
            state.semantics = null;
            state.error = "";
            invalidateTarget();
            retireViews();
            interrupt();
        } catch (error) { fail(error.message); }
        finally { state.running = running; }
        Qt.callLater(refresh);
    }

    onConfigurationChanged: reconfigure()
    onMotionChannelsChanged: reconfigure()
    onDisabledFocusableChanged: reconfigure()
    onSamplesPerAxisChanged: reconfigure()
    onXChanged: reconfigure()
    onYChanged: reconfigure()
    onVisibleChanged: reconfigure()
    onParentChanged: reconfigure()
    onNativeWindowChanged: reconfigure()
    Component.onCompleted: { state.started = true; Qt.callLater(refresh); }
    Component.onDestruction: { state.started = false; state.pending = ""; state.pendingView = null; }
    onHoveredChanged: { if (state.qualified || !state.error.length) refresh(); }

    Connections {
        target: root.nativeWindow
        function onVisibleChanged() { root.reconfigure(); }
        function onAfterAnimating() {
            if (state.awaitingWindow && view.presentsNativeWindow()) {
                state.awaitingWindow = false;
                Qt.callLater(root.refresh);
            }
        }
    }
    Connections {
        target: root.controller
        function onPublicationChanged() {
            // Key auto-repeat republishes an unchanged state; keep the current frame.
            if (root.keyRepeat && state.started && state.qualified && !state.error.length
                && state.prepared === JSON.stringify([JSON.parse(root.controller.publication).activation.state, JSON.parse(root.controller.publication).checked])) return;
            if (state.qualified || !state.error.length) root.refresh();
        }
        function onActivated(checked) { root.activated(checked); }
    }
    Connections {
        target: root.pointerOwner
        function onTargetPublicationChanged() {
            if (state.started && state.qualified && !root.targetPublication.length && !state.running)
                root.reconfigure();
        }
    }
    Connections {
        target: state.pendingView
        function onSnapshotChanged() { if (state.pendingView && state.pendingView.snapshot.length) Qt.callLater(root.finish); }
        function onPaintErrorChanged() {
            if (!state.running && state.pending.length && state.pendingView && state.pendingView.paintError.length) {
                if (view.presentsNativeWindow()) root.fail(state.pendingView.paintError);
                else root.reconfigure();
            }
        }
    }
    Connections {
        target: root.currentFrame
        function onSnapshotChanged() {
            if (root.currentFrame && !root.currentFrame.snapshot.length && !state.running && resolver.publication.length)
                root.reconfigure();
        }
    }
    Connections {
        target: view
        function onNativeDeviceScaleChanged() { root.reconfigure(); }
    }
    FrameAnimation {
        running: state.motionActive && motionChannels.length > 0 && !state.pending.length
            && root.visible && root.nativeWindow && root.nativeWindow.visible && !state.error.length
        onTriggered: root.refresh(true)
    }
    ResinaToggleView { id: view; visible: root.ready && root.currentFrame === view && paintReady }
    CommandClock { id: clock }
    ResinaLabelMeasure { id: measurement }
    ToggleSnapshotResolver { id: resolver }
    QtObject {
        id: state
        property var revision: ({})
        property var pendingView: null
        property var alternate: null
        property var channels: null
        property var resumeChannels: null
        property var motionTarget: null
        property string profile: ""
        property var probe: null
        property string probeRequest: ""
        property bool motionActive: false
        property bool started: false
        property bool qualified: false
        property bool armed: false
        property bool running: false
        property bool dirty: false
        property bool awaitingWindow: false
        property string target: ""
        property var reservation: null
        property string pending: ""
        property string prepared: ""
        property var semantics: null
        property string error: ""
    }
}
