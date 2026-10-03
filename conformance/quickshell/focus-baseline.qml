import QtQuick
import QtQuick.Shapes
Shape {
    readonly property real paintOriginX: -4
    readonly property real paintOriginY: -4
    implicitWidth: 28
    implicitHeight: 22
    fillMode: Shape.NoResize
    preferredRendererType: Shape.CurveRenderer
    containsMode: Shape.FillContains
    focus: false
    activeFocusOnTab: false
    Accessible.ignored: true
    ShapePath {
        strokeColor: "transparent"
        strokeWidth: 0
        fillColor: Qt.rgba(1, 1, 1, 1)
        fillRule: ShapePath.OddEvenFill
        PathMove { x: 0; y: 4 }
        PathArc { x: 4; y: 0; radiusX: 4; radiusY: 4; xAxisRotation: 0; direction: PathArc.Clockwise; useLargeArc: false }
        PathLine { x: 24; y: 0 }
        PathArc { x: 28; y: 4; radiusX: 4; radiusY: 4; xAxisRotation: 0; direction: PathArc.Clockwise; useLargeArc: false }
        PathLine { x: 28; y: 16 }
        PathLine { x: 28; y: 18 }
        PathArc { x: 24; y: 22; radiusX: 4; radiusY: 4; xAxisRotation: 0; direction: PathArc.Clockwise; useLargeArc: false }
        PathLine { x: 4; y: 22 }
        PathArc { x: 0; y: 18; radiusX: 4; radiusY: 4; xAxisRotation: 0; direction: PathArc.Clockwise; useLargeArc: false }
        PathLine { x: 0; y: 16 }
        PathLine { x: 0; y: 4 }
        PathLine { x: 0; y: 4 }
        PathMove { x: 2; y: 4 }
        PathArc { x: 4; y: 2; radiusX: 2; radiusY: 2; xAxisRotation: 0; direction: PathArc.Clockwise; useLargeArc: false }
        PathLine { x: 24; y: 2 }
        PathArc { x: 26; y: 4; radiusX: 2; radiusY: 2; xAxisRotation: 0; direction: PathArc.Clockwise; useLargeArc: false }
        PathLine { x: 26; y: 16 }
        PathLine { x: 26; y: 18 }
        PathArc { x: 24; y: 20; radiusX: 2; radiusY: 2; xAxisRotation: 0; direction: PathArc.Clockwise; useLargeArc: false }
        PathLine { x: 4; y: 20 }
        PathArc { x: 2; y: 18; radiusX: 2; radiusY: 2; xAxisRotation: 0; direction: PathArc.Clockwise; useLargeArc: false }
        PathLine { x: 2; y: 16 }
        PathLine { x: 2; y: 4 }
        PathLine { x: 2; y: 4 }
    }
}
