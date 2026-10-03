use resina_model::{ContourSegment, PhysicalVector};
use resina_resolver::{FocusIndicatorIr, PlacedContour};
use std::fmt::{self, Write};

const MAX_COORDINATE_ERROR: f64 = 1.0 / 1024.0;

#[derive(Debug, PartialEq)]
pub enum SvgError {
    CoordinatePrecision(f64),
}

impl fmt::Display for SvgError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoordinatePrecision(value) => write!(
                formatter,
                "SVG coordinate {value} exceeds the 1/1024 logical px binary32 rounding budget"
            ),
        }
    }
}

impl std::error::Error for SvgError {}

fn check_coordinates(values: impl IntoIterator<Item = f64>) -> Result<(), SvgError> {
    for value in values {
        let rounded = f64::from(value as f32);
        if !rounded.is_finite() || (value - rounded).abs() > MAX_COORDINATE_ERROR {
            return Err(SvgError::CoordinatePrecision(value));
        }
    }
    Ok(())
}

pub fn render_focus(ir: &FocusIndicatorIr) -> Result<String, SvgError> {
    let outer = ir.geometry().outer();
    let bounds = outer.contour().bounds().expect("validated focus contour");
    let origin = translated(
        PhysicalVector {
            x: bounds.x,
            y: bounds.y,
        },
        outer.offset(),
    );
    let [red, green, blue] = ir
        .indicator()
        .color()
        .components()
        .map(|value| value * 100.0);
    check_coordinates([origin.x, origin.y, bounds.width, bounds.height])?;
    let mut output = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\" aria-hidden=\"true\" focusable=\"false\"><path fill=\"rgb({red}% {green}% {blue}%)\" fill-rule=\"evenodd\" d=\"",
        bounds.width, bounds.height, origin.x, origin.y, bounds.width, bounds.height,
    );
    append_contour(&mut output, outer)?;
    output.push(' ');
    append_contour(&mut output, ir.geometry().inner())?;
    output.push_str("\"/></svg>\n");
    Ok(output)
}

fn translated(point: PhysicalVector, offset: PhysicalVector) -> PhysicalVector {
    PhysicalVector {
        x: point.x + offset.x,
        y: point.y + offset.y,
    }
}

fn endpoint(segment: &ContourSegment, first: bool) -> PhysicalVector {
    match segment {
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
    }
}

fn append_contour(output: &mut String, placed: &PlacedContour) -> Result<(), SvgError> {
    let segments = placed.contour().segments();
    let first = translated(
        endpoint(segments.first().expect("validated nonempty contour"), true),
        placed.offset(),
    );
    write!(output, "M {} {}", first.x, first.y).unwrap();
    check_coordinates([first.x, first.y])?;
    for segment in segments {
        let end = translated(endpoint(segment, false), placed.offset());
        check_coordinates([end.x, end.y])?;
        match segment {
            ContourSegment::Line { .. } => write!(output, " L {} {}", end.x, end.y).unwrap(),
            ContourSegment::Arc { radii, .. } => {
                check_coordinates([radii.x, radii.y])?;
                write!(
                    output,
                    " A {} {} 0 0 1 {} {}",
                    radii.x, radii.y, end.x, end.y
                )
                .unwrap();
            }
        }
    }
    output.push_str(" Z");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_budget_accepts_ordinary_fractions_and_its_exact_boundary() {
        check_coordinates([0.1, -0.1, 32768.0 + 1.0 / 1024.0]).unwrap();
        let coordinate = 32768.0 + 9.0 / 8192.0;
        assert_eq!(
            check_coordinates([coordinate]),
            Err(SvgError::CoordinatePrecision(coordinate))
        );
        assert!(matches!(
            check_coordinates([f64::MAX]),
            Err(SvgError::CoordinatePrecision(_))
        ));
    }
}
