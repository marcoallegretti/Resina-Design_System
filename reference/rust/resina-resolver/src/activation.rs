use resina_model::{ActivationEvent, ActivationKey, ActivationState, PressHold};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug)]
pub enum ActivationError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    InvalidEvent(&'static str),
}

impl fmt::Display for ActivationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "activation parse failed: {error}"),
            Self::Request(error) => write!(f, "invalid activation request: {error}"),
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidEvent(error) => write!(f, "invalid activation event: {error}"),
        }
    }
}
impl std::error::Error for ActivationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) | Self::Request(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CaptureChange {
    Acquire { id: String },
    Release { id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationResult {
    schema_version: &'static str,
    state: ActivationState,
    activate: bool,
    pressed: bool,
    capture: Option<CaptureChange>,
}
impl ActivationResult {
    pub fn state(&self) -> &ActivationState {
        &self.state
    }
    pub fn activate(&self) -> bool {
        self.activate
    }
    pub fn capture(&self) -> Option<&CaptureChange> {
        self.capture.as_ref()
    }
}

fn pointer_id(hold: Option<&PressHold>) -> Option<&str> {
    match hold {
        Some(PressHold::Pointer { id, .. }) => Some(id),
        _ => None,
    }
}

pub fn resolve_activation(
    state: &ActivationState,
    event: &ActivationEvent,
) -> Result<ActivationResult, ActivationError> {
    event.validate().map_err(ActivationError::InvalidEvent)?;
    let mut enabled = state.enabled();
    let mut focused = state.focused();
    let mut hold = state.hold().cloned();
    let mut activate = false;
    match event {
        ActivationEvent::Availability { enabled: value } => {
            enabled = *value;
            if !enabled {
                hold = None;
            }
        }
        ActivationEvent::Focus { focused: value } => {
            focused = *value;
            if !focused && matches!(hold, Some(PressHold::Key { .. })) {
                hold = None;
            }
        }
        ActivationEvent::Cancel {} => hold = None,
        ActivationEvent::Invoke {} if enabled => {
            hold = None;
            activate = true;
        }
        ActivationEvent::PointerDown { id, inside: true } if enabled && hold.is_none() => {
            hold = Some(PressHold::Pointer {
                id: id.clone(),
                inside: true,
            });
        }
        ActivationEvent::PointerMove { id, inside } if pointer_id(hold.as_ref()) == Some(id) => {
            hold = Some(PressHold::Pointer {
                id: id.clone(),
                inside: *inside,
            });
        }
        ActivationEvent::PointerUp { id, inside } if pointer_id(hold.as_ref()) == Some(id) => {
            hold = None;
            activate = *inside;
        }
        ActivationEvent::PointerCancel { id } if pointer_id(hold.as_ref()) == Some(id) => {
            hold = None
        }
        ActivationEvent::KeyDown { key, repeat: false } if enabled && focused && hold.is_none() => {
            hold = Some(PressHold::Key { key: *key });
            activate = *key == ActivationKey::Enter;
        }
        ActivationEvent::KeyUp { key } if hold == Some(PressHold::Key { key: *key }) => {
            hold = None;
            activate = *key == ActivationKey::Space;
        }
        _ => {}
    }
    let before = pointer_id(state.hold());
    let after = pointer_id(hold.as_ref());
    let capture = if before == after {
        None
    } else if let Some(id) = before {
        Some(CaptureChange::Release { id: id.to_owned() })
    } else {
        after.map(|id| CaptureChange::Acquire { id: id.to_owned() })
    };
    let state =
        ActivationState::try_new(enabled, focused, hold).expect("validated activation transition");
    Ok(ActivationResult {
        schema_version: "0.1.0",
        pressed: state.pressed(),
        state,
        activate,
        capture,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    state: ActivationState,
    event: ActivationEvent,
}

pub fn resolve_activation_source(source: &str) -> Result<ActivationResult, ActivationError> {
    let value = parse_token_document(source).map_err(ActivationError::Parse)?;
    let request: Request = serde_json::from_value(value).map_err(ActivationError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(ActivationError::UnsupportedVersion);
    }
    resolve_activation(&request.state, &request.event)
}
