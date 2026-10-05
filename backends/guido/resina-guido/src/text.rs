use crate::MAX_COORDINATE_ERROR;
use guido::{
    renderer::{DrawCommand, measure_text_styled},
    widgets::font::{FontFamily, FontWeight, LineHeight},
    widgets::{Color, Rect, TextAlign},
};
use resina_model::{PhysicalVector, SurfaceSize};
use resina_resolver::{CommandLabelIr, LabelMeasureInput, ResolvedTypography};
use std::fmt;

#[derive(Debug, PartialEq)]
pub enum LabelMeasureError {
    LetterSpacing,
    FontWeight,
    Precision(&'static str),
    Extent,
}
impl fmt::Display for LabelMeasureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LetterSpacing => f.write_str("pinned GUIdo text does not support letter spacing"),
            Self::FontWeight => f.write_str("GUIdo font weight must be an integer"),
            Self::Precision(field) => write!(f, "GUIdo label {field} exceeds coordinate precision"),
            Self::Extent => f.write_str("GUIdo label measurement is empty or nonfinite"),
        }
    }
}
impl std::error::Error for LabelMeasureError {}
fn scalar(value: f64, name: &'static str) -> Result<f32, LabelMeasureError> {
    let native = value as f32;
    if !native.is_finite()
        || native <= 0.0
        || (f64::from(native) - value).abs() > MAX_COORDINATE_ERROR
    {
        Err(LabelMeasureError::Precision(name))
    } else {
        Ok(native)
    }
}
struct NativeLabelStyle {
    size: f32,
    weight: FontWeight,
    line: LineHeight,
}
fn native_style(style: &ResolvedTypography) -> Result<NativeLabelStyle, LabelMeasureError> {
    if style.letter_spacing() != 0.0 {
        return Err(LabelMeasureError::LetterSpacing);
    }
    if style.font_weight().fract() != 0.0 {
        return Err(LabelMeasureError::FontWeight);
    }
    let size = scalar(style.font_size(), "font size")?;
    if size < 0.01 {
        return Err(LabelMeasureError::Precision("font size"));
    }
    let line = scalar(style.line_height(), "line height")?;
    let native_height = size * line;
    let declared_height = style.font_size() * style.line_height();
    if !native_height.is_finite()
        || native_height <= 0.0
        || !declared_height.is_finite()
        || (f64::from(native_height) - declared_height).abs() > MAX_COORDINATE_ERROR
    {
        return Err(LabelMeasureError::Precision("resolved line height"));
    }
    Ok(NativeLabelStyle {
        size,
        weight: FontWeight(style.font_weight() as u16),
        line: LineHeight::Relative(line),
    })
}
fn wrap_width(value: f64) -> Result<f32, LabelMeasureError> {
    let mut native = scalar(value, "wrap width")?;
    if f64::from(native) > value {
        native = native.next_down();
    }
    if native <= 0.0 || (value - f64::from(native)).abs() > MAX_COORDINATE_ERROR {
        return Err(LabelMeasureError::Precision("wrap width"));
    }
    Ok(native)
}
pub fn measure_command_label(
    family: FontFamily,
    input: LabelMeasureInput<'_>,
) -> Result<SurfaceSize, LabelMeasureError> {
    let style = native_style(input.typography)?;
    let width = input.maximum_width.map(wrap_width).transpose()?;
    let measured = measure_text_styled(
        input.text,
        style.size,
        width,
        family,
        style.weight,
        style.line,
    );
    if !measured.width.is_finite()
        || measured.width <= 0.0
        || !measured.height.is_finite()
        || measured.height <= 0.0
    {
        return Err(LabelMeasureError::Extent);
    }
    Ok(SurfaceSize {
        width: f64::from(measured.width),
        height: f64::from(measured.height),
    })
}

#[derive(Debug, PartialEq)]
pub enum LabelPrepareError {
    Measurement(LabelMeasureError),
    Geometry,
    Color,
    MeasurementMismatch,
}
impl fmt::Display for LabelPrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Measurement(error) => write!(f, "label preparation failed: {error}"),
            Self::Geometry => f.write_str("label bounds exceed native coordinate precision"),
            Self::Color => f.write_str("label color must have finite sRGB channels in [0, 1]"),
            Self::MeasurementMismatch => {
                f.write_str("native complete label measurement disagrees with resolved layout")
            }
        }
    }
}
impl std::error::Error for LabelPrepareError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Measurement(error) => Some(error),
            _ => None,
        }
    }
}
impl From<LabelMeasureError> for LabelPrepareError {
    fn from(error: LabelMeasureError) -> Self {
        Self::Measurement(error)
    }
}
pub fn prepare_command_label(
    ir: &CommandLabelIr,
    family: FontFamily,
    color: Color,
) -> Result<DrawCommand, LabelPrepareError> {
    prepare_command_label_at(ir, family, color, PhysicalVector { x: 0.0, y: 0.0 })
}

/// The returned text rectangle includes the parent origin and must not be translated again.
pub fn prepare_command_label_at(
    ir: &CommandLabelIr,
    family: FontFamily,
    color: Color,
    origin: PhysicalVector,
) -> Result<DrawCommand, LabelPrepareError> {
    if [color.r, color.g, color.b, color.a]
        .iter()
        .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    {
        return Err(LabelPrepareError::Color);
    }
    let style = native_style(ir.typography())?;
    let bounds = ir.label_bounds();
    let width = wrap_width(bounds.width)?;
    let height = scalar(bounds.height, "label height")?;
    let x = bounds.x + origin.x;
    let y = bounds.y + origin.y;
    let rect = Rect::new(
        crate::coordinate(x).map_err(|_| LabelPrepareError::Geometry)?,
        crate::coordinate(y).map_err(|_| LabelPrepareError::Geometry)?,
        width,
        height,
    );
    crate::check_coordinate(x + bounds.width, rect.x + rect.width)
        .map_err(|_| LabelPrepareError::Geometry)?;
    crate::check_coordinate(y + bounds.height, rect.y + rect.height)
        .map_err(|_| LabelPrepareError::Geometry)?;
    let measured = measure_command_label(
        family,
        LabelMeasureInput {
            text: ir.text(),
            typography: ir.typography(),
            maximum_width: Some(bounds.width),
        },
    )?;
    if measured.width > f64::from(width)
        || measured.height > f64::from(height)
        || (measured.height - bounds.height).abs() > MAX_COORDINATE_ERROR
    {
        return Err(LabelPrepareError::MeasurementMismatch);
    }
    Ok(DrawCommand::Text {
        text: ir.text().to_owned(),
        rect,
        color,
        font_size: style.size,
        font_family: family,
        font_weight: style.weight,
        line_height: style.line,
        align: TextAlign::Center,
        fit: None,
    })
}
