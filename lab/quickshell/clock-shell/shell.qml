import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Resina.Runtime 0.1
import "controls" as Resina

ShellRoot {
    id: root
    readonly property string generation: Date.now().toString(36) + Math.floor(Math.random() * 1e9).toString(36)
    readonly property string repository: Quickshell.env("RESINA_ROOT")
    readonly property string theme: Quickshell.env("RESINA_CLOCK_THEME")
        || (platform.colorScheme === "dark" ? "dark" : "light")
    readonly property var screen: Quickshell.screens.length ? Quickshell.screens[0] : null
    readonly property real barHeight: 96
    readonly property real margin: 8
    property var sources: ({})
    property string loadError: ""
    readonly property bool loaded: ["themeSource", "foundation", "appearance", "commandAppearance", "command",
        "preferences", "motionBody", "motionDepth"].every(name => sources[name] !== undefined)
        && typeface.status === FontLoader.Ready && platform.ready
    readonly property var authored: loaded ? JSON.parse(sources.preferences) : null
    readonly property var preferences: authored ? ({
        reducedMotion: platform.reducedMotion !== null && platform.reducedMotion !== undefined
            ? platform.reducedMotion : authored.accessibilityPreferences.reducedMotion,
        reducedTransparency: authored.accessibilityPreferences.reducedTransparency,
        highContrast: platform.highContrast !== null && platform.highContrast !== undefined
            ? platform.highContrast : authored.accessibilityPreferences.highContrast
    }) : null
    readonly property string motionChannels: loaded ? JSON.stringify({
        bodyMix: {dynamics: JSON.parse(sources.motionBody), initial: {position: 0, velocity: 0}},
        depthScale: {dynamics: JSON.parse(sources.motionDepth), initial: {position: 1, velocity: 0}}
    }) : ""
    readonly property var themeRequest: loaded && screen && preferences ? ({
        schemaVersion: "0.1.0",
        themeSource: sources.themeSource,
        externalSources: {"foundation.json": sources.foundation},
        environment: Object.assign({
            schemaVersion: "0.2.0",
            geometry: {width: screen.width, height: screen.height,
                safeArea: {start: 0, end: 0, top: 0, bottom: 0}},
            scale: screen.devicePixelRatio
        }, authored, {accessibilityPreferences: preferences})
    }) : null
    readonly property var backdrop: {
        if (!themeRequest)
            return null;
        const result = JSON.parse(colors.resolveOpaqueColor(JSON.stringify(themeRequest), "surface.base"));
        if (result.error) {
            fail("backdrop colour: " + result.error);
            return null;
        }
        return result.color;
    }

    function fail(message) {
        loadError = message;
        console.error("RESINA_CLOCK_ERROR " + message);
    }

    function report(part, message) {
        if (message.length)
            console.warn("RESINA_CLOCK_PREPARATION " + part + ": " + message);
    }

    function target(control, originX, originY) {
        if (!control || !control.ready)
            return null;
        const bounds = JSON.parse(control.targetPublication).bounds;
        const point = control.coordinateFrame.mapToItem(null, bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
        return {x: originX + point.x, y: originY + point.y};
    }

    function hex(color) {
        return Qt.rgba(color.components[0], color.components[1], color.components[2], color.alpha);
    }

    function surface(materialRole, colorRole, shape, elevation, width, height, adjacent) {
        if (!themeRequest || !adjacent || width <= 0 || height <= 0)
            return "";
        return JSON.stringify({
            schemaVersion: "0.1.0",
            body: {
                schemaVersion: "0.1.0",
                theme: themeRequest,
                surface: {schemaVersion: "0.2.0", materialRole: materialRole, colorRole: colorRole,
                    form: {schemaVersion: "0.1.0", shape: shape, elevation: elevation},
                    states: {schemaVersion: "0.1.0", states: ["rest"]},
                    treatmentStack: {schemaVersion: "0.1.0", treatments: ["none"]}},
                size: {width: width, height: height},
                appearance: JSON.parse(sources.appearance),
                foregroundRole: "content.primary",
                postTreatmentBackdrop: adjacent,
                adjacentColor: adjacent,
                minimumContentContrast: 4.5,
                minimumEdgeContrast: 3
            }
        });
    }

    function command(text, description, surroundingView, bounds) {
        if (!themeRequest || !surroundingView.paintReady)
            return "";
        const body = JSON.parse(surroundingView.publication).body.pigment.body;
        const definition = JSON.parse(sources.command).variants.standard.body;
        return JSON.stringify({
            content: {themeSource: sources.themeSource, externalSources: {"foundation.json": sources.foundation},
                environment: themeRequest.environment, text: text,
                fontFamily: {role: "sans", name: typeface.name},
                minimumSize: {width: 96, height: 48}, maximumSize: {width: 320, height: 200},
                padding: {start: 20, end: 20, top: 12, bottom: 12}},
            body: {surface: {schemaVersion: "0.2.0", materialRole: definition.materialRole,
                    colorRole: definition.colorRole,
                    form: {schemaVersion: "0.1.0", shape: definition.shape, elevation: definition.elevation},
                    states: {schemaVersion: "0.1.0", states: ["rest"]},
                    treatmentStack: {schemaVersion: "0.1.0", treatments: ["none"]}},
                appearance: JSON.parse(sources.appearance),
                commandAppearance: JSON.parse(sources.commandAppearance),
                foregroundRole: definition.contentRole, adjacentColor: body,
                postTreatmentBackdrop: body, surroundingColor: body,
                minimumContentContrast: 4.5, minimumEdgeContrast: 3},
            activation: {schemaVersion: "0.1.0", enabled: true, focused: false, hold: null},
            hovered: false, description: description, focusable: true,
            availableBounds: bounds, componentMinimum: {width: 48, height: 48}, occupiedRegions: []
        });
    }

    FontLoader {
        id: typeface
        source: Quickshell.env("RESINA_CLOCK_FONT")
        onStatusChanged: if (status === FontLoader.Error) root.fail("font could not be loaded")
    }

    Instantiator {
        model: [
            ["themeSource", "tokens/themes/" + root.theme + ".json"],
            ["foundation", "tokens/foundation.json"],
            ["appearance", "definitions/tier0-surface-appearance.json"],
            ["commandAppearance", "definitions/command-appearance-" + root.theme + ".json"],
            ["command", "definitions/components/command.json"],
            ["motionBody", "definitions/motion/elastomer-body.json"],
            ["motionDepth", "definitions/motion/elastomer-depth.json"],
            ["preferences", Qt.resolvedUrl("preferences.json").toString().replace("file://", "")]
        ]
        delegate: FileView {
            required property var modelData
            path: modelData[1].startsWith("/") ? modelData[1] : root.repository + "/" + modelData[1]
            onLoaded: {
                const next = Object.assign({}, root.sources);
                next[modelData[0]] = text();
                root.sources = next;
            }
            onLoadFailed: error => root.fail(modelData[1] + " could not be loaded: " + error)
        }
    }

    PersistentProperties {
        id: persisted
        reloadableId: "resina-clock"
        property bool open: false
    }

    SystemClock {
        id: clock
        precision: SystemClock.Seconds
        enabled: persisted.open
    }

    SurfacePaintResolver { id: colors }
    PlatformAppearance { id: platform }

    PanelWindow {
        id: backdropWindow
        screen: root.screen
        visible: root.backdrop !== null
        anchors { top: true; bottom: true; left: true; right: true }
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.layer: WlrLayer.Background
        WlrLayershell.namespace: "resina-clock-backdrop"
        color: root.backdrop ? root.hex(root.backdrop) : "transparent"
    }

    PanelWindow {
        id: bar
        screen: root.screen
        visible: root.backdrop !== null
        anchors { top: true; left: true; right: true }
        implicitHeight: root.barHeight
        WlrLayershell.namespace: "resina-clock-bar"
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
        color: root.backdrop ? root.hex(root.backdrop) : "transparent"

        Resina.ResinaSurfaceView {
            id: barSurface
            x: root.margin
            y: root.margin
            samplesPerAxis: 2
            request: root.surface("surface.chrome", "surface.chrome", "structural", "floating",
                bar.width - 2 * root.margin, root.barHeight - 3 * root.margin, root.backdrop)
            onPaintErrorChanged: root.report("bar", paintError)
        }

        Loader {
            id: showClock
            active: barSurface.paintReady
            readonly property var control: item
            sourceComponent: Resina.ResinaCommandControl {
                x: barSurface.x + barSurface.contentBounds.x + root.margin
                y: barSurface.y + barSurface.contentBounds.y + root.margin
                samplesPerAxis: 4
                motionChannels: root.motionChannels
                disabledFocusable: false
                activeFocusOnTab: true
                configuration: root.command("Show clock", "Open the clock surface.", barSurface, {
                    x: -root.margin, y: -root.margin,
                    width: barSurface.contentBounds.width, height: barSurface.contentBounds.height})
                onActivated: persisted.open = true
                onPreparationErrorChanged: root.report("Show clock", preparationError)
                Component.onCompleted: controller.dispatch(JSON.stringify({kind: "availability", enabled: true}))
            }
        }
    }

    PanelWindow {
        id: clockWindow
        screen: root.screen
        visible: persisted.open && root.backdrop !== null
        anchors { top: true; left: true }
        margins { top: root.barHeight; left: root.margin }
        implicitWidth: 360
        implicitHeight: 220
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.namespace: "resina-clock-surface"
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
        color: root.backdrop ? root.hex(root.backdrop) : "transparent"

        Resina.ResinaSurfaceView {
            id: clockSurface
            x: root.margin
            y: root.margin
            samplesPerAxis: 2
            request: clockWindow.visible ? root.surface("surface.transient", "surface.high", "rounded", "overlay",
                clockWindow.width - 3 * root.margin, clockWindow.height - 3 * root.margin, root.backdrop) : ""
            onPaintErrorChanged: root.report("clock surface", paintError)

            Column {
                x: clockSurface.contentBounds.x + 16
                y: clockSurface.contentBounds.y + 16
                width: clockSurface.contentBounds.width - 32
                visible: clockSurface.paintReady
                spacing: 4
                Text {
                    id: time
                    text: Qt.locale(root.themeRequest ? root.themeRequest.environment.locale : "en")
                        .toString(clock.date, "HH:mm:ss")
                    color: clockSurface.foreground
                    font.family: typeface.name
                    font.pixelSize: 40
                    font.features: {"tnum": 1}
                }
                Text {
                    text: clock.date.toLocaleDateString(
                        Qt.locale(root.themeRequest ? root.themeRequest.environment.locale : "en"), Locale.LongFormat)
                    color: clockSurface.foreground
                    font.family: typeface.name
                    font.pixelSize: 16
                    width: parent.width
                    wrapMode: Text.WordWrap
                }
            }
        }

        Loader {
            id: closeClock
            active: clockSurface.paintReady
            readonly property var control: item
            sourceComponent: Resina.ResinaCommandControl {
                x: clockSurface.x + clockSurface.contentBounds.x + 16
                y: clockSurface.y + clockSurface.contentBounds.y + clockSurface.contentBounds.height - 64
                samplesPerAxis: 4
                motionChannels: root.motionChannels
                disabledFocusable: false
                activeFocusOnTab: true
                configuration: root.command("Close", "Close the clock surface.", clockSurface, {
                    x: -16, y: 64 - clockSurface.contentBounds.height,
                    width: clockSurface.contentBounds.width, height: clockSurface.contentBounds.height})
                onActivated: persisted.open = false
                onReadyChanged: if (ready) forceActiveFocus()
                onPreparationErrorChanged: root.report("Close", preparationError)
                Keys.onEscapePressed: persisted.open = false
                Component.onCompleted: controller.dispatch(JSON.stringify({kind: "availability", enabled: true}))
            }
        }
    }

    IpcHandler {
        target: "clock"
        function open(): void { persisted.open = true; }
        function close(): void { persisted.open = false; }
        function reload(hard: bool): void { Quickshell.reload(hard); }
        function request(part: string): string {
            if (part === "bar") return barSurface.request;
            if (part === "clock") return clockSurface.request;
            if (part === "showClock") return showClock.control ? showClock.control.configuration : "";
            if (part === "close") return closeClock.control ? closeClock.control.configuration : "";
            return "";
        }
        function observe(): string {
            const show = showClock.control;
            const close = closeClock.control;
            return JSON.stringify({
                generation: root.generation, open: persisted.open, theme: root.theme,
                preferences: root.preferences,
                platform: {ready: platform.ready, error: platform.error, reducedMotion: platform.reducedMotion,
                    highContrast: platform.highContrast, colorScheme: platform.colorScheme},
                outputWidth: root.screen ? root.screen.width : 0, outputHeight: root.screen ? root.screen.height : 0,
                probes: {
                    bar: [barSurface.x + barSurface.contentBounds.x + barSurface.contentBounds.width - 24,
                        barSurface.y + barSurface.contentBounds.y + barSurface.contentBounds.height / 2],
                    clock: [root.margin + clockSurface.x + clockSurface.contentBounds.x + clockSurface.contentBounds.width - 24,
                        root.barHeight + clockSurface.y + clockSurface.contentBounds.y + clockSurface.contentBounds.height - 24]
                },
                targets: {showClock: root.target(show, 0, 0), close: root.target(close, root.margin, root.barHeight)}, loadError: root.loadError, backdrop: root.backdrop,
                errors: {bar: barSurface.paintError, clock: clockSurface.paintError,
                    showClock: show ? show.preparationError : "", close: close ? close.preparationError : ""},
                bar: barSurface.publication.length ? JSON.parse(barSurface.publication) : null,
                clock: clockSurface.publication.length ? JSON.parse(clockSurface.publication) : null,
                showClock: show && show.ready ? {snapshot: JSON.parse(show.snapshot), hovered: show.hovered,
                    state: JSON.parse(show.publication).state, motion: show.motion.length ? JSON.parse(show.motion) : null} : null,
                close: close && close.ready ? {snapshot: JSON.parse(close.snapshot), hovered: close.hovered,
                    state: JSON.parse(close.publication).state, motion: close.motion.length ? JSON.parse(close.motion) : null} : null,
                time: clockSurface.paintReady ? time.text : null,
                clockDate: clock.date.getTime(), clockEnabled: clock.enabled,
                focus: {showClock: Boolean(show && show.activeFocus), close: Boolean(close && close.activeFocus)}
            });
        }
    }
}
