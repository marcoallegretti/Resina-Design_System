import Quickshell
import QtQuick
import QtQuick.Window

FloatingWindow {
    implicitWidth: 320
    implicitHeight: 200
    title: "Resina Quickshell runtime accessibility probe"

    CommandRuntimeFixture {
        anchors.fill: parent
    }
}
