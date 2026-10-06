use crate::MAX_COORDINATE_ERROR;
use guido::{
    renderer::{DrawCommand, LineFit, Measured, measure_text_full},
    widgets::font::{FontFamily, FontWeight, LineHeight},
    widgets::{Color, Rect, TextAlign, TextOverflow},
};
use resina_model::{PhysicalVector, SurfaceSize};
use resina_resolver::{CommandLabelIr, LabelMeasureInput, ResolvedTypography};
use std::fmt;

#[derive(Debug, PartialEq)]
pub enum LabelMeasureError {
    FontWeight,
    Precision(&'static str),
    Extent,
    UnsupportedLineBreak(char),
    UnsupportedParagraphSeparator(char),
    BidiInitiators,
}
impl fmt::Display for LabelMeasureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FontWeight => f.write_str("GUIdo font weight must be an integer"),
            Self::Precision(field) => write!(f, "GUIdo label {field} exceeds coordinate precision"),
            Self::Extent => f.write_str("GUIdo label measurement is empty or nonfinite"),
            Self::UnsupportedLineBreak(break_char) => write!(
                f,
                "GUIdo text does not end a line at mandatory break U+{:04X}",
                u32::from(*break_char)
            ),
            Self::UnsupportedParagraphSeparator(separator) => write!(
                f,
                "GUIdo text cannot separate bidirectional paragraphs at U+{:04X}",
                u32::from(*separator)
            ),
            Self::BidiInitiators => write!(
                f,
                "GUIdo text has more than {MAX_BIDI_INITIATORS} bidirectional embedding or isolate initiators between line breaks"
            ),
        }
    }
}
impl std::error::Error for LabelMeasureError {}
/// Line contents split at CR and LF the way GUIdo's shaper does: each ending starts
/// at the next CR or LF and takes CRLF, then (only with `lf_cr`) LF CR, as one.
fn hard_lines(text: &str, lf_cr: bool) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut rest = text;
    while let Some(index) = rest.find(['\r', '\n']) {
        lines.push(&rest[..index]);
        let after = &rest[index..];
        let ending = if after.starts_with("\r\n") || (lf_cr && after.starts_with("\n\r")) {
            2
        } else {
            1
        };
        rest = &after[ending..];
    }
    lines.push(rest);
    lines
}
/// GUIdo ends lines only at CR and LF; Unicode mandatory breaks keep only CR LF
/// together, while GUIdo's shaper also joins an LF CR that does not continue a CRLF.
/// It also cannot separate bidirectional paragraphs within one line.
fn supported_separators(text: &str) -> Result<(), LabelMeasureError> {
    if let Some(break_char) = text
        .chars()
        .find(|c| matches!(c, '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}'))
    {
        return Err(LabelMeasureError::UnsupportedLineBreak(break_char));
    }
    if hard_lines(text, true) != hard_lines(text, false) {
        return Err(LabelMeasureError::UnsupportedLineBreak('\r'));
    }
    // Bidirectional paragraph separators that do not break lines. When the
    // paragraphs they separate resolve to different directions, the pinned
    // shaper panics.
    if let Some(separator) = text.chars().find(|c| ('\u{1c}'..='\u{1e}').contains(c)) {
        return Err(LabelMeasureError::UnsupportedParagraphSeparator(separator));
    }
    let initiator = |c: &char| {
        matches!(
            c,
            '\u{202a}' | '\u{202b}' | '\u{202d}' | '\u{202e}' | '\u{2066}'..='\u{2068}'
        )
    };
    if hard_lines(text, false)
        .iter()
        .any(|line| line.chars().filter(initiator).count() > MAX_BIDI_INITIATORS)
    {
        return Err(LabelMeasureError::BidiInitiators);
    }
    Ok(())
}
/// Text with n initiators resolves to level 2n + 2 at most: each raises the explicit
/// level by at most two from a paragraph level of 0 or 1, and implicit resolution
/// adds two to an even level or one to an odd level. The pinned shaper panics when a
/// wrapped line resolves to 126, above 125, the highest right-to-left level. This
/// limit keeps the maximum at 122. Counting initiators, not nesting, also covers
/// terminators the bidirectional algorithm ignores.
const MAX_BIDI_INITIATORS: usize = 60;
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
    spacing: f32,
}
fn native_style(style: &ResolvedTypography) -> Result<NativeLabelStyle, LabelMeasureError> {
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
    let spacing = style.letter_spacing() as f32;
    if !spacing.is_finite()
        || (f64::from(spacing) - style.letter_spacing()).abs() > MAX_COORDINATE_ERROR
    {
        return Err(LabelMeasureError::Precision("letter spacing"));
    }
    Ok(NativeLabelStyle {
        size,
        weight: FontWeight(style.font_weight() as u16),
        line: LineHeight::Relative(line),
        spacing,
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
/// Shapes with the same scaled size, spacing and width as GUIdo's draw path at `scale`.
fn shape_native(
    text: &str,
    family: FontFamily,
    style: &NativeLabelStyle,
    width: Option<f32>,
    scale: f32,
) -> Measured {
    measure_text_full(
        text,
        style.size * scale,
        width.map(|width| width * scale),
        family,
        style.weight,
        style.line,
        style.spacing * scale,
        None,
    )
}
fn measure_native(
    text: &str,
    family: FontFamily,
    style: &NativeLabelStyle,
    width: Option<f32>,
    scale: f32,
) -> Result<Measured, LabelMeasureError> {
    let measured = shape_native(text, family, style, width, scale);
    if !measured.size.width.is_finite()
        || measured.size.width <= 0.0
        || !measured.size.height.is_finite()
        || measured.size.height <= 0.0
    {
        return Err(LabelMeasureError::Extent);
    }
    Ok(measured)
}
pub fn measure_command_label(
    family: FontFamily,
    input: LabelMeasureInput<'_>,
) -> Result<SurfaceSize, LabelMeasureError> {
    supported_separators(input.text)?;
    let style = native_style(input.typography)?;
    let width = input.maximum_width.map(wrap_width).transpose()?;
    let measured = measure_native(input.text, family, &style, width, 1.0)?;
    Ok(SurfaceSize {
        width: f64::from(measured.size.width),
        height: f64::from(measured.size.height),
    })
}

#[derive(Debug, PartialEq)]
pub enum LabelPrepareError {
    Measurement(LabelMeasureError),
    InvalidScale,
    Geometry,
    Color,
    MeasurementMismatch,
    ScaledMetrics,
    ScaledLineMismatch,
}
impl fmt::Display for LabelPrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Measurement(error) => write!(f, "label preparation failed: {error}"),
            Self::InvalidScale => f.write_str("device scale must be finite and positive"),
            Self::Geometry => f.write_str("label bounds exceed native coordinate precision"),
            Self::Color => f.write_str("label color must have finite sRGB channels in [0, 1]"),
            Self::MeasurementMismatch => {
                f.write_str("native complete label measurement disagrees with resolved layout")
            }
            Self::ScaledMetrics => {
                f.write_str("label metrics at the device scale are not representable by GUIdo")
            }
            Self::ScaledLineMismatch => f.write_str(
                "native label lines shaped at the device scale disagree with resolved layout",
            ),
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
    device_scale: f32,
) -> Result<DrawCommand, LabelPrepareError> {
    prepare_command_label_at(
        ir,
        family,
        color,
        PhysicalVector { x: 0.0, y: 0.0 },
        device_scale,
    )
}

/// The returned text rectangle includes the parent origin and must not be translated again.
/// GUIdo shapes text in device pixels, so prepare again when the device scale changes.
pub fn prepare_command_label_at(
    ir: &CommandLabelIr,
    family: FontFamily,
    color: Color,
    origin: PhysicalVector,
    device_scale: f32,
) -> Result<DrawCommand, LabelPrepareError> {
    if !device_scale.is_finite() || device_scale <= 0.0 {
        return Err(LabelPrepareError::InvalidScale);
    }
    if [color.r, color.g, color.b, color.a]
        .iter()
        .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    {
        return Err(LabelPrepareError::Color);
    }
    supported_separators(ir.text())?;
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
    // GUIdo raises a device font size below 0.01 px and drops nonfinite spacing
    // instead of failing.
    if style.size * device_scale < 0.01
        || [style.size, style.spacing, width, height]
            .iter()
            .any(|value| !(value * device_scale).is_finite())
    {
        return Err(LabelPrepareError::ScaledMetrics);
    }
    let measured = measure_native(ir.text(), family, &style, Some(width), 1.0)?;
    if measured.size.width > width
        || measured.size.height > height
        || (f64::from(measured.size.height) - bounds.height).abs() > MAX_COORDINATE_ERROR
    {
        return Err(LabelPrepareError::MeasurementMismatch);
    }
    // A line that exactly fills the box can wrap again when shaped at device size:
    // scaled advances and spacing round independently of the scaled width. GUIdo's
    // transformed text path shapes at twice the device scale; doubling is exact in
    // binary32, so this check also covers that path.
    let fit = if measured.wraps {
        let scaled = measure_native(ir.text(), family, &style, Some(width), device_scale)?;
        let scale = f64::from(device_scale);
        let budget = MAX_COORDINATE_ERROR * scale;
        if (f64::from(scaled.size.width) - f64::from(measured.size.width) * scale).abs() > budget {
            return Err(LabelPrepareError::ScaledLineMismatch);
        }
        // Greedy breaking makes the first differing break change the line count of
        // the prefix that ends at one of the two break positions.
        let ends = ir.text().char_indices().skip(1).map(|(end, _)| end);
        for end in ends.chain([ir.text().len()]) {
            let prefix = &ir.text()[..end];
            let logical = shape_native(prefix, family, &style, Some(width), 1.0);
            let device = shape_native(prefix, family, &style, Some(width), device_scale);
            if (f64::from(device.size.height) - f64::from(logical.size.height) * scale).abs()
                > budget
            {
                return Err(LabelPrepareError::ScaledLineMismatch);
            }
        }
        None
    } else {
        Some(LineFit {
            width: None,
            max_lines: None,
            overflow: TextOverflow::Clip,
            wrap: false,
        })
    };
    Ok(DrawCommand::Text {
        text: ir.text().to_owned(),
        rect,
        color,
        font_size: style.size,
        font_family: family,
        font_weight: style.weight,
        line_height: style.line,
        letter_spacing: style.spacing,
        align: TextAlign::Center,
        fit,
    })
}
