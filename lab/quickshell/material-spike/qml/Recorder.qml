import QtQuick

Item {
    id: root
    property int warmup: 20
    property int frames: 300
    property real maxSeconds: 12
    property bool active: false
    property var intervals: []
    property double last: 0
    property double startedAt: 0
    signal finished()

    function start() {
        intervals = [];
        last = 0;
        startedAt = Date.now();
        active = true;
    }

    function summary(extra) {
        return JSON.stringify(Object.assign({intervals: intervals,
            seconds: (Date.now() - startedAt) / 1000,
            dpr: Window.window ? Window.window.devicePixelRatio : 0,
            api: GraphicsInfo.api}, extra || {}));
    }

    Connections {
        target: root.Window.window
        enabled: root.active
        function onFrameSwapped() {
            const now = Date.now();
            if (root.last > 0) {
                root.intervals.push(now - root.last);
            }
            root.last = now;
            const count = root.intervals.length;
            if (count >= root.warmup + root.frames || (now - root.startedAt) / 1000 > root.maxSeconds) {
                root.active = false;
                root.intervals = root.intervals.slice(root.warmup);
                root.finished();
            }
        }
    }
}
