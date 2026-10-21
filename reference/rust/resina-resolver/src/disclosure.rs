use crate::{
    ActivationError, ActivationResult, CommandAccessibilityError, CommandAccessibilityInput,
    CommandAccessibilityIr, resolve_activation, resolve_command_accessibility,
};
use resina_model::{ActivationEvent, ActivationState};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy)]
pub struct DisclosureInput<'a> {
    pub command: CommandAccessibilityInput<'a>,
    pub expanded: bool,
    pub controlled_content_identity: &'a str,
    pub controlled_content_present: bool,
    pub event: &'a ActivationEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DisclosureAccessibilityState {
    enabled: bool,
    focused: bool,
    expanded: bool,
}

impl DisclosureAccessibilityState {
    pub fn enabled(self) -> bool {
        self.enabled
    }

    pub fn focused(self) -> bool {
        self.focused
    }

    pub fn expanded(self) -> bool {
        self.expanded
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DisclosureAccessibilityAction {
    kind: &'static str,
    available: bool,
}

impl DisclosureAccessibilityAction {
    pub fn kind(self) -> &'static str {
        self.kind
    }

    pub fn available(self) -> bool {
        self.available
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DisclosureRelationship {
    kind: &'static str,
    target: String,
}

impl DisclosureRelationship {
    pub fn kind(&self) -> &'static str {
        self.kind
    }

    pub fn target(&self) -> &str {
        &self.target
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisclosureAccessibilityIr {
    schema_version: &'static str,
    role: &'static str,
    name: String,
    description: Option<String>,
    value: (),
    state: DisclosureAccessibilityState,
    actions: [DisclosureAccessibilityAction; 1],
    focusable: bool,
    relationships: [DisclosureRelationship; 1],
}

impl DisclosureAccessibilityIr {
    pub fn role(&self) -> &'static str {
        self.role
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn state(&self) -> DisclosureAccessibilityState {
        self.state
    }

    pub fn actions(&self) -> &[DisclosureAccessibilityAction; 1] {
        &self.actions
    }

    pub fn focusable(&self) -> bool {
        self.focusable
    }

    pub fn relationships(&self) -> &[DisclosureRelationship; 1] {
        &self.relationships
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisclosureResult {
    schema_version: &'static str,
    activation: ActivationResult,
    expanded: bool,
    accessibility: DisclosureAccessibilityIr,
}

impl DisclosureResult {
    pub fn activation(&self) -> &ActivationResult {
        &self.activation
    }

    pub fn expanded(&self) -> bool {
        self.expanded
    }

    pub fn accessibility(&self) -> &DisclosureAccessibilityIr {
        &self.accessibility
    }
}

#[derive(Debug)]
pub enum DisclosureError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    InvalidRequestShape,
    BlankName,
    BlankDescription,
    EmptyControlledContentIdentity,
    StaleControlledContent,
    EnabledNotFocusable,
    FocusedNotFocusable,
    Activation(ActivationError),
}

impl fmt::Display for DisclosureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "disclosure parse failed: {error}"),
            Self::Request(error) => write!(f, "invalid disclosure request: {error}"),
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidRequestShape => f.write_str("disclosure request must be a JSON object"),
            Self::BlankName => f.write_str("disclosure name must not be blank"),
            Self::BlankDescription => {
                f.write_str("disclosure description must be nonblank or absent")
            }
            Self::EmptyControlledContentIdentity => {
                f.write_str("controlled content identity must not be empty")
            }
            Self::StaleControlledContent => f.write_str("controlled content identity is stale"),
            Self::EnabledNotFocusable => f.write_str("enabled disclosure must be focusable"),
            Self::FocusedNotFocusable => {
                f.write_str("actual disclosure focus requires focusability")
            }
            Self::Activation(error) => write!(f, "disclosure activation: {error}"),
        }
    }
}

impl std::error::Error for DisclosureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) | Self::Request(error) => Some(error),
            Self::Activation(error) => Some(error),
            _ => None,
        }
    }
}

pub fn resolve_disclosure(input: DisclosureInput<'_>) -> Result<DisclosureResult, DisclosureError> {
    if input.controlled_content_identity.is_empty() {
        return Err(DisclosureError::EmptyControlledContentIdentity);
    }
    if !input.controlled_content_present {
        return Err(DisclosureError::StaleControlledContent);
    }
    let command = resolve_command_accessibility(input.command).map_err(disclosure_command_error)?;
    resolve_disclosure_command(
        command,
        input.command.activation,
        input.expanded,
        input.controlled_content_identity,
        input.event,
    )
}

fn disclosure_command_error(error: CommandAccessibilityError) -> DisclosureError {
    match error {
        CommandAccessibilityError::BlankDescription => DisclosureError::BlankDescription,
        CommandAccessibilityError::EnabledNotFocusable => DisclosureError::EnabledNotFocusable,
        CommandAccessibilityError::FocusedNotFocusable => DisclosureError::FocusedNotFocusable,
    }
}

fn resolve_disclosure_command(
    command: CommandAccessibilityIr,
    activation_state: &ActivationState,
    was_expanded: bool,
    controlled_content_identity: &str,
    event: &ActivationEvent,
) -> Result<DisclosureResult, DisclosureError> {
    let activation =
        resolve_activation(activation_state, event).map_err(DisclosureError::Activation)?;
    if !command.focusable() {
        if activation.state().enabled() {
            return Err(DisclosureError::EnabledNotFocusable);
        }
        if activation.state().focused() {
            return Err(DisclosureError::FocusedNotFocusable);
        }
    }
    let expanded = if activation.activate() {
        !was_expanded
    } else {
        was_expanded
    };
    let state = activation.state();
    Ok(DisclosureResult {
        schema_version: "0.1.0",
        accessibility: DisclosureAccessibilityIr {
            schema_version: "0.1.0",
            role: "button",
            name: command.name().to_owned(),
            description: command.description().map(str::to_owned),
            value: (),
            state: DisclosureAccessibilityState {
                enabled: state.enabled(),
                focused: state.focused(),
                expanded,
            },
            actions: [DisclosureAccessibilityAction {
                kind: "invoke",
                available: state.enabled(),
            }],
            focusable: command.focusable(),
            relationships: [DisclosureRelationship {
                kind: "controls",
                target: controlled_content_identity.to_owned(),
            }],
        },
        activation,
        expanded,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ControlledContent {
    identity: String,
    present: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    name: String,
    #[serde(deserialize_with = "Option::deserialize")]
    description: Option<String>,
    activation: ActivationState,
    focusable: bool,
    expanded: bool,
    controlled_content: ControlledContent,
    event: ActivationEvent,
}

pub fn resolve_disclosure_source(source: &str) -> Result<DisclosureResult, DisclosureError> {
    let value = parse_token_document(source).map_err(DisclosureError::Parse)?;
    if !value.is_object() {
        return Err(DisclosureError::InvalidRequestShape);
    }
    let request: Request = serde_json::from_value(value).map_err(DisclosureError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(DisclosureError::UnsupportedVersion);
    }
    if request.name.trim().is_empty() {
        return Err(DisclosureError::BlankName);
    }
    if request.controlled_content.identity.is_empty() {
        return Err(DisclosureError::EmptyControlledContentIdentity);
    }
    if !request.controlled_content.present {
        return Err(DisclosureError::StaleControlledContent);
    }
    let command = crate::command_accessibility::resolve_command_accessibility_name(
        &request.name,
        &request.activation,
        request.description.as_deref(),
        request.focusable,
    )
    .map_err(disclosure_command_error)?;
    resolve_disclosure_command(
        command,
        &request.activation,
        request.expanded,
        &request.controlled_content.identity,
        &request.event,
    )
}
