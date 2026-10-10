import QtQuick
import Resina.Runtime 0.1

Item {
    id: root
    readonly property string layout: state.layout
    readonly property bool paintReady: layout.length > 0 && rendered.paintReady
    readonly property string paintError: state.error
    width: state.label ? state.label.size.width : 0
    height: state.label ? state.label.size.height : 0
    visible: paintReady
    enabled: false
    clip: false
    focus: false
    activeFocusOnTab: false
    Accessible.ignored: true

    function prepare(source, color) {
        if (state.preparing)
            throw new Error("Qt command label preparation is in progress");
        state.preparing = true;
        state.layout = "";
        state.label = null;
        state.error = "";
        try {
            const result = JSON.parse(resolver.resolveLabel(source, measurement));
            if (result.error)
                throw new Error(result.error);
            const label = result.label;
            const family = JSON.parse(source).fontFamily.name;
            rendered.prepare({text:label.text, family:family,
                pixelSize:label.typography.fontSize.value, weight:label.typography.fontWeight,
                spacing:label.typography.letterSpacing.value, lineHeight:label.typography.lineHeight,
                wrappingWidth:label.labelBounds.width, height:label.labelBounds.height,
                x:label.labelBounds.x, y:label.labelBounds.y, color:color});
            state.label = label;
            state.layout = JSON.stringify(label);
            state.error = "";
        } catch (error) {
            state.label = null;
            state.layout = "";
            state.error = error.message;
            throw error;
        } finally {
            state.preparing = false;
        }
    }

    QtObject {
        id: state
        property var label: null
        property string layout: ""
        property string error: ""
        property bool preparing: false
    }
    CommandLabelResolver { id: resolver }
    ResinaLabelMeasure { id: measurement }
    ResinaLabel { id: rendered }
}
