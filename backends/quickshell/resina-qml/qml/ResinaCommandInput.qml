import QtQuick
import Resina.Runtime 0.1

ResinaCommandKeyboard {
    id: root
    property alias coordinateFrame: pointer.coordinateFrame
    readonly property CommandPointerItem pointerOwner: pointer
    readonly property string targetPublication: pointer ? pointer.targetPublication : ""
    readonly property string pointerError: pointer ? pointer.lastError : "native pointer owner was removed"

    function prepareTarget(request) { return pointer ? pointer.prepareTarget(request) : false; }
    function invalidateTarget() { if (pointer) pointer.invalidate(); }

    CommandPointerItem {
        id: pointer
        anchors.fill: parent
        controller: root.controller
        Accessible.ignored: true
    }
}
