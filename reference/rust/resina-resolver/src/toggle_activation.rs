use crate::{ActivationError, ActivationResult, resolve_activation};
use resina_model::{ActivationEvent, ActivationState};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleActivationResult {
    schema_version: &'static str,
    activation: ActivationResult,
    checked: bool,
}

impl ToggleActivationResult {
    pub fn activation(&self) -> &ActivationResult {
        &self.activation
    }
    pub fn checked(&self) -> bool {
        self.checked
    }
}

pub fn resolve_toggle_activation(
    state: &ActivationState,
    checked: bool,
    event: &ActivationEvent,
) -> Result<ToggleActivationResult, ActivationError> {
    let activation = resolve_activation(state, event)?;
    let checked = if activation.activate() {
        !checked
    } else {
        checked
    };
    Ok(ToggleActivationResult {
        schema_version: "0.1.0",
        activation,
        checked,
    })
}

#[derive(Debug)]
pub enum ToggleActivationError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    InvalidRequestShape,
    Activation(ActivationError),
}

impl fmt::Display for ToggleActivationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "toggle activation parse failed: {error}"),
            Self::Request(error) => write!(f, "invalid toggle activation request: {error}"),
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidRequestShape => {
                f.write_str("toggle activation request must be a JSON object")
            }
            Self::Activation(error) => write!(f, "toggle activation: {error}"),
        }
    }
}
impl std::error::Error for ToggleActivationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) | Self::Request(error) => Some(error),
            Self::Activation(error) => Some(error),
            Self::UnsupportedVersion | Self::InvalidRequestShape => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    state: ActivationState,
    checked: bool,
    event: ActivationEvent,
}

pub fn resolve_toggle_activation_source(
    source: &str,
) -> Result<ToggleActivationResult, ToggleActivationError> {
    let value = parse_token_document(source).map_err(ToggleActivationError::Parse)?;
    if !value.is_object() {
        return Err(ToggleActivationError::InvalidRequestShape);
    }
    let request: Request = serde_json::from_value(value).map_err(ToggleActivationError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(ToggleActivationError::UnsupportedVersion);
    }
    resolve_toggle_activation(&request.state, request.checked, &request.event)
        .map_err(ToggleActivationError::Activation)
}
