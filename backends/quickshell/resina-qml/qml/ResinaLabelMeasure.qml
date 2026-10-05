import QtQuick

Item {
    visible: false
    width: 0
    height: 0
    focus: false
    activeFocusOnTab: false
    Accessible.ignored: true

    function measure(text, family, pixelSize, weight, spacing, lineHeight, maximumWidth) {
        if (typeof text !== "string" || !text.trim().length)
            throw new Error("Qt Quick label text must not be blank");
        if (text.includes("\u009c"))
            throw new Error("Qt Quick label cannot contain the native multi-length string separator");
        if (typeof family !== "string" || !Qt.fontFamilies().includes(family))
            throw new Error("Qt Quick label font family is not registered");
        if (!Number.isInteger(pixelSize) || pixelSize <= 0 || pixelSize > 2147483647)
            throw new Error("Qt Quick label pixel size must be a positive native integer");
        if (!Number.isInteger(weight) || weight < 1 || weight > 1000)
            throw new Error("Qt Quick label weight must be an integer in 1..1000");
        if (!Number.isFinite(spacing) || !Number.isFinite(lineHeight) || lineHeight <= 0)
            throw new Error("Qt Quick label spacing and line height must be finite, with positive line height");
        // QFont converts spacing to a signed 26.6 fixed-point value before shaping.
        if (spacing * 64 < -2147483648 || spacing * 64 > 2147483647)
            throw new Error("Qt Quick label letter spacing exceeds the native fixed-point range");
        if (maximumWidth !== null && (!Number.isFinite(maximumWidth) || maximumWidth <= 0))
            throw new Error("Qt Quick label wrapping width must be null or finite and positive");
        const fixedHeight = pixelSize * lineHeight;
        if (!Number.isFinite(fixedHeight) || fixedHeight <= 0)
            throw new Error("Qt Quick label resolved line height is not representable");

        const nativeText = text.replace(/\r\n?|[\u000b\u000c\u0085]/g, "\n");
        label.font.family = family;
        label.font.pixelSize = pixelSize;
        label.font.weight = weight;
        label.font.letterSpacing = spacing;
        if (Math.abs(label.font.letterSpacing - spacing) > 1 / 1024)
            throw new Error("Qt Quick label letter spacing exceeds the 1/1024 logical px native precision budget");
        label.lineHeight = fixedHeight;
        const paragraphs = [];
        let width = 0;
        let height = 0;
        for (const paragraph of nativeText.split("\u2029")) {
            label.text = paragraph;
            label.forceLayout();
            label.width = maximumWidth === null ? label.implicitWidth : maximumWidth;
            label.forceLayout();
            // Qt does not refresh fontInfo for an empty layout, which has no glyphs.
            if (paragraph.length && (label.fontInfo.family !== family
                || label.fontInfo.pixelSize !== pixelSize || label.fontInfo.weight !== weight))
                throw new Error("Qt Quick label substituted the requested family, pixel size or weight");
            const paragraphWidth = maximumWidth === null ? label.implicitWidth : label.contentWidth;
            const paragraphHeight = label.implicitHeight;
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
        return { width: width, height: height, nativeText: nativeText, paragraphs: paragraphs };
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
