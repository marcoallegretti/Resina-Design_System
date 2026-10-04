use crate::CommandLabelIr;
use resina_model::ActivationState;
use serde::Serialize;
use std::fmt;

pub struct CommandAccessibilityInput<'a> {
    pub label: &'a CommandLabelIr,
    pub activation: &'a ActivationState,
    pub description: Option<&'a str>,
    pub focusable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CommandAccessibilityState {
    enabled: bool,
    focused: bool,
}
impl CommandAccessibilityState {
    pub fn enabled(self) -> bool {
        self.enabled
    }
    pub fn focused(self) -> bool {
        self.focused
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CommandAccessibilityAction {
    kind: &'static str,
    available: bool,
}
impl CommandAccessibilityAction {
    pub fn kind(self) -> &'static str {
        self.kind
    }
    pub fn available(self) -> bool {
        self.available
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandAccessibilityIr {
    schema_version: &'static str,
    role: &'static str,
    name: String,
    description: Option<String>,
    value: (),
    state: CommandAccessibilityState,
    actions: [CommandAccessibilityAction; 1],
    focusable: bool,
    relationships: [(); 0],
}
impl CommandAccessibilityIr {
    pub fn role(&self) -> &'static str {
        self.role
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    pub fn state(&self) -> CommandAccessibilityState {
        self.state
    }
    pub fn actions(&self) -> &[CommandAccessibilityAction; 1] {
        &self.actions
    }
    pub fn focusable(&self) -> bool {
        self.focusable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAccessibilityError {
    BlankDescription,
    EnabledNotFocusable,
    FocusedNotFocusable,
}
impl fmt::Display for CommandAccessibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BlankDescription => "command description must be nonblank or absent",
            Self::EnabledNotFocusable => "enabled command must be focusable",
            Self::FocusedNotFocusable => "actual command focus requires focusability",
        })
    }
}
impl std::error::Error for CommandAccessibilityError {}

pub fn resolve_command_accessibility(
    input: CommandAccessibilityInput<'_>,
) -> Result<CommandAccessibilityIr, CommandAccessibilityError> {
    if input.description.is_some_and(|text| text.trim().is_empty()) {
        return Err(CommandAccessibilityError::BlankDescription);
    }
    if !input.focusable {
        if input.activation.enabled() {
            return Err(CommandAccessibilityError::EnabledNotFocusable);
        }
        if input.activation.focused() {
            return Err(CommandAccessibilityError::FocusedNotFocusable);
        }
    }
    Ok(CommandAccessibilityIr {
        schema_version: "0.1.0",
        role: "button",
        name: input.label.text().to_owned(),
        description: input.description.map(str::to_owned),
        value: (),
        state: CommandAccessibilityState {
            enabled: input.activation.enabled(),
            focused: input.activation.focused(),
        },
        actions: [CommandAccessibilityAction {
            kind: "invoke",
            available: input.activation.enabled(),
        }],
        focusable: input.focusable,
        relationships: [],
    })
}
