use resina_model::{ActivationState, InteractionState, StateSet, StateSetInput};
use serde::Deserialize;
use std::fmt;

pub fn resolve_command_states(activation: &ActivationState, hovered: bool) -> StateSet {
    let mut states = Vec::new();
    if !activation.enabled() {
        states.push(InteractionState::Disabled);
    }
    if hovered {
        states.push(InteractionState::Hover);
    }
    if activation.pressed() {
        states.push(InteractionState::Pressed);
    }
    if states.is_empty() {
        states.push(InteractionState::Rest);
    }
    if activation.focused() {
        states.push(InteractionState::Focused);
    }
    StateSet::try_from(StateSetInput {
        schema_version: "0.1.0".to_owned(),
        states,
    })
    .expect("command signals are unique and nonempty")
}

#[derive(Debug)]
pub enum CommandStatesError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
}
impl fmt::Display for CommandStatesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "command states parse failed: {error}"),
            Self::Request(error) => write!(f, "invalid command states request: {error}"),
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
        }
    }
}
impl std::error::Error for CommandStatesError {
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
}

pub fn resolve_command_states_source(source: &str) -> Result<StateSet, CommandStatesError> {
    let value = resina_tokens::parse_token_document(source).map_err(CommandStatesError::Parse)?;
    let request: Request = serde_json::from_value(value).map_err(CommandStatesError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(CommandStatesError::UnsupportedVersion);
    }
    Ok(resolve_command_states(&request.activation, request.hovered))
}
