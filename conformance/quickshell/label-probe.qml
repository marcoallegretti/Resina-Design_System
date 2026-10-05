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
                    "a\rb", "a\r", "\ra", "a\u2028b", "a\u000bb", "a\u000cb", "a\u0085b"];
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
                            const explicitLines = text.split(/\r\n|[\n\r\u2028\u000b\u000c\u0085]/).length;
                            if (Math.abs(natural.height - explicitLines * size * 1.4) > 1)
                                throw new Error("explicit line breaks lost their complete height");
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
                const spaced = measurer.measure("Reconnect", font.name, 20, 400, 2, 1.4, null);
                if (spaced.width < base.width + 10)
                    throw new Error("letter spacing did not reach native shaping");
                for (const weight of [100, 600, 900])
                    measurer.measure("Reconnect", font.name, 20, weight, 0, 1.4, null);
                const literal = measurer.measure("<b>Reconnect</b>", font.name, 20, 400, 0, 1.4, null);
                if (literal.width <= base.width)
                    throw new Error("plain text was interpreted as markup");
                const failures = [
                    [" ", font.name, 20, 400, 0, 1.4, null],
                    ["a\u009cb", font.name, 20, 400, 0, 1.4, null],
                    ["a\u2029b", font.name, 20, 400, 0, 1.4, null],
                    ["a", "Missing Resina Test Family", 20, 400, 0, 1.4, null],
                    ["a", font.name, 20.5, 400, 0, 1.4, null],
                    ["a", font.name, 20, 400.5, 0, 1.4, null],
                    ["a", font.name, 20, 400, NaN, 1.4, null],
                    ["a", font.name, 20, 400, 0.01, 1.4, null],
                    ["a", font.name, 20, 400, 1e30, 1.4, null],
                    ["a", font.name, 20, 400, 0, Infinity, null],
                    ["a", font.name, 20, 400, 0, 1.4, 0],
                    ["a", font.name, 20, 400, 0, 1.4, undefined],
                    ["W", font.name, 20, 400, 0, 1.4, 0.001]
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
