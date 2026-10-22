import QtQuick
import Quickshell

Item {
    id: oracle
    visible: false
    readonly property var scripts: JSON.parse(Quickshell.env("RESINA_LABEL_SCRIPT_FACES") || "[]")

    Text { id: fallback; textFormat: Text.PlainText; renderType: Text.QtRendering; font.pixelSize: 20 }
    Text { id: pinned; textFormat: Text.PlainText; renderType: Text.QtRendering; font.pixelSize: 20 }

    // A script sample has no glyph in the primary family, so it is drawn with the fallback face. That
    // face is the pinned master only when the same sample drawn directly with the master has the same advance.
    function resolve(primaryFamily, weights) {
        if (!scripts.length)
            throw new Error("the pinned script faces must be provided");
        const faces = [];
        for (const script of scripts) {
            for (const weight of weights) {
                fallback.font.family = primaryFamily;
                fallback.font.weight = weight;
                fallback.text = script.sample;
                pinned.font.family = script.family;
                pinned.font.weight = weight;
                pinned.text = script.sample;
                fallback.forceLayout();
                pinned.forceLayout();
                if (pinned.fontInfo.family !== script.family)
                    throw new Error(script.script + " master " + script.family + " is not registered");
                if (fallback.implicitWidth !== pinned.implicitWidth)
                    throw new Error(script.script + " text at weight " + weight + " was not drawn with the pinned "
                        + pinned.fontInfo.family + " " + pinned.fontInfo.styleName + " master");
                faces.push({ script: script.script, weight: weight,
                    face: { family: pinned.fontInfo.family, style: pinned.fontInfo.styleName } });
            }
        }
        return faces;
    }
}
