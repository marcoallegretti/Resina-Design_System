import QtQuick

Item {
    id: root
    readonly property bool paintReady: state.snapshot !== null
        && paragraphs.count === state.snapshot.paragraphs.length
    readonly property string paintError: state.error
    readonly property real paintOriginX: state.snapshot ? state.snapshot.x : 0
    readonly property real paintOriginY: state.snapshot ? state.snapshot.y : 0
    x: paintOriginX
    y: paintOriginY
    width: state.snapshot ? state.snapshot.width : 0
    height: state.snapshot ? state.snapshot.height : 0
    visible: paintReady
    enabled: false
    clip: false
    focus: false
    activeFocusOnTab: false
    Accessible.ignored: true

    function prepare(request) {
        function checkCoordinate(value) {
            if (!Number.isFinite(value) || !Number.isFinite(Math.fround(value))
                || Math.abs(value - Math.fround(value)) > 1 / 1024)
                throw new Error("Qt Quick label coordinate exceeds the 1/1024 logical px precision budget");
        }
        try {
            const fields = ["text", "family", "pixelSize", "weight", "spacing", "lineHeight",
                "wrappingWidth", "height", "color", "x", "y"];
            if (!request || typeof request !== "object"
                || Object.keys(request).length !== fields.length
                || fields.some(field => !Object.prototype.hasOwnProperty.call(request, field)))
                throw new Error("Qt Quick label preparation requires exactly the declared request fields");
            if (!Array.isArray(request.color) || request.color.length !== 4
                || [0, 1, 2, 3].some(index => !Number.isFinite(request.color[index])
                    || request.color[index] < 0 || request.color[index] > 1))
                throw new Error("Qt Quick label color must have four finite sRGB channels in [0, 1]");
            if (!Number.isFinite(request.wrappingWidth) || request.wrappingWidth <= 0
                || !Number.isFinite(request.height) || request.height <= 0)
                throw new Error("Qt Quick label bounds must be finite and positive");
            for (const value of [request.x, request.y, request.wrappingWidth, request.height,
                request.x + request.wrappingWidth, request.y + request.height])
                checkCoordinate(value);
            for (const [origin, extent] of [[request.x, request.wrappingWidth], [request.y, request.height]]) {
                if (origin + extent <= origin
                    || Math.fround(Math.fround(origin) + Math.fround(extent)) <= Math.fround(origin))
                    throw new Error("Qt Quick label placement loses a positive extent");
            }
            const measured = measurer.measure(request.text, request.family, request.pixelSize,
                request.weight, request.spacing, request.lineHeight, request.wrappingWidth);
            if (Math.abs(measured.height - request.height) > 1 / 1024)
                throw new Error("Qt Quick native complete label height disagrees with resolved layout");
            for (const paragraph of measured.paragraphs) {
                checkCoordinate(paragraph.y);
                const actualY = Math.fround(Math.fround(request.y) + Math.fround(paragraph.y));
                if (Math.abs(actualY - (request.y + paragraph.y)) > 1 / 1024)
                    throw new Error("Qt Quick native paragraph placement exceeds cumulative coordinate precision");
            }
            const style = {
                family: request.family, pixelSize: request.pixelSize, weight: request.weight,
                spacing: request.spacing, fixedLineHeight: request.pixelSize * request.lineHeight,
                wrappingWidth: request.wrappingWidth,
                color: Qt.rgba(...request.color)
            };
            state.snapshot = {
                x: request.x, y: request.y, width: request.wrappingWidth, height: request.height,
                paragraphs: measured.paragraphs.map(paragraph => ({
                    nativeText: paragraph.nativeText, height: paragraph.height, y: paragraph.y, style: style
                }))
            };
            state.error = "";
        } catch (error) {
            state.snapshot = null;
            state.error = error.message;
            throw error;
        }
    }

    QtObject {
        id: state
        property var snapshot: null
        property string error: ""
    }
    ResinaLabelMeasure { id: measurer }
    Repeater {
        id: paragraphs
        model: state.snapshot ? state.snapshot.paragraphs : []
        Text {
            required property var modelData
            x: 0
            y: modelData.y
            width: modelData.style.wrappingWidth
            height: modelData.height
            text: modelData.nativeText
            font.family: modelData.style.family
            font.pixelSize: modelData.style.pixelSize
            font.weight: modelData.style.weight
            font.letterSpacing: modelData.style.spacing
            lineHeight: modelData.style.fixedLineHeight
            color: modelData.style.color
            textFormat: Text.PlainText
            renderType: Text.QtRendering
            fontSizeMode: Text.FixedSize
            font.italic: false
            font.underline: false
            font.strikeout: false
            font.capitalization: Font.MixedCase
            lineHeightMode: Text.FixedHeight
            wrapMode: Text.Wrap
            elide: Text.ElideNone
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignTop
            clip: false
            focus: false
            activeFocusOnTab: false
            Accessible.ignored: true
        }
    }
}
