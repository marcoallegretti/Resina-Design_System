import QtQuick
import QtQuick.Window

Window {
    width: 320
    height: 200
    visible: true
    title: "Resina Qt accessibility probe"

    Repeater {
        model: [
            {name: "Enabled command", available: true, focusable: true},
            {name: "Disabled command", available: false, focusable: true},
            {name: "Excluded command", available: false, focusable: false}
        ]
        delegate: Item {
            required property var modelData
            required property int index
            x: 10
            y: 10 + index * 50
            width: 100
            height: 40
            enabled: modelData.available
            activeFocusOnTab: modelData.focusable
            Accessible.role: Accessible.Button
            Accessible.name: modelData.name
            Accessible.description: "Native availability conformance fixture."
            Accessible.focusable: modelData.focusable
            Accessible.focused: activeFocus

            // Qt's role already advertises Press; an attached signal adds a duplicate.
            function accessiblePressAction() {
                const result = JSON.parse(resinaLifecycle.invoke(modelData.name, enabled));
                enabled = result.committedState.enabled;
                console.log("RESINA_QT_ACTION " + JSON.stringify(result));
            }
        }
    }
}
