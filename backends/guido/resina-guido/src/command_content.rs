use crate::{LabelPrepareError, PrepareError, prepare_command_label, prepare_surface_paint};
use guido::{
    renderer::DrawCommand,
    widgets::{Color, ContentFit, Rect, font::FontFamily},
};
use resina_resolver::{CommandContentError, CommandLabelIr, CommandPaintIr};
use std::fmt;

#[derive(Debug)]
pub enum CommandContentPrepareError {
    Content(CommandContentError),
    Label(LabelPrepareError),
    Paint(PrepareError),
}
impl fmt::Display for CommandContentPrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Content(error) => write!(f, "command content containment: {error}"),
            Self::Label(error) => write!(f, "command content label: {error}"),
            Self::Paint(error) => write!(f, "command content paint: {error}"),
        }
    }
}
impl std::error::Error for CommandContentPrepareError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Content(error) => Some(error),
            Self::Label(error) => Some(error),
            Self::Paint(error) => Some(error),
        }
    }
}

/// Returns material/navigation paint followed by complete text in one command origin.
/// Keep both commands together when replacing static or sampled motion content.
/// The caller owns font mapping, glyph overhang verification and surrounding clipping.
pub fn prepare_command_content(
    paint: &CommandPaintIr,
    label: &CommandLabelIr,
    family: FontFamily,
    device_scale: f32,
    samples_per_axis: u8,
) -> Result<[DrawCommand; 2], CommandContentPrepareError> {
    let body = paint.paint().body();
    label
        .validate_content(body)
        .map_err(CommandContentPrepareError::Content)?;
    let [r, g, b] = body.foreground().components().map(|value| value as f32);
    let text = prepare_command_label(
        label,
        family,
        Color::rgba(r, g, b, body.foreground().alpha() as f32),
    )
    .map_err(CommandContentPrepareError::Label)?;
    let material = prepare_surface_paint(paint.paint(), device_scale, samples_per_axis)
        .map_err(CommandContentPrepareError::Paint)?;
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
