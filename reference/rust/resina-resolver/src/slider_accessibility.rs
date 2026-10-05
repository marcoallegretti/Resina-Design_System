use crate::{CommandLabelIr, SliderValueIr};
use resina_model::SliderValue;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SliderOrientation {
    Horizontal,
    Vertical,
}
pub struct SliderAccessibilityInput<'a> {
    pub label: &'a CommandLabelIr,
    pub value: &'a SliderValueIr,
    pub description: Option<&'a str>,
    pub value_text: Option<&'a str>,
    pub enabled: bool,
    pub read_only: bool,
    pub focused: bool,
    pub focusable: bool,
    pub orientation: SliderOrientation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliderAccessibilityState {
    enabled: bool,
    read_only: bool,
    focused: bool,
}
impl SliderAccessibilityState {
    pub fn enabled(self) -> bool {
        self.enabled
    }
    pub fn read_only(self) -> bool {
        self.read_only
    }
    pub fn focused(self) -> bool {
        self.focused
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SliderAccessibilityAction {
    kind: &'static str,
    available: bool,
}
impl SliderAccessibilityAction {
    pub fn kind(self) -> &'static str {
        self.kind
    }
    pub fn available(self) -> bool {
        self.available
    }
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliderAccessibilityIr {
    schema_version: &'static str,
    role: &'static str,
    name: String,
    description: Option<String>,
    value: SliderValue,
    value_text: Option<String>,
    state: SliderAccessibilityState,
    orientation: SliderOrientation,
    actions: [SliderAccessibilityAction; 3],
    focusable: bool,
    relationships: [(); 0],
}
impl SliderAccessibilityIr {
    pub fn role(&self) -> &'static str {
        self.role
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    pub fn value(&self) -> &SliderValue {
        &self.value
    }
    pub fn value_text(&self) -> Option<&str> {
        self.value_text.as_deref()
    }
    pub fn state(&self) -> SliderAccessibilityState {
        self.state
    }
    pub fn orientation(&self) -> SliderOrientation {
        self.orientation
    }
    pub fn actions(&self) -> &[SliderAccessibilityAction; 3] {
        &self.actions
    }
    pub fn focusable(&self) -> bool {
        self.focusable
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderAccessibilityError {
    BlankDescription,
    BlankValueText,
    EnabledNotFocusable,
    FocusedNotFocusable,
}
impl fmt::Display for SliderAccessibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BlankDescription => "slider description must be nonblank or absent",
            Self::BlankValueText => "slider value text must be nonblank or absent",
            Self::EnabledNotFocusable => "enabled slider must be focusable",
            Self::FocusedNotFocusable => "actual slider focus requires focusability",
        })
    }
}
impl std::error::Error for SliderAccessibilityError {}

pub fn resolve_slider_accessibility(
    input: SliderAccessibilityInput<'_>,
) -> Result<SliderAccessibilityIr, SliderAccessibilityError> {
    if input.description.is_some_and(|text| text.trim().is_empty()) {
        return Err(SliderAccessibilityError::BlankDescription);
    }
    if input.value_text.is_some_and(|text| text.trim().is_empty()) {
        return Err(SliderAccessibilityError::BlankValueText);
    }
    if !input.focusable {
        if input.enabled {
            return Err(SliderAccessibilityError::EnabledNotFocusable);
        }
        if input.focused {
            return Err(SliderAccessibilityError::FocusedNotFocusable);
        }
    }
    let available = input.enabled && !input.read_only;
    Ok(SliderAccessibilityIr {
        schema_version: "0.1.0",
        role: "slider",
        name: input.label.text().to_owned(),
        description: input.description.map(str::to_owned),
        value: *input.value.value(),
        value_text: input.value_text.map(str::to_owned),
        state: SliderAccessibilityState {
            enabled: input.enabled,
            read_only: input.read_only,
            focused: input.focused,
        },
        orientation: input.orientation,
        actions: ["setValue", "increase", "decrease"]
            .map(|kind| SliderAccessibilityAction { kind, available }),
        focusable: input.focusable,
        relationships: [],
    })
}
