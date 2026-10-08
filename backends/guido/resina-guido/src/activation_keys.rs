use guido::widgets::{Event, Key, Modifiers};
use resina_model::{ActivationEvent, ActivationKey, ActivationState, PressHold};

/// Maps command key delivery; the owner resolves and commits the returned event.
/// Pointer, focus, availability and interruption routing remain owner obligations.
pub fn activation_key_event(state: &ActivationState, event: &Event) -> Option<ActivationEvent> {
    let (native_key, modifiers) = match event {
        Event::KeyDown { key, modifiers, .. } | Event::KeyUp { key, modifiers } => (key, modifiers),
        _ => return None,
    };
    let key = match native_key {
        Key::Char(' ') => ActivationKey::Space,
        Key::Enter => ActivationKey::Enter,
        _ => return None,
    };
    let unmodified = unmodified(modifiers);
    match event {
        Event::KeyDown { repeat, .. } if unmodified => Some(ActivationEvent::KeyDown {
            key,
            repeat: *repeat,
        }),
        Event::KeyUp { .. }
            if unmodified
                || matches!(state.hold(), Some(PressHold::Key { key: held }) if *held == key) =>
        {
            Some(ActivationEvent::KeyUp { key })
        }
        _ => None,
    }
}

fn unmodified(modifiers: &Modifiers) -> bool {
    // Caps Lock is a latch and does not modify either command key.
    !(modifiers.ctrl || modifiers.alt || modifiers.shift || modifiers.logo)
}
