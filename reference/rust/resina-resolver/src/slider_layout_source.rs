use crate::{
    SliderLayoutError, SliderLayoutInput, SliderLayoutIr, SliderMinimumPosition, SliderValueError,
    resolve_slider_layout, resolve_slider_value_source,
};
use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{SliderOrientation, SurfaceSize};
use resina_tokens::parse_token_document;
use serde::Deserialize;
use serde_json::value::RawValue;
use std::fmt;

#[derive(Debug)]
pub enum SliderLayoutSourceError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    InvalidShape(&'static str, &'static str),
    Value(SliderValueError),
    Layout(SliderLayoutError),
}

impl fmt::Display for SliderLayoutSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "slider layout parse failed: {error}"),
            Self::Request(error) => write!(f, "invalid slider layout request: {error}"),
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidShape(field, shape) => {
                write!(f, "slider layout request {field} must be a JSON {shape}")
            }
            Self::Value(error) => write!(f, "slider layout value: {error}"),
            Self::Layout(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for SliderLayoutSourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) | Self::Request(error) => Some(error),
            Self::Value(error) => Some(error),
            Self::Layout(error) => Some(error),
            Self::UnsupportedVersion | Self::InvalidShape(_, _) => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request<'a> {
    schema_version: String,
    allocation_size: SurfaceSize,
    thumb_size: SurfaceSize,
    track_thickness: f64,
    insets: SafeArea,
    layout_direction: LayoutDirection,
    orientation: SliderOrientation,
    minimum_position: SliderMinimumPosition,
    #[serde(borrow)]
    value: &'a RawValue,
}

pub fn resolve_slider_layout_source(
    source: &str,
) -> Result<SliderLayoutIr, SliderLayoutSourceError> {
    let document = parse_token_document(source).map_err(SliderLayoutSourceError::Parse)?;
    if !document.is_object() {
        return Err(SliderLayoutSourceError::InvalidShape("root", "object"));
    }
    for field in ["allocationSize", "thumbSize", "insets", "value"] {
        if !document[field].is_object() {
            return Err(SliderLayoutSourceError::InvalidShape(field, "object"));
        }
    }
    for field in ["layoutDirection", "orientation", "minimumPosition"] {
        if !document[field].is_string() {
            return Err(SliderLayoutSourceError::InvalidShape(field, "string"));
        }
    }
    let request: Request<'_> =
        serde_json::from_str(source).map_err(SliderLayoutSourceError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(SliderLayoutSourceError::UnsupportedVersion);
    }
    let value =
        resolve_slider_value_source(request.value.get()).map_err(SliderLayoutSourceError::Value)?;
    resolve_slider_layout(SliderLayoutInput {
        allocation_size: request.allocation_size,
        thumb_size: request.thumb_size,
        track_thickness: request.track_thickness,
        insets: &request.insets,
        layout_direction: request.layout_direction,
        orientation: request.orientation,
        minimum_position: request.minimum_position,
        value: &value,
    })
    .map_err(SliderLayoutSourceError::Layout)
}
