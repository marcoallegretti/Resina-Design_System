use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivationKey {
    Space,
    Enter,
}

impl<'de> Deserialize<'de> for ActivationKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Key;

        impl Visitor<'_> for Key {
            type Value = ActivationKey;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("`space` or `enter`")
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                match value {
                    "space" => Ok(ActivationKey::Space),
                    "enter" => Ok(ActivationKey::Enter),
                    _ => Err(E::unknown_variant(value, &["space", "enter"])),
                }
            }
        }

        deserializer.deserialize_str(Key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum PressHold {
    Pointer { id: String, inside: bool },
    Key { key: ActivationKey },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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

fn deserialize_object<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Object<T>(std::marker::PhantomData<T>);

    impl<'de, T: Deserialize<'de>> Visitor<'de> for Object<T> {
        type Value = T;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an object")
        }

        fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
            T::deserialize(MapAccessDeserializer::new(map))
        }
    }

    deserializer.deserialize_map(Object(std::marker::PhantomData))
}

impl<'de> Deserialize<'de> for ActivationState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize_object::<D, StateInput>(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum HoldInput {
    Pointer { id: String, inside: bool },
    Key { key: ActivationKey },
}

impl<'de> Deserialize<'de> for PressHold {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match deserialize_object::<D, HoldInput>(deserializer)? {
            HoldInput::Pointer { id, inside } => Self::Pointer { id, inside },
            HoldInput::Key { key } => Self::Key { key },
        })
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum EventInput {
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

impl<'de> Deserialize<'de> for ActivationEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match deserialize_object::<D, EventInput>(deserializer)? {
            EventInput::PointerDown { id, inside } => Self::PointerDown { id, inside },
            EventInput::PointerMove { id, inside } => Self::PointerMove { id, inside },
            EventInput::PointerUp { id, inside } => Self::PointerUp { id, inside },
            EventInput::PointerCancel { id } => Self::PointerCancel { id },
            EventInput::KeyDown { key, repeat } => Self::KeyDown { key, repeat },
            EventInput::KeyUp { key } => Self::KeyUp { key },
            EventInput::Focus { focused } => Self::Focus { focused },
            EventInput::Availability { enabled } => Self::Availability { enabled },
            EventInput::Invoke {} => Self::Invoke {},
            EventInput::Cancel {} => Self::Cancel {},
        })
    }
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
