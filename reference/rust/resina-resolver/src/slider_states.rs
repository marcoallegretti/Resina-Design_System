use crate::{SliderPointerPhase, SliderPointerState, SliderPresentation, SliderPresentationError};
use resina_model::{InteractionState, StateSet, StateSetInput};
use std::fmt;

pub struct SliderStatesInput<'a> {
    pub presentation: &'a SliderPresentation,
    pub pointer: &'a SliderPointerState,
    pub enabled: bool,
    pub read_only: bool,
    pub focused: bool,
    pub hovered: bool,
    pub key_pressed: bool,
}

#[derive(Debug)]
pub enum SliderStatesError {
    Presentation(SliderPresentationError),
    IncoherentPresentation,
    UnavailableHold,
    UnfocusedKeyHold,
}
impl fmt::Display for SliderStatesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Presentation(error) => write!(f, "slider states: {error}"),
            Self::IncoherentPresentation => {
                f.write_str("slider states require the pointer's current presentation")
            }
            Self::UnavailableHold => {
                f.write_str("slider states cannot retain a press while disabled or read-only")
            }
            Self::UnfocusedKeyHold => f.write_str("slider key press requires actual focus"),
        }
    }
}
impl std::error::Error for SliderStatesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Presentation(error) => Some(error),
            _ => None,
        }
    }
}

pub fn resolve_slider_states(input: SliderStatesInput<'_>) -> Result<StateSet, SliderStatesError> {
    let presentation = input.presentation;
    let pointer_presentation = SliderPresentation::try_new(
        presentation.committed(),
        presentation.revision(),
        presentation.value_policy(),
        input.pointer.edit(),
    )
    .map_err(SliderStatesError::Presentation)?;
    if &pointer_presentation != presentation {
        return Err(SliderStatesError::IncoherentPresentation);
    }
    let pointer_pressed = input.pointer.edit().is_some();
    if (pointer_pressed || input.key_pressed) && (!input.enabled || input.read_only) {
        return Err(SliderStatesError::UnavailableHold);
    }
    if input.key_pressed && !input.focused {
        return Err(SliderStatesError::UnfocusedKeyHold);
    }
    let mut states = Vec::new();
    if !input.enabled {
        states.push(InteractionState::Disabled);
    }
    if input.hovered {
        states.push(InteractionState::Hover);
    }
    if pointer_pressed || input.key_pressed {
        states.push(InteractionState::Pressed);
    }
    if input.pointer.phase() == Some(SliderPointerPhase::Acquired) {
        states.push(InteractionState::Dragging);
    }
    if states.is_empty() {
        states.push(InteractionState::Rest);
    }
    if input.focused {
        states.push(InteractionState::Focused);
    }
    Ok(StateSet::try_from(StateSetInput {
        schema_version: "0.1.0".to_owned(),
        states,
    })
    .expect("slider signals are unique and nonempty"))
}
