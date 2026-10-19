use crate::CommandLabelIr;
use resina_model::{ActivationState, CommandContent, CommandContentForm};
use serde::Serialize;
use std::fmt;

pub struct CommandAccessibilityInput<'a> {
    pub label: &'a CommandLabelIr,
    pub activation: &'a ActivationState,
    pub description: Option<&'a str>,
    pub focusable: bool,
}

pub struct CommandContentAccessibilityInput<'a> {
    pub content: &'a CommandContent,
    pub label: Option<&'a CommandLabelIr>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandContentAccessibilityError {
    LabelForm,
    MissingLabelLayout,
    UnexpectedLabelLayout,
    LabelLayoutMismatch,
    BlankDescription,
    EnabledNotFocusable,
    FocusedNotFocusable,
}
impl fmt::Display for CommandContentAccessibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LabelForm => "label content resolves through ordinary command accessibility",
            Self::MissingLabelLayout => "icon and label content requires its laid-out label",
            Self::UnexpectedLabelLayout => "icon only content has no laid-out label",
            Self::LabelLayoutMismatch => "laid-out label text differs from the content label",
            Self::BlankDescription => "command description must be nonblank or absent",
            Self::EnabledNotFocusable => "enabled command must be focusable",
            Self::FocusedNotFocusable => "actual command focus requires focusability",
        })
    }
}
impl std::error::Error for CommandContentAccessibilityError {}
impl From<CommandAccessibilityError> for CommandContentAccessibilityError {
    fn from(error: CommandAccessibilityError) -> Self {
        match error {
            CommandAccessibilityError::BlankDescription => Self::BlankDescription,
            CommandAccessibilityError::EnabledNotFocusable => Self::EnabledNotFocusable,
            CommandAccessibilityError::FocusedNotFocusable => Self::FocusedNotFocusable,
        }
    }
}

pub fn resolve_command_accessibility(
    input: CommandAccessibilityInput<'_>,
) -> Result<CommandAccessibilityIr, CommandAccessibilityError> {
    snapshot(
        input.label.text(),
        input.activation,
        input.description,
        input.focusable,
    )
}

pub fn resolve_command_content_accessibility(
    input: CommandContentAccessibilityInput<'_>,
) -> Result<CommandAccessibilityIr, CommandContentAccessibilityError> {
    match (input.content.form(), input.label) {
        (CommandContentForm::Label, _) => {
            return Err(CommandContentAccessibilityError::LabelForm);
        }
        (CommandContentForm::IconAndLabel, None) => {
            return Err(CommandContentAccessibilityError::MissingLabelLayout);
        }
        (CommandContentForm::IconAndLabel, Some(label)) if label.text() != input.content.text() => {
            return Err(CommandContentAccessibilityError::LabelLayoutMismatch);
        }
        (CommandContentForm::IconOnly, Some(_)) => {
            return Err(CommandContentAccessibilityError::UnexpectedLabelLayout);
        }
        _ => {}
    }
    Ok(snapshot(
        input.content.text(),
        input.activation,
        input.description,
        input.focusable,
    )?)
}

fn snapshot(
    name: &str,
    activation: &ActivationState,
    description: Option<&str>,
    focusable: bool,
) -> Result<CommandAccessibilityIr, CommandAccessibilityError> {
    if description.is_some_and(|text| text.trim().is_empty()) {
        return Err(CommandAccessibilityError::BlankDescription);
    }
    if !focusable {
        if activation.enabled() {
            return Err(CommandAccessibilityError::EnabledNotFocusable);
        }
        if activation.focused() {
            return Err(CommandAccessibilityError::FocusedNotFocusable);
        }
    }
    Ok(CommandAccessibilityIr {
        schema_version: "0.1.0",
        role: "button",
        name: name.to_owned(),
        description: description.map(str::to_owned),
        value: (),
        state: CommandAccessibilityState {
            enabled: activation.enabled(),
            focused: activation.focused(),
        },
        actions: [CommandAccessibilityAction {
            kind: "invoke",
            available: activation.enabled(),
        }],
        focusable,
        relationships: [],
    })
}
