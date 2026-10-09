use crate::{CommandResponse, InteractionState, MaterialFamily, StateSet};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SliderPart {
    Track,
    Thumb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SliderPhase {
    Rest,
    Hover,
    Pressed,
    Dragging,
    Disabled,
    ReadOnly,
    ReadOnlyHover,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliderAppearance {
    schema_version: String,
    track: Families,
    thumb: Families,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SliderAppearanceMembers {
    #[serde(deserialize_with = "crate::deserialize_version")]
    schema_version: String,
    track: Families,
    thumb: Families,
}

impl<'de> Deserialize<'de> for SliderAppearance {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct AppearanceVisitor;
        impl<'de> Visitor<'de> for AppearanceVisitor {
            type Value = SliderAppearance;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a slider appearance object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let input = SliderAppearanceMembers::deserialize(MapAccessDeserializer::new(map))?;
                Ok(SliderAppearance {
                    schema_version: input.schema_version,
                    track: input.track,
                    thumb: input.thumb,
                })
            }
        }
        deserializer.deserialize_map(AppearanceVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct Families {
    cast: Phases,
    frost: Phases,
    elastomer: Phases,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FamiliesMembers {
    cast: Phases,
    frost: Phases,
    elastomer: Phases,
}

impl<'de> Deserialize<'de> for Families {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FamiliesVisitor;
        impl<'de> Visitor<'de> for FamiliesVisitor {
            type Value = Families;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a slider material family object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let input = FamiliesMembers::deserialize(MapAccessDeserializer::new(map))?;
                Ok(Families {
                    cast: input.cast,
                    frost: input.frost,
                    elastomer: input.elastomer,
                })
            }
        }
        deserializer.deserialize_map(FamiliesVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Phases {
    hover: CommandResponse,
    pressed: CommandResponse,
    dragging: CommandResponse,
    disabled: CommandResponse,
    read_only: CommandResponse,
    read_only_hover: CommandResponse,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PhasesMembers {
    hover: CommandResponse,
    pressed: CommandResponse,
    dragging: CommandResponse,
    disabled: CommandResponse,
    read_only: CommandResponse,
    read_only_hover: CommandResponse,
}

impl<'de> Deserialize<'de> for Phases {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PhasesVisitor;
        impl<'de> Visitor<'de> for PhasesVisitor {
            type Value = Phases;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a slider phase collection object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let input = PhasesMembers::deserialize(MapAccessDeserializer::new(map))?;
                Ok(Phases {
                    hover: input.hover,
                    pressed: input.pressed,
                    dragging: input.dragging,
                    disabled: input.disabled,
                    read_only: input.read_only,
                    read_only_hover: input.read_only_hover,
                })
            }
        }
        deserializer.deserialize_map(PhasesVisitor)
    }
}

impl SliderAppearance {
    pub fn response_for(
        &self,
        part: SliderPart,
        family: MaterialFamily,
        phase: SliderPhase,
    ) -> Result<CommandResponse, &'static str> {
        let families = match part {
            SliderPart::Track => &self.track,
            SliderPart::Thumb => &self.thumb,
        };
        let phases = match family {
            MaterialFamily::Cast => &families.cast,
            MaterialFamily::Frost => &families.frost,
            MaterialFamily::Elastomer => &families.elastomer,
            MaterialFamily::Gel => return Err("Gel cannot be a persistent slider part material"),
        };
        match phase {
            SliderPhase::Rest => CommandResponse::try_new(0.0, 1.0),
            SliderPhase::Hover => Ok(phases.hover),
            SliderPhase::Pressed => Ok(phases.pressed),
            SliderPhase::Dragging => Ok(phases.dragging),
            SliderPhase::Disabled => Ok(phases.disabled),
            SliderPhase::ReadOnly => Ok(phases.read_only),
            SliderPhase::ReadOnlyHover => Ok(phases.read_only_hover),
        }
    }
}

pub fn resolve_slider_phase(
    states: &StateSet,
    read_only: bool,
) -> Result<SliderPhase, &'static str> {
    use InteractionState::{Disabled, Dragging, Focused, Hover, Pressed, Rest};
    if states.states().iter().any(|state| {
        !matches!(
            state,
            Rest | Hover | Focused | Pressed | Disabled | Dragging
        )
    }) {
        return Err(
            "slider appearance supports only rest, hover, focused, pressed, disabled and dragging states",
        );
    }
    let disabled = states.contains(Disabled);
    let pressed = states.contains(Pressed);
    let dragging = states.contains(Dragging);
    let hovered = states.contains(Hover);
    if (disabled || read_only) && (pressed || dragging) {
        return Err("disabled or read-only slider appearance cannot retain a press or drag");
    }
    if dragging && !pressed {
        return Err("slider dragging appearance requires pressed state");
    }
    if states.contains(Rest) != (!disabled && !hovered && !pressed) {
        return Err("slider rest state must match its current body interaction");
    }
    Ok(if disabled {
        SliderPhase::Disabled
    } else if dragging {
        SliderPhase::Dragging
    } else if pressed {
        SliderPhase::Pressed
    } else if read_only && hovered {
        SliderPhase::ReadOnlyHover
    } else if read_only {
        SliderPhase::ReadOnly
    } else if hovered {
        SliderPhase::Hover
    } else {
        SliderPhase::Rest
    })
}
