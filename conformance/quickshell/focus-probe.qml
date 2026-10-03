import QtQuick
import QtQuick.Shapes
import Quickshell

FloatingWindow {
    implicitWidth: 40
    implicitHeight: 32
    color: "transparent"

    Item {
        id: capture
        width: 40
        height: 32

        Loader {
            id: ring
            source: Quickshell.env("RESINA_FOCUS_QML")
            x: item ? 8 + item.paintOriginX : 0
            y: item ? 8 + item.paintOriginY : 0
        }

        Timer {
            interval: 300
            running: true
            onTriggered: {
                if (ring.status !== Loader.Ready || ring.item.paintOriginX !== -4
                    || ring.item.paintOriginY !== -4 || ring.item.implicitWidth !== 28
                    || ring.item.implicitHeight !== 22 || ring.item.focus
                    || ring.item.activeFocusOnTab || !ring.item.Accessible.ignored) {
                    console.error("RESINA_FOCUS_FAIL: component profile");
                    Qt.quit();
                    return;
                }
                let checked = 0;
                for (let y = -5.25; y < 20; y += 0.5) {
                    for (let x = -5.25; x < 26; x += 0.5) {
                        const distance = Math.hypot(Math.max(0, -x, x - 20), Math.max(0, -y, y - 14));
                        if (Math.abs(distance - 2) < 0.05 || Math.abs(distance - 4) < 0.05)
                            continue;
                        const expected = distance <= 4 && distance > 2;
                        const actual = ring.item.contains(Qt.point(x - ring.item.paintOriginX, y - ring.item.paintOriginY));
                        if (actual !== expected) {
                            console.error("RESINA_FOCUS_FAIL: containment", x, y);
                            Qt.quit();
                            return;
                        }
                        checked++;
                    }
                }
                if (!capture.grabToImage(function(result) {
                    if (result.saveToFile(Quickshell.env("RESINA_FOCUS_IMAGE")))
                        console.log("RESINA_FOCUS_PASS", checked, ring.item.rendererType);
                    else
                        console.error("RESINA_FOCUS_FAIL: image save");
                    Qt.quit();
                }, Qt.size(160, 128))) {
                    console.error("RESINA_FOCUS_FAIL: image capture");
                    Qt.quit();
                }
            }
        }
    }
}
