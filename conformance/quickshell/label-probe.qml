import QtQuick
import Quickshell

FloatingWindow {
    id: window
    implicitWidth: 400
    implicitHeight: 240
    color: "white"

    FontLoader {
        id: font
        source: Quickshell.env("RESINA_LABEL_FONT")
    }
    Loader {
        id: measurement
        source: Quickshell.env("RESINA_LABEL_MEASURE_QML")
    }
    Text {
        id: paragraphOracle
        visible: false
        font.family: font.name
        font.pixelSize: 20
        font.weight: 400
        textFormat: Text.PlainText
        renderType: Text.QtRendering
        lineHeight: 28
        lineHeightMode: Text.FixedHeight
    }

    Timer {
        interval: 100
        running: true
        onTriggered: {
            try {
                if (font.status !== FontLoader.Ready || font.name !== "DejaVu Sans")
                    throw new Error("the explicit DejaVu Sans font must be loaded first");
                if (measurement.status !== Loader.Ready)
                    throw new Error("the native measurement component must be loaded");
                const measurer = measurement.item;
                const expectedScale = Number(Quickshell.env("RESINA_LABEL_DEVICE_SCALE"));
                if (!Number.isFinite(expectedScale) || expectedScale <= 0
                    || Math.abs(window.screen.devicePixelRatio - expectedScale) > 0.00001)
                    throw new Error("actual window device scale differs from the requested scale");
                if (measurer.visible || measurer.focus || measurer.activeFocusOnTab
                    || !measurer.Accessible.ignored)
                    throw new Error("measurement must remain invisible and decorative");
                const texts = ["Reconnect", "Erneut mit dem Netzwerk verbinden", "مرحبا بالعالم",
                    "Reconnect now", "Reconnect to Wi-Fi", "<b>Reconnect</b>",
                    "a\nb", "a\n", "\na", "a\n\nb", "a\r\nb", "a\r\n",
                    "a\rb", "a\r", "\ra", "a\u2028b", "a\u000bb", "a\u000cb", "a\u0085b",
                    "a\u2029b", "a\u2029", "\u2029a", "a\u2029\u2029b", "a\n\u2029b",
                    "مرحبا بالعالم\u2029Reconnect", "Reconnect\u2029مرحبا بالعالم",
                    "مرحبا بالعالم\u2029! 123\u2029Reconnect"];
                let checked = 0;
                for (const scale of [1, 1.5, 2]) {
                    const size = 20 * scale;
                    for (const text of texts) {
                        for (const spacing of [-0.25, 0, 0.5]) {
                            const natural = measurer.measure(text, font.name, size, 400, spacing, 1.4, null);
                            const offered = Math.min(natural.width, 100 * scale);
                            const fitted = measurer.measure(text, font.name, size, 400, spacing, 1.4, offered);
                            if (fitted.width > offered || fitted.height < natural.height)
                                throw new Error("complete reflow extent is inconsistent");
                            if (fitted.nativeText !== natural.nativeText)
                                throw new Error("natural and fitted shaping text differ");
                            const explicitLines = text.split(/\r\n|[\n\r\u2028\u2029\u000b\u000c\u0085]/).length;
                            if (Math.abs(natural.height - explicitLines * size * 1.4) > 1)
                                throw new Error("explicit line breaks lost their complete height");
                            let paragraphHeight = 0;
                            let paragraphWidth = 0;
                            const expectedParagraphs = natural.nativeText.split("\u2029");
                            if (fitted.paragraphs.length !== expectedParagraphs.length)
                                throw new Error("independent paragraphs were not preserved");
                            for (let index = 0; index < fitted.paragraphs.length; index++) {
                                const paragraph = fitted.paragraphs[index];
                                if (paragraph.nativeText !== expectedParagraphs[index]
                                    || paragraph.y !== paragraphHeight)
                                    throw new Error("paragraph content or placement changed");
                                if (paragraph.nativeText.trim().length) {
                                    const alone = measurer.measure(paragraph.nativeText, font.name, size, 400, spacing, 1.4, offered);
                                    if (alone.width !== paragraph.width || alone.height !== paragraph.height)
                                        throw new Error("neighboring paragraphs changed native shaping");
                                }
                                paragraphHeight += paragraph.height;
                                paragraphWidth = Math.max(paragraphWidth, paragraph.width);
                            }
                            if (fitted.height !== paragraphHeight || fitted.width !== paragraphWidth)
                                throw new Error("complete extent differs from the independent paragraphs");
                            checked++;
                        }
                    }
                }
                const word = "Supercalifragilisticexpialidocious";
                const natural = measurer.measure(word, font.name, 20, 400, 0, 1.4, null);
                const fitted = measurer.measure(word, font.name, 20, 400, 0, 1.4, 60);
                if (fitted.width > 60 || fitted.height <= natural.height * 2)
                    throw new Error("a wide word did not reflow completely");
                const base = measurer.measure("Reconnect", font.name, 20, 400, 0, 1.4, null);
                const mixed = measurer.measure("مرحبا بالعالم\u2029! 123\u2029Reconnect", font.name, 20, 400, 0, 1.4, null);
                for (let index = 0; index < mixed.paragraphs.length; index++) {
                    const paragraph = mixed.paragraphs[index];
                    paragraphOracle.text = paragraph.nativeText;
                    paragraphOracle.forceLayout();
                    if (paragraphOracle.implicitWidth !== paragraph.width
                        || paragraphOracle.implicitHeight !== paragraph.height
                        || paragraphOracle.effectiveHorizontalAlignment
                            !== (index === 0 ? Text.AlignRight : Text.AlignLeft))
                        throw new Error("independent native paragraph metrics or base direction differ: "
                            + JSON.stringify({ index: index, paragraph: paragraph,
                                width: paragraphOracle.implicitWidth, height: paragraphOracle.implicitHeight,
                                alignment: paragraphOracle.effectiveHorizontalAlignment }));
                }
                const spaced = measurer.measure("Reconnect", font.name, 20, 400, 2, 1.4, null);
                if (spaced.width < base.width + 10)
                    throw new Error("letter spacing did not reach native shaping");
                for (const weight of [100, 600, 900])
                    measurer.measure("\u2029Reconnect\u2029", font.name, 20, weight, 0, 1.4, null);
                const fractional = measurer.measure("a\u2029\u2029b", font.name, 20, 400, 0, 1.405, null);
                for (let index = 0; index < fractional.paragraphs.length; index++) {
                    if (Math.abs(fractional.paragraphs[index].height - 28.1) > 1e-9
                        || Math.abs(fractional.paragraphs[index].y - index * 28.1) > 1e-9)
                        throw new Error("fractional paragraph height or offset was rounded");
                }
                if (Math.abs(fractional.height - 84.3) > 1e-9)
                    throw new Error("fractional complete paragraph height was lost");
                const literal = measurer.measure("<b>Reconnect</b>", font.name, 20, 400, 0, 1.4, null);
                if (literal.width <= base.width)
                    throw new Error("plain text was interpreted as markup");
                const firstParagraphWidth = measurer.measure("a", font.name, 20, 400, 0, 1.4, null).width;
                const failures = [
                    [" ", font.name, 20, 400, 0, 1.4, null],
                    ["a\u009cb", font.name, 20, 400, 0, 1.4, null],
                    ["a", "Missing Resina Test Family", 20, 400, 0, 1.4, null],
                    ["a", font.name, 20.5, 400, 0, 1.4, null],
                    ["a", font.name, 20, 400.5, 0, 1.4, null],
                    ["a", font.name, 20, 400, NaN, 1.4, null],
                    ["a", font.name, 20, 400, 0.01, 1.4, null],
                    ["a", font.name, 20, 400, 1e30, 1.4, null],
                    ["a", font.name, 20, 400, 0, Infinity, null],
                    ["a", font.name, 20, 400, 0, 1.4, 0],
                    ["a", font.name, 20, 400, 0, 1.4, undefined],
                    ["W", font.name, 20, 400, 0, 1.4, 0.001],
                    ["a\u2029W", font.name, 20, 400, 0, 1.4, firstParagraphWidth]
                ];
                for (const request of failures) {
                    let rejected = false;
                    try { measurer.measure(...request); }
                    catch (error) { rejected = true; }
                    if (!rejected)
                        throw new Error("invalid or unsupported native measurement succeeded");
                }
                const recovered = measurer.measure("Reconnect", font.name, 20, 400, 0, 1.4, null);
                if (recovered.width !== base.width || recovered.height !== base.height)
                    throw new Error("a failed request contaminated the next complete measurement");
                console.log("RESINA_LABEL_PASS", checked, failures.length, window.screen.devicePixelRatio);
            } catch (error) {
                console.error("RESINA_LABEL_FAIL", error.message);
            }
            Qt.quit();
        }
    }
}
