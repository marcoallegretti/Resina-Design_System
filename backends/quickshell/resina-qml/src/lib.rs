use resina_model::{ContourSegment, PhysicalVector};
use resina_resolver::{FocusIndicatorIr, PlacedContour};
use std::fmt::{self, Write};

const MAX_COORDINATE_ERROR: f64 = 1.0 / 1024.0;

#[derive(Debug, PartialEq)]
pub enum QmlError {
    CoordinatePrecision(f64),
}

impl fmt::Display for QmlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoordinatePrecision(value) => write!(
                formatter,
                "Qt Quick coordinate {value} exceeds the 1/1024 logical px binary32 rounding budget"
            ),
        }
    }
}

impl std::error::Error for QmlError {}

fn check_coordinates(values: impl IntoIterator<Item = f64>) -> Result<(), QmlError> {
    for value in values {
        let rounded = f64::from(value as f32);
        if !rounded.is_finite() || (value - rounded).abs() > MAX_COORDINATE_ERROR {
            return Err(QmlError::CoordinatePrecision(value));
        }
    }
    Ok(())
}

pub fn render_focus(ir: &FocusIndicatorIr) -> Result<String, QmlError> {
    let outer = ir.geometry().outer();
    let bounds = outer.contour().bounds().expect("validated focus contour");
    let origin = PhysicalVector {
        x: bounds.x + outer.offset().x,
        y: bounds.y + outer.offset().y,
    };
    check_coordinates([origin.x, origin.y, bounds.width, bounds.height])?;
    let [red, green, blue] = ir.indicator().color().components();
    let mut output = format!(
        "import QtQuick\nimport QtQuick.Shapes\n\nShape {{\n    readonly property real paintOriginX: {}\n    readonly property real paintOriginY: {}\n    implicitWidth: {}\n    implicitHeight: {}\n    fillMode: Shape.NoResize\n    preferredRendererType: Shape.CurveRenderer\n    containsMode: Shape.FillContains\n    focus: false\n    activeFocusOnTab: false\n    Accessible.ignored: true\n    ShapePath {{\n        strokeColor: \"transparent\"\n        strokeWidth: 0\n        fillColor: Qt.rgba({red}, {green}, {blue}, 1)\n        fillRule: ShapePath.OddEvenFill\n",
        origin.x, origin.y, bounds.width, bounds.height,
    );
    append_contour(&mut output, outer, origin)?;
    append_contour(&mut output, ir.geometry().inner(), origin)?;
    output.push_str("    }\n}\n");
    Ok(output)
}

fn local_endpoint(
    segment: &ContourSegment,
    first: bool,
    offset: PhysicalVector,
    origin: PhysicalVector,
) -> PhysicalVector {
    let point = match segment {
        ContourSegment::Line { from, to } => {
            if first {
                *from
            } else {
                *to
            }
        }
        ContourSegment::Arc {
            center,
            radii,
            start,
            end,
        } => {
            let radial = if first { start } else { end };
            PhysicalVector {
                x: center.x + radii.x * radial.x,
                y: center.y + radii.y * radial.y,
            }
        }
    };
    PhysicalVector {
        x: point.x + offset.x - origin.x,
        y: point.y + offset.y - origin.y,
    }
}

fn append_contour(
    output: &mut String,
    placed: &PlacedContour,
    origin: PhysicalVector,
) -> Result<(), QmlError> {
    let segments = placed.contour().segments();
    let first = local_endpoint(
        segments.first().expect("validated nonempty contour"),
        true,
        placed.offset(),
        origin,
    );
    check_coordinates([first.x, first.y])?;
    writeln!(
        output,
        "        PathMove {{ x: {}; y: {} }}",
        first.x, first.y
    )
    .unwrap();
    for segment in segments {
        let end = local_endpoint(segment, false, placed.offset(), origin);
        check_coordinates([end.x, end.y])?;
        match segment {
            ContourSegment::Line { .. } => {
                writeln!(output, "        PathLine {{ x: {}; y: {} }}", end.x, end.y).unwrap();
            }
            ContourSegment::Arc { radii, .. } => {
                check_coordinates([radii.x, radii.y])?;
                writeln!(output, "        PathArc {{ x: {}; y: {}; radiusX: {}; radiusY: {}; xAxisRotation: 0; direction: PathArc.Clockwise; useLargeArc: false }}", end.x, end.y, radii.x, radii.y).unwrap();
            }
        }
    }
    writeln!(
        output,
        "        PathLine {{ x: {}; y: {} }}",
        first.x, first.y
    )
    .unwrap();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_budget_rejects_precision_loss_and_overflow() {
        check_coordinates([0.1, -0.1, 32768.0 + 1.0 / 1024.0]).unwrap();
        let coordinate = 32768.0 + 9.0 / 8192.0;
        assert_eq!(
            check_coordinates([coordinate]),
            Err(QmlError::CoordinatePrecision(coordinate))
        );
        assert!(check_coordinates([f64::MAX]).is_err());
    }
}
