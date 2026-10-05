use crate::{
    LabelPrepareError, PrepareError, PreparedPaint, prepare_surface_paint_at,
    text::prepare_command_label_at,
};
use guido::{
    renderer::DrawCommand,
    widgets::{Color, ContentFit, Rect, font::FontFamily},
};
use resina_model::PhysicalVector;
use resina_resolver::SliderSnapshot;
use std::fmt;

#[derive(Debug)]
pub enum SliderContentPrepareError {
    Label(LabelPrepareError),
    Track(PrepareError),
    Thumb(PrepareError),
}
impl fmt::Display for SliderContentPrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Label(error) => write!(f, "slider content label: {error}"),
            Self::Track(error) => write!(f, "slider content track: {error}"),
            Self::Thumb(error) => write!(f, "slider content thumb: {error}"),
        }
    }
}
impl std::error::Error for SliderContentPrepareError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Label(error) => Some(error),
            Self::Track(error) | Self::Thumb(error) => Some(error),
        }
    }
}
fn image(paint: PreparedPaint) -> DrawCommand {
    let origin = paint.origin();
    let size = paint.logical_size();
    DrawCommand::Image {
        source: paint.image_source(),
        decoded: None,
        rect: Rect::new(origin.0, origin.1, size.width, size.height),
        content_fit: ContentFit::Fill,
        tint: None,
    }
}

/// Prepare the checked track, thumb with focus, and complete native label together.
/// The caller owns font mapping, glyph overhang, parent clipping and native semantics.
pub fn prepare_slider_content(
    snapshot: &SliderSnapshot<'_>,
    family: FontFamily,
    device_scale: f32,
    samples_per_axis: u8,
) -> Result<[DrawCommand; 3], SliderContentPrepareError> {
    let foreground = snapshot.label_foreground();
    let [r, g, b] = foreground.components().map(|value| value as f32);
    let text = prepare_command_label_at(
        snapshot.label(),
        family,
        Color::rgba(r, g, b, foreground.alpha() as f32),
        snapshot.label_origin(),
    )
    .map_err(SliderContentPrepareError::Label)?;
    let track = snapshot.layout().track_bounds();
    let track = prepare_surface_paint_at(
        snapshot.track().paint(),
        PhysicalVector {
            x: track.x,
            y: track.y,
        },
        device_scale,
        samples_per_axis,
    )
    .map_err(SliderContentPrepareError::Track)?;
    let thumb = snapshot.layout().thumb_bounds();
    let thumb = prepare_surface_paint_at(
        snapshot.thumb().paint(),
        PhysicalVector {
            x: thumb.x,
            y: thumb.y,
        },
        device_scale,
        samples_per_axis,
    )
    .map_err(SliderContentPrepareError::Thumb)?;
    Ok([image(track), image(thumb), text])
}
