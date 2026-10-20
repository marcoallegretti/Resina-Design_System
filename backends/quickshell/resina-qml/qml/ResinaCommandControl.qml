import QtQuick
import Resina.Runtime 0.1

ResinaCommandInput {
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
    // Qt sends no focus event for an ignored item, nor when it stops being
    // ignored: expose the complete node before consumers observe readiness.
    readonly property bool ready: state.published && !Accessible.ignored
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
    // Refreshes keep the previous complete semantics exposed until the next
    // complete semantics replace them; withdrawal clears them.
    Accessible.ignored: !state.semantics || !presented()
    Accessible.role: Accessible.Button
    Accessible.name: state.semantics ? state.semantics.name : ""
    Accessible.description: state.semantics && state.semantics.description !== null ? state.semantics.description : ""
    // Retained semantics must not keep an unavailable Command focusable before its next publication.
    Accessible.focusable: state.semantics ? state.semantics.focusable && enabled : false
    Accessible.pressed: false
    function accessiblePressAction() { invoke(); }

    function presented() { return visible && Boolean(nativeWindow) && nativeWindow.visible; }

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
            target:state.motionTarget,policy:state.policy,time:time}))).channels;
    }

    function motionSource(request, options) {
        // Resolve policy/target through Rust; no material policy is implemented in QML.
        const retained = state.motionTarget ? captureMotion() : state.channels;
        const probe = clockValue(resolver.resolveMotionSnapshot(request,measurement,options,
            JSON.stringify({schemaVersion:"0.1.0",channels:retained,time:0}),false)).motion;
        if (!state.motionTarget) {
            // A reconfiguration resumes the retained trajectory at the instant its clock restarts.
            const restart = clockValue(clock.restart()).previousTime;
            state.retargetTime = null;
            state.resumeTime = null;
            if (state.resume) {
                state.channels = clockValue(resolver.retargetMotion(JSON.stringify({channels:state.resume.channels,
                    target:state.resume.target,policy:state.resume.policy,time:restart}))).channels;
                state.resumeTime = restart;
                state.resume = null;
            }
        } else if (canonical(probe.target) !== canonical(state.motionTarget) || probe.policy !== state.policy) {
            const previousTime = clockValue(clock.restart()).previousTime;
            const previous = clockValue(resolver.retargetMotion(JSON.stringify({channels:state.channels,
                target:state.motionTarget,policy:state.policy,time:previousTime})));
            state.channels = previous.channels;
            state.retargetTime = previousTime;
            state.resumeTime = null;
        }
        state.motionTarget = probe.target;
        state.policy = probe.policy;
        return JSON.stringify({schemaVersion:"0.1.0",channels:state.channels,time:clockValue(clock.sample()).time});
    }

    function fail(cause) {
        state.awaitingWindow = false;
        state.revision = {};
        state.prepared = "";
        state.qualified = false;
        state.armed = false;
        resolver.publishSnapshot("");
        state.semantics = null;
        state.pending = "";
        state.error = cause;
        state.motionActive = false;
        state.channels = null;
        state.motionTarget = null;
        state.resume = null;
        state.publishedSpan = null;
        state.frameDue = false;
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
        if (motionChannels.length) {
            const channels = clockValue(resolver.validateMotionChannels(motionChannels)).channels;
            const profile = canonical(channels);
            if (profile !== state.profile) {
                state.resume = null;
                clock.invalidate();
            }
            state.channels = channels;
            state.profile = profile;
            state.motionTarget = null;
            if (!state.alternate) {
                const component = Qt.createComponent("ResinaCommandView.qml");
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
        state.frameDue = false;
        try {
            const cycle = clockValue(preparationClock.restart()).previousTime;
            const predecessorSpan = state.publishedSpan;
            const publicationDelay = predecessorSpan === null ? null : cycle - predecessorSpan;
            state.publishedSpan = null;
            if (frameOnly !== true || !motionChannels.length) {
                resolver.publishSnapshot("");
                retireViews();
            }
            if (disabledFocusable)
                throw new Error("Qt command control does not support discoverable disabled focus; choose an explicit excluding policy");
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
            state.prepared = JSON.stringify(JSON.parse(controller.publication).state);
            const origin = view.mapToItem(null,0,0);
            const options = JSON.stringify({x:origin.x,y:origin.y,
                deviceScale:view.nativeDeviceScale,samplesPerAxis:samplesPerAxis});
            const candidate = motionChannels.length && root.currentFrame === view ? state.alternate : view;
            state.pendingView = candidate;
            candidate.invalidate();
            if (motionChannels.length)
                candidate.prepare(state.pending,options,motionSource(state.pending,options),state.retargetTime,
                    state.resumeTime,preparationClock,publicationDelay,predecessorSpan);
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
        if (canonical(pending.activation) !== canonical(current) || pending.hovered !== root.hovered) {
            Qt.callLater(refresh);
            return;
        }
        if (canonical(JSON.parse(candidate.snapshot).hitRegion) !== canonical(JSON.parse(targetPublication || "null"))
            || canonical(JSON.parse(targetPublication || "null")) !== canonical(JSON.parse(state.target))) {
            fail("native command publication lost its stable target");
            return;
        }
        const revision = state.revision;
        state.pending = "";
        state.pendingView = null;
        state.semantics = JSON.parse(candidate.snapshot).accessibility;
        if (state.revision !== revision) return;
        resolver.publishFrame(candidate.snapshot,candidate.motion,candidate);
        if (state.revision !== revision) return;
        state.publishedSpan = candidate.motion.length ? clockValue(preparationClock.sample()).time : null;
        if (candidate.motion.length) {
            const sample = JSON.parse(candidate.motion).motion;
            state.motionActive = !(sample.bodyMix.settled && sample.depthScale.settled);
        } else state.motionActive = false;
        if (state.revision !== revision) return;
        state.armed = true;
        if (state.revision !== revision) return;
        if (!ready) fail("native command publication was invalidated");
        else if (state.frameDue) Qt.callLater(resume);
    }

    function advance() {
        // A tick during preparation starts the next frame at publication, not one tick later.
        if (state.running || state.pending.length) state.frameDue = true;
        else refresh(true);
    }

    function resume() { if (state.started && state.frameDue && frames.running) advance(); }

    function reconfigure() {
        if (!state.started) return;
        state.revision = {};
        state.prepared = "";
        const running = state.running;
        state.running = true;
        try {
            const next = motionChannels.length ? JSON.parse(resolver.validateMotionChannels(motionChannels)) : null;
            // Retain the running trajectory and its clock; the next epoch samples it when the clock restarts.
            if (!next || next.error || canonical(next.channels) !== state.profile) state.resume = null;
            else if (state.motionTarget)
                state.resume = {channels:state.channels,target:state.motionTarget,policy:state.policy};
            state.motionActive = false;
            state.motionTarget = null;
            state.publishedSpan = null;
            state.frameDue = false;
            if (!state.resume) clock.invalidate();
            state.qualified = false;
            state.armed = false;
            resolver.publishSnapshot("");
            state.pending = "";
            // Read live values: a binding on the same change signal may not have updated yet.
            if (!presented()) state.semantics = null;
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
    Component.onDestruction: { state.started = false; state.frameDue = false; state.pending = ""; state.pendingView = null; }
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
                && state.prepared === JSON.stringify(JSON.parse(root.controller.publication).state)) return;
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
        id: frames
        running: state.motionActive && motionChannels.length > 0
            && root.visible && root.nativeWindow && root.nativeWindow.visible && !state.error.length
        onTriggered: root.advance()
    }
    ResinaCommandView { id: view; visible: root.ready && root.currentFrame === view && paintReady }
    CommandClock { id: clock }
    CommandClock { id: preparationClock }
    ResinaLabelMeasure { id: measurement }
    CommandSnapshotResolver { id: resolver }
    QtObject {
        id: state
        readonly property bool published: root.acceptsNewGestures && root.pointerOwner
            && root.pointerOwner.acceptsNewGestures && root.snapshot.length > 0
            && root.currentFrame && root.currentFrame.paintReady
        property var revision: ({})
        property var pendingView: null
        property var alternate: null
        property var channels: null
        property var resume: null
        property var motionTarget: null
        property var retargetTime: null
        property var resumeTime: null
        property string profile: ""
        property string policy: ""
        property bool motionActive: false
        property bool started: false
        property bool qualified: false
        property bool armed: false
        property bool running: false
        property bool dirty: false
        property bool awaitingWindow: false
        property bool frameDue: false
        property var publishedSpan: null
        property string target: ""
        property var reservation: null
        property string pending: ""
        property string prepared: ""
        property var semantics: null
        property string error: ""
    }
}
