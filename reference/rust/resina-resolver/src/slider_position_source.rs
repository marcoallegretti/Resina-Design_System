use crate::{
    SliderAdjustmentIr, SliderLayoutSourceError, SliderPositionError, SliderPositionInput,
    resolve_slider_layout_source, resolve_slider_position,
};
use resina_tokens::parse_token_document;
use serde::Deserialize;
use serde_json::value::RawValue;
use std::fmt;

#[derive(Debug)]
pub enum SliderPositionSourceError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    InvalidObject,
    Layout(SliderLayoutSourceError),
    Position(SliderPositionError),
}

impl fmt::Display for SliderPositionSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "slider position parse failed: {error}"),
            Self::Request(error) => write!(f, "invalid slider position request: {error}"),
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidObject => f.write_str("slider position request must be a JSON object"),
            Self::Layout(error) => write!(f, "slider position layout: {error}"),
            Self::Position(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for SliderPositionSourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) | Self::Request(error) => Some(error),
            Self::Layout(error) => Some(error),
            Self::Position(error) => Some(error),
            Self::UnsupportedVersion | Self::InvalidObject => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request<'a> {
    schema_version: String,
    #[serde(borrow)]
    layout: &'a RawValue,
    desired_origin: f64,
    enabled: bool,
    read_only: bool,
}

pub fn resolve_slider_position_source(
    source: &str,
) -> Result<SliderAdjustmentIr, SliderPositionSourceError> {
    let document = parse_token_document(source).map_err(SliderPositionSourceError::Parse)?;
    if !document.is_object() {
        return Err(SliderPositionSourceError::InvalidObject);
    }
    let request: Request<'_> =
        serde_json::from_str(source).map_err(SliderPositionSourceError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(SliderPositionSourceError::UnsupportedVersion);
    }
    let layout = resolve_slider_layout_source(request.layout.get())
        .map_err(SliderPositionSourceError::Layout)?;
    resolve_slider_position(SliderPositionInput {
        layout: &layout,
        desired_origin: request.desired_origin,
        enabled: request.enabled,
        read_only: request.read_only,
    })
    .map_err(SliderPositionSourceError::Position)
}
