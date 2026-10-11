import QtQuick
import QtQuick.Window

Window {
    width: 320
    height: 200
    visible: true
    title: "Resina Qt runtime accessibility probe"

    CommandRuntimeFixture {
        anchors.fill: parent
    }
}
