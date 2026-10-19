use crate::{
    LabelPrepareError, PrepareError, prepare_command_label_at, prepare_lit_control_paint_at,
};
use guido::{
    renderer::DrawCommand,
    widgets::{Color, ContentFit, Rect, font::FontFamily},
};
use resina_model::PhysicalVector;
use resina_resolver::CommandSnapshot;
use std::fmt;

#[derive(Debug)]
pub enum CommandContentPrepareError {
    Label(LabelPrepareError),
    Paint(PrepareError),
}
impl fmt::Display for CommandContentPrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Label(error) => write!(f, "command content label: {error}"),
            Self::Paint(error) => write!(f, "command content paint: {error}"),
        }
    }
}
impl std::error::Error for CommandContentPrepareError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Label(error) => Some(error),
            Self::Paint(error) => Some(error),
        }
    }
}

/// Returns material/navigation paint followed by complete text in one command origin.
/// Keep both commands together when replacing static or sampled motion content.
/// The caller owns font mapping, glyph overhang verification and surrounding clipping.
pub fn prepare_command_content(
    snapshot: &CommandSnapshot<'_>,
    family: FontFamily,
    device_scale: f32,
    samples_per_axis: u8,
) -> Result<[DrawCommand; 2], CommandContentPrepareError> {
    prepare_command_content_at(
        snapshot,
        family,
        PhysicalVector { x: 0.0, y: 0.0 },
        device_scale,
        samples_per_axis,
    )
}

/// Applies parent placement before sampling. Both commands include it and must not be translated again.
pub fn prepare_command_content_at(
    snapshot: &CommandSnapshot<'_>,
    family: FontFamily,
    origin: PhysicalVector,
    device_scale: f32,
    samples_per_axis: u8,
) -> Result<[DrawCommand; 2], CommandContentPrepareError> {
    let paint = snapshot.paint();
    let label = snapshot.label();
    let foreground = paint.paint().body().face().foreground();
    let [r, g, b] = foreground.components().map(|value| value as f32);
    let material =
        prepare_lit_control_paint_at(paint.paint(), origin, device_scale, samples_per_axis)
            .map_err(CommandContentPrepareError::Paint)?;
    let text = prepare_command_label_at(
        label,
        family,
        Color::rgba(r, g, b, foreground.alpha() as f32),
        origin,
        device_scale,
    )
    .map_err(CommandContentPrepareError::Label)?;
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
