import QtQuick
import Resina.Runtime 0.1

ResinaToggleKeyboard {
    id: root
    property alias coordinateFrame: pointer.coordinateFrame
    readonly property CommandPointerItem pointerOwner: pointer
    // The native property is read-only; aliasing avoids a reentrant Boolean binding.
    property alias hovered: pointer.hovered
    readonly property string targetPublication: pointer ? pointer.targetPublication : ""
    readonly property string pointerError: pointer ? pointer.lastError : "native pointer owner was removed"

    function prepareTarget(request) { return pointer ? pointer.prepareTarget(request) : false; }
    function invalidateTarget() { if (pointer) pointer.invalidate(); }

    CommandPointerItem {
        id: pointer
        anchors.fill: parent
        controller: root.controller
        acceptsNewGestures: root.acceptsNewGestures
        Accessible.ignored: true
    }
}
