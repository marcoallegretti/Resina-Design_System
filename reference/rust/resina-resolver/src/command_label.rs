use crate::ResolvedTypography;
use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{PhysicalBounds, SurfaceSize};
use serde::Serialize;
use std::fmt;

pub struct CommandLabelInput<'a> {
    pub text: &'a str,
    pub typography: &'a ResolvedTypography,
    pub minimum_size: SurfaceSize,
    pub maximum_size: SurfaceSize,
    pub padding: SafeArea,
    pub direction: LayoutDirection,
}
pub struct LabelMeasureInput<'a> {
    pub text: &'a str,
    pub typography: &'a ResolvedTypography,
    pub maximum_width: Option<f64>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandLabelIr {
    schema_version: &'static str,
    text: String,
    typography: ResolvedTypography,
    size: SurfaceSize,
    label_bounds: PhysicalBounds,
    layout_direction: LayoutDirection,
}
impl CommandLabelIr {
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn typography(&self) -> &ResolvedTypography {
        &self.typography
    }
    pub fn size(&self) -> SurfaceSize {
        self.size
    }
    pub fn label_bounds(&self) -> PhysicalBounds {
        self.label_bounds
    }
    pub fn layout_direction(&self) -> LayoutDirection {
        self.layout_direction
    }
}
#[derive(Debug, PartialEq)]
pub enum CommandLabelError<E> {
    Invalid(&'static str),
    Measurement(E),
}
impl<E: fmt::Display> fmt::Display for CommandLabelError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => f.write_str(message),
            Self::Measurement(error) => write!(f, "label measurement failed: {error}"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for CommandLabelError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Measurement(error) => Some(error),
            _ => None,
        }
    }
}
fn valid_size(size: SurfaceSize) -> bool {
    size.width.is_finite() && size.width > 0.0 && size.height.is_finite() && size.height > 0.0
}
fn extent_sum(left: f64, right: f64) -> Option<f64> {
    let sum = left + right;
    (sum.is_finite() && (left == 0.0 || sum > right) && (right == 0.0 || sum > left)).then_some(sum)
}
pub fn resolve_command_label<E>(
    input: CommandLabelInput<'_>,
    mut measure: impl FnMut(LabelMeasureInput<'_>) -> Result<SurfaceSize, E>,
) -> Result<CommandLabelIr, CommandLabelError<E>> {
    use CommandLabelError::Invalid;
    if input.text.trim().is_empty() {
        return Err(Invalid("command label must not be blank"));
    }
    if !valid_size(input.minimum_size) || !valid_size(input.maximum_size) {
        return Err(Invalid("command size limits must be finite and positive"));
    }
    if input.minimum_size.width > input.maximum_size.width
        || input.minimum_size.height > input.maximum_size.height
    {
        return Err(Invalid("minimum command size exceeds maximum size"));
    }
    let padding = &input.padding;
    if [padding.start, padding.end, padding.top, padding.bottom]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.0)
    {
        return Err(Invalid("command padding must be finite and nonnegative"));
    }
    let horizontal = extent_sum(padding.start, padding.end).ok_or(Invalid(
        "horizontal padding exceeds representable arithmetic",
    ))?;
    let vertical = extent_sum(padding.top, padding.bottom)
        .ok_or(Invalid("vertical padding exceeds representable arithmetic"))?;
    if horizontal >= input.maximum_size.width || vertical >= input.maximum_size.height {
        return Err(Invalid("command padding leaves no label extent"));
    }
    let natural = measure(LabelMeasureInput {
        text: input.text,
        typography: input.typography,
        maximum_width: None,
    })
    .map_err(CommandLabelError::Measurement)?;
    if !valid_size(natural) {
        return Err(Invalid(
            "natural label measurement must be finite and positive",
        ));
    }
    let preferred_width = extent_sum(natural.width, horizontal).ok_or(Invalid(
        "preferred command width exceeds representable arithmetic",
    ))?;
    let width = preferred_width
        .max(input.minimum_size.width)
        .min(input.maximum_size.width);
    let label_width = width - horizontal;
    if label_width <= 0.0 || (horizontal > 0.0 && label_width == width) {
        return Err(Invalid("chosen command width leaves no label extent"));
    }
    let fitted = measure(LabelMeasureInput {
        text: input.text,
        typography: input.typography,
        maximum_width: Some(label_width),
    })
    .map_err(CommandLabelError::Measurement)?;
    if !valid_size(fitted) {
        return Err(Invalid(
            "fitted label measurement must be finite and positive",
        ));
    }
    if fitted.width > label_width {
        return Err(Invalid("fitted label exceeds offered width"));
    }
    let height = extent_sum(fitted.height, vertical)
        .ok_or(Invalid("command height exceeds representable arithmetic"))?
        .max(input.minimum_size.height);
    if height > input.maximum_size.height {
        return Err(Invalid("complete label exceeds available command height"));
    }
    let content_height = height - vertical;
    if content_height < fitted.height || (vertical > 0.0 && content_height == height) {
        return Err(Invalid(
            "label height is lost in command padding arithmetic",
        ));
    }
    let left = if input.direction == LayoutDirection::Ltr {
        padding.start
    } else {
        padding.end
    };
    let top = padding.top + (content_height - fitted.height) * 0.5;
    Ok(CommandLabelIr {
        schema_version: "0.1.0",
        text: input.text.to_owned(),
        typography: input.typography.clone(),
        size: SurfaceSize { width, height },
        label_bounds: PhysicalBounds {
            x: left,
            y: top,
            width: label_width,
            height: fitted.height,
        },
        layout_direction: input.direction,
    })
}
