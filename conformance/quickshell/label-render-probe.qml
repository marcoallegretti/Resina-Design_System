import QtQuick
import Quickshell

FloatingWindow {
    id: window
    implicitWidth: 400
    implicitHeight: 640
    color: "white"
    property var frames: []
    property int frameIndex: 0
    property var oracle: null
    property bool showOracle: false

    FontLoader { id: font; source: Quickshell.env("RESINA_LABEL_FONT") }
    Loader { id: measure; source: Quickshell.env("RESINA_LABEL_MEASURE_QML") }
    Item {
        id: capture
        width: 400
        height: 640
        Rectangle { anchors.fill: parent; color: "white" }
        Loader {
            id: rendered
            visible: !window.showOracle
            active: font.status === FontLoader.Ready
            source: Quickshell.env("RESINA_LABEL_QML")
        }
        Repeater {
            model: window.oracle ? window.oracle.paragraphs : []
            Text {
                required property var modelData
                visible: window.showOracle
                x: window.oracle ? window.oracle.x : 0
                y: 40 + modelData.y
                width: window.oracle ? window.oracle.width : 0
                height: modelData.height
                text: typeof modelData.nativeText === "string" ? modelData.nativeText : ""
                font.family: font.name
                font.pixelSize: window.oracle ? window.oracle.pixelSize : 20
                font.weight: 400
                font.letterSpacing: window.oracle ? window.oracle.spacing : 0
                lineHeight: window.oracle ? window.oracle.pixelSize * 1.4 : 28
                lineHeightMode: Text.FixedHeight
                textFormat: Text.PlainText
                renderType: Text.QtRendering
                fontSizeMode: Text.FixedSize
                wrapMode: Text.Wrap
                elide: Text.ElideNone
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignTop
                color: "black"
                clip: false
            }
        }
    }

    function fail(error) {
        console.error("RESINA_LABEL_RENDER_FAIL", error.message);
        Qt.quit();
    }

    function captureFrame(reference) {
        const path = Quickshell.env("RESINA_LABEL_CAPTURE_DIR") + "/frame-" + frameIndex
            + (reference ? "-reference" : "") + ".png";
        if (!capture.grabToImage(function(result) {
            if (!result.saveToFile(path)) {
                fail(new Error("native capture could not be saved"));
                return;
            }
            if (reference) {
                showOracle = false;
                frameIndex++;
                drawNext();
            } else {
                showOracle = true;
                Qt.callLater(function() { captureFrame(true); });
            }
        }))
            fail(new Error("native capture could not be scheduled"));
    }

    function drawNext() {
        try {
            if (frameIndex === frames.length) {
                const valid = {text:"Reconnect", family:font.name, pixelSize:20, weight:400, spacing:0,
                    lineHeight:1.4, wrappingWidth:120, height:28, color:[0,0,0,1], x:20, y:40};
                const invalid = [
                    Object.assign({}, valid, {height:29}),
                    Object.assign({}, valid, {color:[NaN,0,0,1]}),
                    Object.assign({}, valid, {color:new Array(4)}),
                    Object.assign({}, valid, {pixelSize:20.5}),
                    Object.assign({}, valid, {x:32768 + 9/8192}),
                    Object.assign({}, valid, {x:2 ** 60}),
                    Object.assign({}, valid, {wrappingWidth:0}),
                    Object.assign({}, valid, {family:"Missing Resina Test Family"}),
                    Object.assign({}, valid, {unexpected:true})
                ];
                for (const request of invalid) {
                    let rejected = false;
                    try { rendered.item.prepare(request); }
                    catch (error) { rejected = true; }
                    if (!rejected || rendered.item.paintReady || !rendered.item.paintError)
                        throw new Error("invalid native drawing remained published");
                }
                rendered.item.prepare(valid);
                Qt.callLater(function() {
                    if (!rendered.item.paintReady || rendered.item.paintError)
                        fail(new Error("native drawing did not recover after failure"));
                    else {
                        console.log("RESINA_LABEL_RENDER_PASS", frames.length, invalid.length, window.screen.devicePixelRatio);
                        Qt.quit();
                    }
                });
                return;
            }
            const frame = frames[frameIndex];
            const natural = measure.item.measure(frame.text, font.name, frame.pixelSize, 400, frame.spacing, 1.4, null);
            const width = frame.ink ? natural.width : 120;
            const fitted = measure.item.measure(frame.text, font.name, frame.pixelSize, 400, frame.spacing, 1.4, width);
            if (fitted.height > 600)
                throw new Error("fixture exceeds its complete capture viewport");
            const request = {text:frame.text, family:font.name, pixelSize:frame.pixelSize, weight:400,
                spacing:frame.spacing, lineHeight:1.4, wrappingWidth:width, height:fitted.height,
                color:[0,0,0,1], x:20, y:40};
            rendered.item.prepare(request);
            request.text = "Changed after preparation";
            request.color[0] = 1;
            request.x = 100;
            request.wrappingWidth = 1;
            window.oracle = {paragraphs:fitted.paragraphs, x:frame.ink ? 0 : 20,
                width:frame.ink ? width + 40 : width, pixelSize:frame.pixelSize, spacing:frame.spacing};
            Qt.callLater(function() {
                try {
                    if (!rendered.item.paintReady || rendered.item.paintError || rendered.item.clip
                        || rendered.item.enabled || !rendered.item.Accessible.ignored)
                        throw new Error("native drawing profile or readiness differs");
                    captureFrame(false);
                } catch (error) { fail(error); }
            });
        } catch (error) { fail(error); }
    }

    Timer {
        interval: 100
        running: true
        onTriggered: {
            try {
                const scale = Number(Quickshell.env("RESINA_LABEL_DEVICE_SCALE"));
                if (font.status !== FontLoader.Ready || font.name !== "DejaVu Sans"
                    || measure.status !== Loader.Ready || rendered.status !== Loader.Ready
                    || Math.abs(window.screen.devicePixelRatio - scale) > 0.00001)
                    throw new Error("font, components or actual device scale differ");
                frames = [{text:"j", pixelSize:60, spacing:0, ink:true}];
                for (const textScale of [1, 1.5, 2]) {
                    for (const text of ["Reconnect", "Erneut mit dem Netzwerk verbinden", "مرحبا بالعالم",
                        "Reconnect now", "Reconnect to Wi-Fi", "<b>Reconnect</b>",
                        "مرحبا بالعالم\u2029! 123\u2029Reconnect", "\u2029Reconnect\u2029\u2029"])
                        frames.push({text:text, pixelSize:20 * textScale, spacing:0.5, ink:false});
                }
                drawNext();
            } catch (error) { fail(error); }
        }
    }
}
