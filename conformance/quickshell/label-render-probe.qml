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
    readonly property real nativeDeviceScale: capture.Window.window ? capture.Window.window.devicePixelRatio : 0
    readonly property string renderer: capture.GraphicsInfo.api === GraphicsInfo.Software ? "software"
        : capture.GraphicsInfo.api === GraphicsInfo.OpenGL ? "opengl" : "unsupported"

    FontLoader { id: fontAsset; source: Quickshell.env("RESINA_LABEL_FONT") }
    Loader { id: measure; source: Quickshell.env("RESINA_LABEL_MEASURE_QML") }
    Item {
        id: capture
        width: 400
        height: 640
        Rectangle { anchors.fill: parent; color: "white" }
        Loader {
            id: rendered
            visible: !window.showOracle
            active: fontAsset.status === FontLoader.Ready
            source: Quickshell.env("RESINA_LABEL_QML")
        }
        Repeater {
            id: referenceParagraphs
            model: window.oracle ? window.oracle.paragraphs : []
            Text {
                required property var modelData
                visible: window.showOracle
                x: window.oracle ? window.oracle.x : 0
                y: 40 + modelData.y
                width: window.oracle ? window.oracle.nativeWidth : 0
                height: window.oracle ? modelData.height / window.oracle.fontScale : 0
                scale: window.oracle ? window.oracle.fontScale : 1
                transformOrigin: Item.TopLeft
                text: typeof modelData.nativeText === "string" ? modelData.nativeText : ""
                font.family: fontAsset.name
                font.pixelSize: window.oracle ? window.oracle.pixelSize : 20
                font.weight: 400
                font.italic: false
                font.underline: false
                font.strikeout: false
                font.capitalization: Font.MixedCase
                font.letterSpacing: window.oracle ? window.oracle.spacing / window.oracle.fontScale : 0
                lineHeight: window.oracle ? window.oracle.logicalSize * 1.4 / window.oracle.fontScale : 28
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
                const valid = {text:"Reconnect", family:fontAsset.name, pixelSize:20, weight:400, spacing:0,
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
                        console.log("RESINA_LABEL_RENDER_PASS", frames.length, invalid.length, window.nativeDeviceScale, renderer);
                        Qt.quit();
                    }
                });
                return;
            }
            const frame = frames[frameIndex];
            const natural = measure.item.measure(frame.text, fontAsset.name, frame.pixelSize, 400, frame.spacing, 1.4, null);
            const width = frame.ink ? natural.width : 120;
            const fitted = measure.item.measure(frame.text, fontAsset.name, frame.pixelSize, 400, frame.spacing, 1.4, width);
            if (fitted.height > 600)
                throw new Error("fixture exceeds its complete capture viewport");
            const request = {text:frame.text, family:fontAsset.name, pixelSize:frame.pixelSize, weight:400,
                spacing:frame.spacing, lineHeight:1.4, wrappingWidth:width, height:fitted.height,
                color:[0,0,0,1], x:20, y:40};
            rendered.item.prepare(request);
            request.text = "Changed after preparation";
            request.color[0] = 1;
            request.x = 100;
            request.wrappingWidth = 1;
            const transformed = !Number.isInteger(frame.pixelSize) || frame.spacing === 0.15;
            const expandedReference = frame.ink && !transformed;
            const fontScale = transformed ? frame.pixelSize / 1024 : 1;
            let nativeWidth = (expandedReference ? width + 40 : width) / fontScale;
            if (transformed) {
                nativeWidth = Math.floor(nativeWidth * 64) / 64;
                if (nativeWidth * fontScale > width)
                    nativeWidth -= 1 / 64;
            }
            window.oracle = {paragraphs:fitted.paragraphs, x:expandedReference ? 0 : 20,
                nativeWidth:nativeWidth, pixelSize:transformed ? 1024 : frame.pixelSize,
                logicalSize:frame.pixelSize, spacing:frame.spacing,
                fontScale:fontScale};
            Qt.callLater(function() {
                try {
                    if (!rendered.item.paintReady || rendered.item.paintError || rendered.item.clip
                        || rendered.item.enabled || !rendered.item.Accessible.ignored)
                        throw new Error("native drawing profile or readiness differs");
                    for (let index = 0; index < referenceParagraphs.count; ++index) {
                        const paragraph = referenceParagraphs.itemAt(index);
                        paragraph.forceLayout();
                        if (paragraph.text.length && (paragraph.fontInfo.family !== fontAsset.name
                            || paragraph.fontInfo.pixelSize !== window.oracle.pixelSize
                            || paragraph.fontInfo.weight !== 400))
                            throw new Error("independent native drawing substituted its font");
                    }
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
                const expectedRenderer = Quickshell.env("RESINA_LABEL_SCENE_GRAPH") === "rhi" ? "opengl" : "software";
                if (fontAsset.status !== FontLoader.Ready || fontAsset.name !== "DejaVu Sans"
                    || measure.status !== Loader.Ready || rendered.status !== Loader.Ready
                    || Math.abs(window.nativeDeviceScale - scale) > 0.00001 || renderer !== expectedRenderer)
                    throw new Error("font, components, actual device scale or renderer differ");
                frames = [{text:"j", pixelSize:60, spacing:0, ink:true}];
                for (const textScale of [1, 1.5, 2]) {
                    for (const text of ["Reconnect", "Erneut mit dem Netzwerk verbinden", "مرحبا بالعالم",
                        "Reconnect now", "Reconnect to Wi-Fi", "<b>Reconnect</b>",
                        "مرحبا بالعالم\u2029! 123\u2029Reconnect", "\u2029Reconnect\u2029\u2029"])
                        frames.push({text:text, pixelSize:20 * textScale, spacing:0.5, ink:false});
                }
                for (const pixelSize of [12.3, 16.01, 20.5]) {
                    for (const text of ["Reconnect to Wi-Fi", "Erneut mit dem Netzwerk verbinden", "مرحبا بالعالم"])
                        frames.push({text:text, pixelSize:pixelSize, spacing:0.15, ink:false});
                }
                frames.push({text:"مرحبا بالعالم\u2029! 123\u2029Reconnect", pixelSize:20.012, spacing:0.15, ink:false});
                frames.push({text:"j", pixelSize:60.012, spacing:0.15, ink:true});
                drawNext();
            } catch (error) { fail(error); }
        }
    }
}
