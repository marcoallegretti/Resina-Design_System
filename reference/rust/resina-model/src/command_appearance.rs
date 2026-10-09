use crate::{InteractionState, MaterialFamily, StateSet};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandPhase {
    Rest,
    Hover,
    Pressed,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "ResponseInput")]
pub struct CommandResponse {
    body_mix: f64,
    depth_scale: f64,
}

struct ResponseInput {
    body_mix: f64,
    depth_scale: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResponseMembers {
    body_mix: f64,
    depth_scale: f64,
}

impl<'de> Deserialize<'de> for ResponseInput {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ResponseVisitor;
        impl<'de> Visitor<'de> for ResponseVisitor {
            type Value = ResponseInput;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a command response object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let input = ResponseMembers::deserialize(MapAccessDeserializer::new(map))?;
                Ok(ResponseInput {
                    body_mix: input.body_mix,
                    depth_scale: input.depth_scale,
                })
            }
        }
        deserializer.deserialize_map(ResponseVisitor)
    }
}
impl TryFrom<ResponseInput> for CommandResponse {
    type Error = &'static str;
    fn try_from(input: ResponseInput) -> Result<Self, Self::Error> {
        Self::try_new(input.body_mix, input.depth_scale)
    }
}
impl CommandResponse {
    pub fn try_new(body_mix: f64, depth_scale: f64) -> Result<Self, &'static str> {
        if !body_mix.is_finite() || !(-1.0..=1.0).contains(&body_mix) {
            return Err("bodyMix must be finite and in [-1, 1]");
        }
        if !depth_scale.is_finite() || !(0.0..=1.0).contains(&depth_scale) {
            return Err("depthScale must be finite and in [0, 1]");
        }
        Ok(Self {
            body_mix,
            depth_scale,
        })
    }
    pub fn body_mix(&self) -> f64 {
        self.body_mix
    }
    pub fn depth_scale(&self) -> f64 {
        self.depth_scale
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandAppearance {
    schema_version: String,
    profiles: Profiles,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CommandAppearanceMembers {
    #[serde(deserialize_with = "crate::deserialize_version")]
    schema_version: String,
    profiles: Profiles,
}

impl<'de> Deserialize<'de> for CommandAppearance {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct AppearanceVisitor;
        impl<'de> Visitor<'de> for AppearanceVisitor {
            type Value = CommandAppearance;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a command appearance object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let input = CommandAppearanceMembers::deserialize(MapAccessDeserializer::new(map))?;
                Ok(CommandAppearance {
                    schema_version: input.schema_version,
                    profiles: input.profiles,
                })
            }
        }
        deserializer.deserialize_map(AppearanceVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct Profiles {
    cast: Phases,
    frost: Phases,
    elastomer: Phases,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilesMembers {
    cast: Phases,
    frost: Phases,
    elastomer: Phases,
}

impl<'de> Deserialize<'de> for Profiles {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ProfilesVisitor;
        impl<'de> Visitor<'de> for ProfilesVisitor {
            type Value = Profiles;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a command material family object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let input = ProfilesMembers::deserialize(MapAccessDeserializer::new(map))?;
                Ok(Profiles {
                    cast: input.cast,
                    frost: input.frost,
                    elastomer: input.elastomer,
                })
            }
        }
        deserializer.deserialize_map(ProfilesVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct Phases {
    hover: CommandResponse,
    pressed: CommandResponse,
    disabled: CommandResponse,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PhasesMembers {
    hover: CommandResponse,
    pressed: CommandResponse,
    disabled: CommandResponse,
}

impl<'de> Deserialize<'de> for Phases {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PhasesVisitor;
        impl<'de> Visitor<'de> for PhasesVisitor {
            type Value = Phases;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a command phase collection object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let input = PhasesMembers::deserialize(MapAccessDeserializer::new(map))?;
                Ok(Phases {
                    hover: input.hover,
                    pressed: input.pressed,
                    disabled: input.disabled,
                })
            }
        }
        deserializer.deserialize_map(PhasesVisitor)
    }
}

impl CommandAppearance {
    pub fn response_for(
        &self,
        family: MaterialFamily,
        phase: CommandPhase,
    ) -> Result<CommandResponse, &'static str> {
        let phases = match family {
            MaterialFamily::Cast => &self.profiles.cast,
            MaterialFamily::Frost => &self.profiles.frost,
            MaterialFamily::Elastomer => &self.profiles.elastomer,
            MaterialFamily::Gel => return Err("Gel cannot be a command material"),
        };
        Ok(match phase {
            CommandPhase::Rest => CommandResponse {
                body_mix: 0.0,
                depth_scale: 1.0,
            },
            CommandPhase::Hover => phases.hover,
            CommandPhase::Pressed => phases.pressed,
            CommandPhase::Disabled => phases.disabled,
        })
    }
}

pub fn resolve_command_phase(states: &StateSet) -> Result<CommandPhase, &'static str> {
    let states = states.states();
    if states.iter().any(|state| {
        !matches!(
            state,
            InteractionState::Rest
                | InteractionState::Hover
                | InteractionState::Pressed
                | InteractionState::Disabled
                | InteractionState::Focused
        )
    }) {
        return Err(
            "command paint supports only rest, hover, pressed, disabled and focused states",
        );
    }
    Ok(if states.contains(&InteractionState::Disabled) {
        CommandPhase::Disabled
    } else if states.contains(&InteractionState::Pressed) {
        CommandPhase::Pressed
    } else if states.contains(&InteractionState::Hover) {
        CommandPhase::Hover
    } else {
        CommandPhase::Rest
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_responses_reject_nonfinite_channels() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                CommandResponse::try_from(ResponseInput {
                    body_mix: value,
                    depth_scale: 1.0
                })
                .is_err()
            );
            assert!(
                CommandResponse::try_from(ResponseInput {
                    body_mix: 0.0,
                    depth_scale: value
                })
                .is_err()
            );
        }
    }
    #[test]
    fn authored_profiles_round_trip_without_inferred_families() {
        for source in [
            include_str!("../../../../definitions/command-appearance-light.json"),
            include_str!("../../../../definitions/command-appearance-dark.json"),
        ] {
            let profiles: CommandAppearance = serde_json::from_str(source).unwrap();
            assert_eq!(
                serde_json::from_value::<CommandAppearance>(
                    serde_json::to_value(&profiles).unwrap()
                )
                .unwrap(),
                profiles
            );
            assert!(
                profiles
                    .response_for(MaterialFamily::Gel, CommandPhase::Rest)
                    .is_err()
            );
        }
    }
}
