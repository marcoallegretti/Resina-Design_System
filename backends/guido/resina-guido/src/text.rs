use crate::MAX_COORDINATE_ERROR;
use guido::{
    renderer::measure_text_styled,
    widgets::font::{FontFamily, FontWeight, LineHeight},
};
use resina_model::SurfaceSize;
use resina_resolver::LabelMeasureInput;
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
pub fn measure_command_label(
    family: FontFamily,
    input: LabelMeasureInput<'_>,
) -> Result<SurfaceSize, LabelMeasureError> {
    let style = input.typography;
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
    let width = input
        .maximum_width
        .map(|value| {
            let mut native = scalar(value, "wrap width")?;
            if f64::from(native) > value {
                native = native.next_down();
            }
            if native <= 0.0 || (value - f64::from(native)).abs() > MAX_COORDINATE_ERROR {
                return Err(LabelMeasureError::Precision("wrap width"));
            }
            Ok(native)
        })
        .transpose()?;
    let measured = measure_text_styled(
        input.text,
        size,
        width,
        family,
        FontWeight(style.font_weight() as u16),
        LineHeight::Relative(line),
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
