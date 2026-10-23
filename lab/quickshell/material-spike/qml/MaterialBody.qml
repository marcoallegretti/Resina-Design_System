import QtQuick

ShaderEffect {
    id: root
    property real bodyWidth: 96
    property real bodyHeight: 48
    property real bodyMargin: 8
    property vector4d radii: Qt.vector4d(24, 24, 24, 24)
    property vector4d relief: Qt.vector4d(1, 2, 2, 1)
    property vector4d lighting: Qt.vector4d(-0.7071067811865475, -0.7071067811865475, 0.25, 0)
    property vector2d interaction: Qt.vector2d(0, 0)
    property color bodyColor: "white"
    property color highlightColor: "white"
    property color sideColor: "gray"
    property color edgeColor: "black"
    property vector4d shadow: Qt.vector4d(0, 6, 0, 0)
    property size itemSize: Qt.size(width, height)
    property size bodySize: Qt.size(bodyWidth, bodyHeight)

    width: bodyWidth + 2 * bodyMargin
    height: bodyHeight + 2 * bodyMargin
    property string shaderFile: "material.frag.qsb"
    fragmentShader: shaderFile

    function applyGeometry(scene) {
        bodyWidth = scene.size[0];
        bodyHeight = scene.size[1];
        bodyMargin = scene.margin;
        radii = Qt.vector4d(scene.radii[0], scene.radii[1], scene.radii[2], scene.radii[3]);
    }

    function applyAppearance(scene) {
        const rgb = value => Qt.rgba(value[0], value[1], value[2], 1);
        const sign = scene.relief.kind === "recessed" ? -1 : scene.relief.kind === "convex" ? 1 : 0;
        if (sign !== 0 && !(scene.relief.shoulder > 0))
            throw new Error("a relieved body needs a positive shoulder width: " + scene.name);
        relief = Qt.vector4d(sign * scene.relief.amplitude, (scene.relief.shoulder || 0), scene.depth, scene.edgeWidth);
        const strength = scene.interaction ? scene.interaction.strength : 0;
        lighting = Qt.vector4d(scene.key[0], scene.key[1], scene.ambient, strength);
        interaction = scene.interaction ? Qt.vector2d(scene.interaction.direction[0], scene.interaction.direction[1])
                                        : Qt.vector2d(0, 0);
        bodyColor = rgb(scene.body);
        highlightColor = rgb(scene.highlight);
        sideColor = rgb(scene.side);
        edgeColor = rgb(scene.edge);
    }

    function apply(scene) {
        applyGeometry(scene);
        applyAppearance(scene);
    }
}
