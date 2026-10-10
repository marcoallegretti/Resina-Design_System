import QtQuick
import Resina.Runtime

Item {
    Repeater {
        model: [
            {name: "Enabled command", available: true, focusable: true},
            {name: "Disabled command", available: false, focusable: true},
            {name: "Excluded command", available: false, focusable: false}
        ]
        delegate: Item {
            id: command
            required property var modelData
            required property int index
            property int activations: 0
            property var invokeResult: null
            property var availabilityResult: null
            property bool delivering: false
            readonly property var current: JSON.parse(controller.publication)
            x: 10
            y: 10 + index * 50
            width: 100
            height: 40
            enabled: current.state.enabled
            activeFocusOnTab: modelData.focusable
            Accessible.role: Accessible.Button
            Accessible.name: modelData.name
            Accessible.description: "Native availability conformance fixture."
            Accessible.focusable: modelData.focusable
            Accessible.focused: activeFocus

            Component.onCompleted: {
                if (!controller.dispatch(JSON.stringify({kind: "availability", enabled: modelData.available})))
                    throw new Error(controller.lastError);
            }

            CommandController {
                id: controller
                onPublicationChanged: {
                    if (command.delivering && command.invokeResult === null)
                        command.invokeResult = JSON.parse(publication);
                }
                onActivated: {
                    const committed = JSON.parse(publication);
                    if (!committed.state.enabled || committed.state.hold !== null)
                        throw new Error("product intent arrived before state commit");
                    ++command.activations;
                    if (!dispatch('{"kind":"availability","enabled":false}'))
                        throw new Error(lastError);
                    command.availabilityResult = JSON.parse(publication);
                }
            }

            function accessiblePressAction() {
                const available = enabled;
                invokeResult = null;
                availabilityResult = null;
                delivering = true;
                const accepted = controller.dispatch('{"kind":"invoke"}');
                delivering = false;
                if (!accepted)
                    throw new Error(controller.lastError);
                console.log("RESINA_QT_ACTION " + JSON.stringify({
                    name: modelData.name, available: available, activations: activations,
                    invokeResult: invokeResult, availabilityResult: availabilityResult,
                    committedState: JSON.parse(controller.publication).state
                }));
            }
        }
    }
}
