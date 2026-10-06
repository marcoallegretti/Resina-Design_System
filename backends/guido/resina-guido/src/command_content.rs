use crate::{LabelPrepareError, PrepareError, prepare_command_label, prepare_surface_paint};
use guido::{
    renderer::DrawCommand,
    widgets::{Color, ContentFit, Rect, font::FontFamily},
};
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
    let paint = snapshot.paint();
    let label = snapshot.label();
    let body = paint.paint().body();
    let [r, g, b] = body.foreground().components().map(|value| value as f32);
    let material = prepare_surface_paint(paint.paint(), device_scale, samples_per_axis)
        .map_err(CommandContentPrepareError::Paint)?;
    let text = prepare_command_label(
        label,
        family,
        Color::rgba(r, g, b, body.foreground().alpha() as f32),
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
