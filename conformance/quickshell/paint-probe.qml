import QtQuick
import QtQuick.Window
import Quickshell

FloatingWindow {
    implicitWidth: 320
    implicitHeight: 160
    color: "transparent"

    Item {
        id: capture
        width: 320
        height: 160

        Loader {
            id: paint
            source: Quickshell.env("RESINA_PAINT_QML")
            x: item ? item.paintOriginX : 0
            y: item ? item.paintOriginY : 0
        }

        Timer {
            interval: 300
            running: true
            onTriggered: {
                if (paint.status !== Loader.Ready || !paint.item.paintReady
                    || paint.item.preparedDeviceScale !== capture.Screen.devicePixelRatio
                    || paint.item.focus || paint.item.activeFocusOnTab
                    || !paint.item.Accessible.ignored) {
                    console.error("RESINA_PAINT_FAIL: component profile or device scale");
                    Qt.quit();
                    return;
                }
                if (!capture.grabToImage(function(result) {
                    if (result.saveToFile(Quickshell.env("RESINA_PAINT_IMAGE")))
                        console.log("RESINA_PAINT_PASS");
                    else
                        console.error("RESINA_PAINT_FAIL: image save");
                    Qt.quit();
                })) {
                    console.error("RESINA_PAINT_FAIL: image capture");
                    Qt.quit();
                }
            }
        }
    }
}
