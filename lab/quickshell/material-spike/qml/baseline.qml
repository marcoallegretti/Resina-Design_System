import QtQuick
import Quickshell
import "controls" as Content
import "request.js" as Request

FloatingWindow {
    id: window
    readonly property int count: parseInt(Quickshell.env("MATERIAL_SPIKE_N") || "10")
    readonly property real cell: Math.sqrt(824 * 480 / count)
    readonly property int columns: Math.max(1, Math.floor(824 / cell))
    readonly property real stepX: 824 / columns
    readonly property real stepY: 480 / Math.ceil(count / columns)
    readonly property int timeout: parseInt(Quickshell.env("MATERIAL_SPIKE_READY_SECONDS") || "180")
    property int ready: 0
    property int fontsLoaded: 0
    property int publications: 0
    property bool available: true
    property bool measuring: false
    property double startedAt: Date.now()

    implicitWidth: 1024
    implicitHeight: 600
    color: "#ffffff"

    Recorder {
        id: recorder
        warmup: 3
        maxSeconds: 30
        onFinished: {
            window.measuring = false;
            console.log("BENCH_RESULT " + recorder.summary({kind: "raster", n: window.count,
                publications: window.publications, ready: window.ready,
                setup: (recorder.startedAt - window.startedAt) / 1000}));
            Qt.quit();
        }
    }

    Instantiator {
        model: JSON.parse(Quickshell.env("MATERIAL_SPIKE_FONTS") || "[]")
        delegate: FontLoader {
            required property string modelData
            source: modelData
            onStatusChanged: if (status === FontLoader.Ready) ++window.fontsLoaded
        }
    }

    Repeater {
        id: controls
        model: window.fontsLoaded >= 3 ? window.count : 0
        Content.ResinaCommandControl {
            required property int index
            x: (index % window.columns) * window.stepX
            y: Math.floor(index / window.columns) * window.stepY
            configuration: Request.configuration
            motionChannels: Request.channels
            disabledFocusable: false
            samplesPerAxis: 4
            onReadyChanged: window.ready += ready ? 1 : -1
            onMotionChanged: if (window.measuring) ++window.publications
            onPreparationErrorChanged: if (preparationError.length) console.log("BENCH_ERROR " + preparationError)
            Component.onCompleted: controller.dispatch(JSON.stringify({kind: "availability", enabled: true}))
        }
    }

    Timer {
        interval: 250
        running: window.ready >= window.count && window.count > 0
        repeat: false
        onTriggered: {
            window.measuring = true;
            recorder.start();
            toggle.running = true;
        }
    }

    Timer {
        id: toggle
        interval: 120
        repeat: true
        onTriggered: {
            window.available = !window.available;
            for (let i = 0; i < controls.count; ++i)
                controls.itemAt(i).controller.dispatch(JSON.stringify({kind: "availability", enabled: window.available}));
        }
    }

    Timer {
        interval: 1000
        running: true
        repeat: true
        onTriggered: {
            if (!window.measuring && (Date.now() - window.startedAt) / 1000 > window.timeout) {
                console.log("BENCH_RESULT " + JSON.stringify({kind: "raster", n: window.count, ready: window.ready,
                    failed: "not ready within " + window.timeout + " s", intervals: []}));
                Qt.quit();
            }
        }
    }
}
