import QtQuick

Item {
    visible: false
    width: 0
    height: 0
    focus: false
    activeFocusOnTab: false
    Accessible.ignored: true

    function measureJson(source) {
        try {
            const request = JSON.parse(source);
            const result = measure(request.text, request.family, request.pixelSize, request.weight,
                request.spacing, request.lineHeight, request.maximumWidth);
            return JSON.stringify({kind:"extent", width:result.width, height:result.height});
        } catch (error) {
            return JSON.stringify({kind:"error", message:error.message});
        }
    }

    function fontMapping(pixelSize, spacing) {
        if (!Number.isFinite(pixelSize) || pixelSize <= 0 || pixelSize > 2147483647)
            throw new Error("Qt Quick label pixel size must be finite, positive and representable");
        if (!Number.isFinite(spacing))
            throw new Error("Qt Quick label spacing must be finite");
        const directSpacing = Math.round(spacing * 64) / 64;
        const direct = Number.isInteger(pixelSize) && Math.abs(directSpacing - spacing) <= 1 / 1024;
        const nativeSize = direct ? pixelSize : Math.max(1024, Math.ceil(pixelSize * 8));
        const scale = pixelSize / nativeSize;
        const nativeSpacing = spacing / scale;
        if (nativeSize > 2147483647 || !Number.isFinite(scale) || scale <= 0
            || !Number.isFinite(Math.fround(scale)) || Math.fround(scale) <= 0
            || Math.abs(nativeSize * Math.fround(scale) - pixelSize) > 1 / 1024)
            throw new Error("Qt Quick label font transform exceeds the 1/1024 logical px precision budget");
        if (!Number.isFinite(nativeSpacing) || nativeSpacing * 64 < -2147483648
            || nativeSpacing * 64 > 2147483647)
            throw new Error("Qt Quick label letter spacing exceeds the native fixed-point range");
        return {pixelSize: nativeSize, scale: scale, spacing: nativeSpacing};
    }

    function measure(text, family, pixelSize, weight, spacing, lineHeight, maximumWidth) {
        if (typeof text !== "string" || !text.trim().length)
            throw new Error("Qt Quick label text must not be blank");
        if (text.includes("\u009c"))
            throw new Error("Qt Quick label cannot contain the native multi-length string separator");
        if (typeof family !== "string" || !Qt.fontFamilies().includes(family))
            throw new Error("Qt Quick label font family is not registered");
        if (!Number.isInteger(weight) || weight < 1 || weight > 1000)
            throw new Error("Qt Quick label weight must be an integer in 1..1000");
        if (!Number.isFinite(spacing) || !Number.isFinite(lineHeight) || lineHeight <= 0)
            throw new Error("Qt Quick label spacing and line height must be finite, with positive line height");
        if (maximumWidth !== null && (!Number.isFinite(maximumWidth) || maximumWidth <= 0))
            throw new Error("Qt Quick label wrapping width must be null or finite and positive");
        const fixedHeight = pixelSize * lineHeight;
        if (!Number.isFinite(fixedHeight) || fixedHeight <= 0)
            throw new Error("Qt Quick label resolved line height is not representable");

        const nativeText = text.replace(/\r\n?|[\u000b\u000c\u0085]/g, "\n");
        const mapping = fontMapping(pixelSize, spacing);
        const nativeHeight = fixedHeight / mapping.scale;
        let nativeWidth = maximumWidth === null ? null : maximumWidth / mapping.scale;
        // QTextLayout uses 26.6 widths. A transformed offer must never widen.
        if (nativeWidth !== null && mapping.scale !== 1) {
            nativeWidth = Math.floor(nativeWidth * 64) / 64;
            if (nativeWidth * mapping.scale > maximumWidth)
                nativeWidth -= 1 / 64;
            if (maximumWidth - nativeWidth * mapping.scale > 1 / 1024)
                throw new Error("Qt Quick label wrapping width exceeds the 1/1024 logical px precision budget");
        }
        if (!Number.isFinite(nativeHeight) || nativeHeight <= 0
            || (nativeWidth !== null && (!Number.isFinite(nativeWidth) || nativeWidth <= 0)))
            throw new Error("Qt Quick label native extent is not representable");
        label.font.family = family;
        label.font.pixelSize = mapping.pixelSize;
        label.font.weight = weight;
        label.font.letterSpacing = mapping.spacing;
        if (Math.abs(label.font.letterSpacing * mapping.scale - spacing) > 1 / 1024)
            throw new Error("Qt Quick label letter spacing exceeds the 1/1024 logical px native precision budget");
        label.lineHeight = nativeHeight;
        const paragraphs = [];
        let width = 0;
        let height = 0;
        let face = null;
        for (const paragraph of nativeText.split("\u2029")) {
            label.text = paragraph;
            label.forceLayout();
            label.width = nativeWidth === null ? label.implicitWidth : nativeWidth;
            label.forceLayout();
            // Qt does not refresh fontInfo for an empty layout, which has no glyphs.
            // fontInfo.weight repeats the requested weight even when Qt matched another
            // master. The matched master is published by its style so the font mapping
            // can verify it against the faces it registered.
            if (paragraph.length) {
                if (label.fontInfo.family !== family || label.fontInfo.pixelSize !== mapping.pixelSize)
                    throw new Error("Qt Quick label substituted the requested family or pixel size");
                if (face === null)
                    face = { family: label.fontInfo.family, style: label.fontInfo.styleName };
                else if (face.style !== label.fontInfo.styleName)
                    throw new Error("Qt Quick label paragraphs resolved different font masters");
            }
            const paragraphWidth = (nativeWidth === null ? label.implicitWidth : label.contentWidth) * mapping.scale;
            const paragraphHeight = label.implicitHeight * mapping.scale;
            if (label.truncated || !Number.isFinite(paragraphWidth) || paragraphWidth < 0
                || !Number.isFinite(paragraphHeight) || paragraphHeight <= 0
                || (maximumWidth !== null && paragraphWidth > maximumWidth))
                throw new Error("Qt Quick complete paragraph extent is invalid or exceeds the wrapping width");
            const nextHeight = height + paragraphHeight;
            if (!Number.isFinite(nextHeight) || nextHeight <= height
                || (height > 0 && nextHeight <= paragraphHeight))
                throw new Error("Qt Quick complete paragraph height exceeds representable arithmetic");
            paragraphs.push({ nativeText: paragraph, width: paragraphWidth, height: paragraphHeight, y: height });
            width = Math.max(width, paragraphWidth);
            height = nextHeight;
        }
        if (width <= 0)
            throw new Error("Qt Quick complete label advance must be positive");
        return { width: width, height: height, nativeText: nativeText, paragraphs: paragraphs, face: face,
            fontMapping: mapping, nativeLineHeight: nativeHeight, nativeWrappingWidth: nativeWidth };
    }

    Text {
        id: label
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
