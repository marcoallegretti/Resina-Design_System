use resina_model::{ActivationState, InteractionState, StateSet, StateSetInput};
use serde::Deserialize;
use std::fmt;

pub fn resolve_toggle_states(
    activation: &ActivationState,
    hovered: bool,
    checked: bool,
) -> StateSet {
    let base = crate::resolve_command_states(activation, hovered);
    if !checked {
        return base;
    }
    let mut states = base.states().to_vec();
    states.push(InteractionState::Checked);
    StateSet::try_from(StateSetInput {
        schema_version: "0.1.0".to_owned(),
        states,
    })
    .expect("toggle signals are unique and nonempty")
}

#[derive(Debug)]
pub enum ToggleStatesError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
}
impl fmt::Display for ToggleStatesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "toggle states parse failed: {error}"),
            Self::Request(error) => write!(f, "invalid toggle states request: {error}"),
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
        }
    }
}
impl std::error::Error for ToggleStatesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) | Self::Request(error) => Some(error),
            Self::UnsupportedVersion => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    activation: ActivationState,
    hovered: bool,
    checked: bool,
}

pub fn resolve_toggle_states_source(source: &str) -> Result<StateSet, ToggleStatesError> {
    let value = resina_tokens::parse_token_document(source).map_err(ToggleStatesError::Parse)?;
    let request: Request = serde_json::from_value(value).map_err(ToggleStatesError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(ToggleStatesError::UnsupportedVersion);
    }
    Ok(resolve_toggle_states(
        &request.activation,
        request.hovered,
        request.checked,
    ))
}
