use crate::{CommandResponse, InteractionState, MaterialFamily, StateSet};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SliderAppearance {
    #[serde(deserialize_with = "crate::deserialize_version")]
    schema_version: String,
    track: Families,
    thumb: Families,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Families {
    cast: Phases,
    frost: Phases,
    elastomer: Phases,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Phases {
    hover: CommandResponse,
    pressed: CommandResponse,
    dragging: CommandResponse,
    disabled: CommandResponse,
    read_only: CommandResponse,
    read_only_hover: CommandResponse,
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
