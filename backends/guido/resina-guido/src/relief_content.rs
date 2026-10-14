use crate::{
    LabelPrepareError, PrepareError, PreparedPaint, prepare_command_label_at,
    prepare_material_relief_paint_at, prepare_relief_paint_at,
};
use guido::{
    renderer::DrawCommand,
    widgets::{Color, ContentFit, Rect, font::FontFamily},
};
use resina_model::PhysicalVector;
use resina_resolver::{CommandContentError, CommandLabelIr, MaterialReliefPaint, ReliefPaint};
use std::fmt;

#[derive(Debug)]
pub enum ReliefContentPrepareError {
    Content(CommandContentError),
    Label(LabelPrepareError),
    Paint(PrepareError),
}
impl fmt::Display for ReliefContentPrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Content(error) => write!(f, "relief content containment: {error}"),
            Self::Label(error) => write!(f, "relief content label: {error}"),
            Self::Paint(error) => write!(f, "relief content paint: {error}"),
        }
    }
}
impl std::error::Error for ReliefContentPrepareError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Content(error) => Some(error),
            Self::Label(error) => Some(error),
            Self::Paint(error) => Some(error),
        }
    }
}

/// Prepares a relief face followed by complete text at one physical origin.
/// The caller owns font mapping, actual glyph ink verification and surrounding clipping.
pub fn prepare_relief_content_at(
    paint: &ReliefPaint,
    label: &CommandLabelIr,
    family: FontFamily,
    origin: PhysicalVector,
    device_scale: f32,
    samples_per_axis: u8,
) -> Result<[DrawCommand; 2], ReliefContentPrepareError> {
    label
        .validate_relief_content(paint)
        .map_err(ReliefContentPrepareError::Content)?;
    let [r, g, b] = paint.foreground().components().map(|value| value as f32);
    prepare_content_at(
        label,
        family,
        Color::rgba(r, g, b, 1.0),
        origin,
        device_scale,
        || prepare_relief_paint_at(paint, origin, device_scale, samples_per_axis),
    )
}

pub fn prepare_material_relief_content_at(
    paint: &MaterialReliefPaint,
    label: &CommandLabelIr,
    family: FontFamily,
    origin: PhysicalVector,
    device_scale: f32,
    samples_per_axis: u8,
) -> Result<[DrawCommand; 2], ReliefContentPrepareError> {
    label
        .validate_material_relief_content(paint)
        .map_err(ReliefContentPrepareError::Content)?;
    let [r, g, b] = paint.foreground().components().map(|value| value as f32);
    prepare_content_at(
        label,
        family,
        Color::rgba(r, g, b, 1.0),
        origin,
        device_scale,
        || prepare_material_relief_paint_at(paint, origin, device_scale, samples_per_axis),
    )
}

fn prepare_content_at(
    label: &CommandLabelIr,
    family: FontFamily,
    color: Color,
    origin: PhysicalVector,
    device_scale: f32,
    material: impl FnOnce() -> Result<PreparedPaint, PrepareError>,
) -> Result<[DrawCommand; 2], ReliefContentPrepareError> {
    let text = prepare_command_label_at(label, family, color, origin, device_scale)
        .map_err(ReliefContentPrepareError::Label)?;
    let material = material().map_err(ReliefContentPrepareError::Paint)?;
    let origin = material.origin();
    let size = material.logical_size();
    Ok([
        DrawCommand::Image {
            source: material.image_source(),
            decoded: None,
            rect: Rect::new(origin.0, origin.1, size.width, size.height),
            content_fit: ContentFit::Fill,
            tint: None,
        },
        text,
    ])
}
