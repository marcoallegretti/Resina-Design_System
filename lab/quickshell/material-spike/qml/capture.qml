import QtQuick
import Quickshell
import "scenes.js" as Data

FloatingWindow {
    id: window
    readonly property string outputDir: Quickshell.env("MATERIAL_SPIKE_OUT")
    readonly property var scales: [1, 2]
    property var queue: []
    property int next: 0
    property var shadowed: Quickshell.env("MATERIAL_SPIKE_SHADOW") === "1"
    readonly property bool omitRing: Quickshell.env("MATERIAL_SPIKE_NO_RING") === "1"

    implicitWidth: 400
    implicitHeight: 300
    color: "#202020"

    Item {
        id: stage
        MaterialBody {
            id: body
            x: 0
            y: 0
            shadow: Qt.vector4d(window.shadowed ? 0.35 : 0, 6, 0, 1.5)
            shaderFile: Quickshell.env("MATERIAL_SPIKE_SHADER") || "material.frag.qsb"
        }
    }

    function step() {
        if (next >= queue.length) {
            console.log("CAPTURE_DONE " + queue.length);
            Qt.quit();
            return;
        }
        const job = queue[next++];
        const scene = job.scene;
        const view = scene.views[String(job.scale)];
        body.apply(scene);
        if (omitRing) body.edgeColor = body.bodyColor;
        stage.width = view.width / job.scale;
        stage.height = view.height / job.scale;
        body.x = 0;
        body.y = 0;
        stage.grabToImage(result => {
            const name = scene.name + "@" + job.scale + ".png";
            if (!result.saveToFile(outputDir + "/" + name)) console.log("CAPTURE_FAILED " + name);
            Qt.callLater(step);
        }, Qt.size(view.width, view.height));
    }

    Component.onCompleted: {
        for (const scene of Data.scenes)
            for (const scale of scales)
                queue.push({scene: scene, scale: scale});
        Qt.callLater(step);
    }
}
