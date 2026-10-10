import QtQuick
import "../../backends/quickshell/resina-qml/qml" as Content

Window {
    id: window
    width: 500; height: 400; visible: true
    property string configuration: ""
    property bool disabledFocusable: false
    property int effects: 0
    property int readinessViolations: 0
    property bool revoke: false
    property bool revokeOnReady: false
    property var publicationRevocation: null
    property bool logEvents: false
    Item {
        id: carrier
        anchors.fill: parent
        Content.ResinaToggleControl {
            id: command
            objectName: "toggleControl"
            x: 50; y: 50
            configuration: window.configuration
            disabledFocusable: window.disabledFocusable
            samplesPerAxis: 4
            onReadyChanged: {
                if (ready && (!acceptsNewGestures || !pointerOwner.acceptsNewGestures)) ++window.readinessViolations;
            }
            onSnapshotChanged: {
                if (window.revokeOnReady && snapshot.length) {
                    window.revokeOnReady = false;
                    window.publicationRevocation = availability(false);
                }
            }
            onActivated: (checked) => {
                if (!JSON.parse(togglePublication).activation.state.enabled || !JSON.parse(togglePublication).activation.activate || checked !== JSON.parse(togglePublication).checked)
                    throw new Error("product effect preceded committed activation");
                ++window.effects;
                if (window.revoke) availability(false);
                if (window.logEvents)
                    console.log("RESINA_CONTROL_EFFECT " + observation());
            }
        }
        Item { id: other; width: 1; height: 1 }
    }
    function availability(value) { return command.controller.dispatch(JSON.stringify({kind:"availability",enabled:value})); }
    function setChecked(value) { return command.setChecked(value); }
    function invoke() { return command.invoke(); }
    function focusCommand() { command.forceActiveFocus(); }
    function focusOther() { other.forceActiveFocus(); }
    function rotateCarrier(value) { carrier.rotation = value; }
    function moveCommand(value) { command.x = value; }
    function showCommand(value) { command.visible = value; }
    function renew() { command.reconfigure(); }
    function observation() {
        return JSON.stringify({ready:command.ready,error:command.preparationError,snapshot:command.snapshot,
            checked:command.checked,
            target:command.targetPublication,state:JSON.parse(command.publication).state,
            hovered:command.hovered,effects:effects,focused:command.activeFocus,
            readinessViolations:readinessViolations,
            publicationRevocation:publicationRevocation,
            width:command.width,height:command.height});
    }
}
