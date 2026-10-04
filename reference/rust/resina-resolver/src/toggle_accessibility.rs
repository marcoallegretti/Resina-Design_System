use crate::CommandLabelIr;
use resina_model::ActivationState;
use serde::Serialize;
use std::fmt;

pub struct ToggleAccessibilityInput<'a> {
    pub label: &'a CommandLabelIr,
    pub activation: &'a ActivationState,
    pub description: Option<&'a str>,
    pub focusable: bool,
    pub checked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ToggleAccessibilityState {
    enabled: bool,
    focused: bool,
    checked: bool,
}
impl ToggleAccessibilityState {
    pub fn checked(self) -> bool {
        self.checked
    }
    pub fn enabled(self) -> bool {
        self.enabled
    }
    pub fn focused(self) -> bool {
        self.focused
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ToggleAccessibilityAction {
    kind: &'static str,
    available: bool,
}
impl ToggleAccessibilityAction {
    pub fn kind(self) -> &'static str {
        self.kind
    }
    pub fn available(self) -> bool {
        self.available
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleAccessibilityIr {
    schema_version: &'static str,
    role: &'static str,
    name: String,
    description: Option<String>,
    value: (),
    state: ToggleAccessibilityState,
    actions: [ToggleAccessibilityAction; 1],
    focusable: bool,
    relationships: [(); 0],
}
impl ToggleAccessibilityIr {
    pub fn role(&self) -> &'static str {
        self.role
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    pub fn state(&self) -> ToggleAccessibilityState {
        self.state
    }
    pub fn actions(&self) -> &[ToggleAccessibilityAction; 1] {
        &self.actions
    }
    pub fn focusable(&self) -> bool {
        self.focusable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleAccessibilityError {
    BlankDescription,
    EnabledNotFocusable,
    FocusedNotFocusable,
}
impl fmt::Display for ToggleAccessibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BlankDescription => "toggle description must be nonblank or absent",
            Self::EnabledNotFocusable => "enabled toggle must be focusable",
            Self::FocusedNotFocusable => "actual toggle focus requires focusability",
        })
    }
}
impl std::error::Error for ToggleAccessibilityError {}

pub fn resolve_toggle_accessibility(
    input: ToggleAccessibilityInput<'_>,
) -> Result<ToggleAccessibilityIr, ToggleAccessibilityError> {
    if input.description.is_some_and(|text| text.trim().is_empty()) {
        return Err(ToggleAccessibilityError::BlankDescription);
    }
    if !input.focusable {
        if input.activation.enabled() {
            return Err(ToggleAccessibilityError::EnabledNotFocusable);
        }
        if input.activation.focused() {
            return Err(ToggleAccessibilityError::FocusedNotFocusable);
        }
    }
    Ok(ToggleAccessibilityIr {
        schema_version: "0.1.0",
        role: "switch",
        name: input.label.text().to_owned(),
        description: input.description.map(str::to_owned),
        value: (),
        state: ToggleAccessibilityState {
            enabled: input.activation.enabled(),
            focused: input.activation.focused(),
            checked: input.checked,
        },
        actions: [ToggleAccessibilityAction {
            kind: "invoke",
            available: input.activation.enabled(),
        }],
        focusable: input.focusable,
        relationships: [],
    })
}
