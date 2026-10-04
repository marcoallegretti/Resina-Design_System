use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivationKey {
    Space,
    Enter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum PressHold {
    Pointer { id: String, inside: bool },
    Key { key: ActivationKey },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "StateInput")]
pub struct ActivationState {
    schema_version: String,
    enabled: bool,
    focused: bool,
    hold: Option<PressHold>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StateInput {
    schema_version: String,
    enabled: bool,
    focused: bool,
    // Ordinary Option deserialization treats an absent field as null.
    #[serde(deserialize_with = "deserialize_hold")]
    hold: Option<PressHold>,
}

fn deserialize_hold<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<PressHold>, D::Error> {
    Option::<PressHold>::deserialize(deserializer)
}

impl TryFrom<StateInput> for ActivationState {
    type Error = &'static str;

    fn try_from(input: StateInput) -> Result<Self, Self::Error> {
        if input.schema_version != "0.1.0" {
            return Err("schemaVersion must be 0.1.0");
        }
        Self::try_new(input.enabled, input.focused, input.hold)
    }
}

impl ActivationState {
    pub fn try_new(
        enabled: bool,
        focused: bool,
        hold: Option<PressHold>,
    ) -> Result<Self, &'static str> {
        if let Some(PressHold::Pointer { id, .. }) = &hold
            && id.is_empty()
        {
            return Err("pointer ID must not be empty");
        }
        if !enabled && hold.is_some() {
            return Err("disabled state must not hold a press");
        }
        if !focused && matches!(hold, Some(PressHold::Key { .. })) {
            return Err("keyboard hold requires actual focus");
        }
        Ok(Self {
            schema_version: "0.1.0".to_owned(),
            enabled,
            focused,
            hold,
        })
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn focused(&self) -> bool {
        self.focused
    }
    pub fn hold(&self) -> Option<&PressHold> {
        self.hold.as_ref()
    }
    pub fn pressed(&self) -> bool {
        matches!(
            self.hold,
            Some(PressHold::Pointer { inside: true, .. } | PressHold::Key { .. })
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ActivationEvent {
    PointerDown { id: String, inside: bool },
    PointerMove { id: String, inside: bool },
    PointerUp { id: String, inside: bool },
    PointerCancel { id: String },
    KeyDown { key: ActivationKey, repeat: bool },
    KeyUp { key: ActivationKey },
    Focus { focused: bool },
    Availability { enabled: bool },
    Invoke {},
    Cancel {},
}

impl ActivationEvent {
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::PointerDown { id, .. }
            | Self::PointerMove { id, .. }
            | Self::PointerUp { id, .. }
            | Self::PointerCancel { id }
                if id.is_empty() =>
            {
                Err("pointer ID must not be empty")
            }
            _ => Ok(()),
        }
    }
}
