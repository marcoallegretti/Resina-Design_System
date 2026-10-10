import QtQuick
import Resina.Runtime 0.1

// The composing presentation owner publishes Core accessibility and qualifies
// the complete native frame before enabling sourceAvailable. This leaf owns no
// paint, pointer/key edit lifecycle, application value or product effect.
SliderAccessibilityItem {
    id: native
    readonly property var semantics: publication.length ? JSON.parse(publication).accessibility : null
    Accessible.role: Accessible.Slider
    Accessible.ignored: semantics === null || !sourceAvailable
    Accessible.name: semantics ? semantics.name : ""
    Accessible.description: semantics && semantics.description !== null ? semantics.description : ""
    Accessible.focusable: semantics ? semantics.focusable : false
    Accessible.readOnly: semantics ? semantics.state.readOnly : true
}
