import QtQuick
import Quickshell
import "bench-scene.js" as Data

FloatingWindow {
    id: window
    readonly property int count: parseInt(Quickshell.env("MATERIAL_SPIKE_N") || "10")
    readonly property real bodyWidth: 160
    readonly property real bodyHeight: 80
    readonly property real cell: Math.sqrt(824 * 480 / count)
    readonly property int columns: Math.max(1, Math.floor(824 / cell))
    readonly property real stepX: 824 / columns
    readonly property real stepY: 480 / Math.ceil(count / columns)
    property real time: 0

    implicitWidth: 1024
    implicitHeight: 600
    color: "#f1ede6"

    Recorder {
        id: recorder
        onFinished: {
            console.log("BENCH_RESULT " + recorder.summary({kind: "shader", n: window.count,
                status: repeater.itemAt(0).status, log: repeater.itemAt(0).log}));
            Qt.quit();
        }
    }


    FrameAnimation {
        running: true
        onTriggered: {
            window.time = elapsedTime;
            recorder.tick();
        }
    }

    Repeater {
        id: repeater
        model: window.count
        MaterialBody {
            id: body
            required property int index
            readonly property real phase: (index * 0.137) % 1.2
            readonly property real spring: Math.exp(-5 * ((window.time + phase) % 1.2)) * Math.cos(22 * ((window.time + phase) % 1.2))
            bodyMargin: 8
            bodyWidth: window.bodyWidth * (1 + 0.12 * spring)
            bodyHeight: window.bodyHeight * (1 + 0.12 * spring)
            x: (index % window.columns) * window.stepX + 6 * spring
            y: Math.floor(index / window.columns) * window.stepY + 6 * spring
            radii: Qt.vector4d(bodyHeight / 2, bodyHeight / 2, bodyHeight / 2, bodyHeight / 2)
            Component.onCompleted: applyAppearance(Data.scene)
        }
    }

    Component.onCompleted: Qt.callLater(() => startTimer.start())
    Timer {
        id: startTimer
        interval: 1500
        onTriggered: recorder.start()
    }
}
