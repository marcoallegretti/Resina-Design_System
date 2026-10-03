use resina_model::{ContourSegment, PhysicalVector};
use resina_resolver::{FocusIndicatorIr, PlacedContour};
use std::fmt::{self, Write};

const MAX_COORDINATE_ERROR: f64 = 1.0 / 1024.0;

#[derive(Debug, PartialEq)]
pub enum SlintError {
    CoordinatePrecision(f64),
}

impl fmt::Display for SlintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoordinatePrecision(value) => write!(
                formatter,
                "Slint coordinate {value} exceeds the 1/1024 logical px binary32 rounding budget"
            ),
        }
    }
}
impl std::error::Error for SlintError {}

fn check_coordinates(values: impl IntoIterator<Item = f64>) -> Result<(), SlintError> {
    for value in values {
        let rounded = f64::from(value as f32);
        if !rounded.is_finite() || (value - rounded).abs() > MAX_COORDINATE_ERROR {
            return Err(SlintError::CoordinatePrecision(value));
        }
    }
    Ok(())
}

pub fn render_focus(ir: &FocusIndicatorIr) -> Result<String, SlintError> {
    let outer = ir.geometry().outer();
    let bounds = outer.contour().bounds().expect("validated focus contour");
    let origin = PhysicalVector {
        x: bounds.x + outer.offset().x,
        y: bounds.y + outer.offset().y,
    };
    check_coordinates([origin.x, origin.y, bounds.width, bounds.height])?;
    let [red, green, blue] = ir
        .indicator()
        .color()
        .components()
        .map(|channel| (channel * 255.0).round() as u8);
    let mut output = format!(
        "export component ResinaFocusIndicator inherits Rectangle {{\n    out property <length> paint-origin-x: {}px;\n    out property <length> paint-origin-y: {}px;\n    width: {}px;\n    height: {}px;\n    background: transparent;\n    clip: false;\n    accessible-role: none;\n    Path {{\n        width: root.width;\n        height: root.height;\n        viewbox-x: 0;\n        viewbox-y: 0;\n        viewbox-width: {};\n        viewbox-height: {};\n        fit: preserve;\n        fill: rgb({red}, {green}, {blue});\n        fill-rule: nonzero;\n        stroke: transparent;\n        stroke-width: 0px;\n        accessible-role: none;\n",
        origin.x, origin.y, bounds.width, bounds.height, bounds.width, bounds.height,
    );
    append_contour(&mut output, outer, origin, false)?;
    // Opposite winding preserves the hole even in Slint's software renderer,
    // which uses nonzero fill independently of the Path fill-rule property.
    append_contour(&mut output, ir.geometry().inner(), origin, true)?;
    output.push_str("    }\n}\n");
    Ok(output)
}

fn endpoint(
    segment: &ContourSegment,
    first: bool,
    placed: &PlacedContour,
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
        x: point.x + placed.offset().x - origin.x,
        y: point.y + placed.offset().y - origin.y,
    }
}

fn append_contour(
    output: &mut String,
    placed: &PlacedContour,
    origin: PhysicalVector,
    reverse: bool,
) -> Result<(), SlintError> {
    let segments = placed.contour().segments();
    let first_segment = if reverse {
        segments.last()
    } else {
        segments.first()
    }
    .expect("validated nonempty contour");
    let first = endpoint(first_segment, !reverse, placed, origin);
    check_coordinates([first.x, first.y])?;
    writeln!(
        output,
        "        MoveTo {{ x: {}; y: {}; }}",
        first.x, first.y
    )
    .unwrap();
    for index in 0..segments.len() {
        let segment = &segments[if reverse {
            segments.len() - 1 - index
        } else {
            index
        }];
        let end = endpoint(segment, reverse, placed, origin);
        check_coordinates([end.x, end.y])?;
        match segment {
            ContourSegment::Line { .. } => {
                writeln!(output, "        LineTo {{ x: {}; y: {}; }}", end.x, end.y).unwrap();
            }
            ContourSegment::Arc { radii, .. } => {
                check_coordinates([radii.x, radii.y])?;
                writeln!(output, "        ArcTo {{ x: {}; y: {}; radius-x: {}; radius-y: {}; x-rotation: 0; sweep: {}; large-arc: false; }}",
                    end.x, end.y, radii.x, radii.y, !reverse).unwrap();
            }
        }
    }
    output.push_str("        Close {}\n");
    Ok(())
}
