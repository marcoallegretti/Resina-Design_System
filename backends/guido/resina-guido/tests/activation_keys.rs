use guido::widgets::{Event, Key, Modifiers};
use resina_guido::activation_key_event;
use resina_model::{ActivationEvent, ActivationKey, ActivationState, PressHold};
use resina_resolver::{resolve_activation, resolve_toggle_activation};

fn modifiers(bits: u8) -> Modifiers {
    Modifiers {
        ctrl: bits & 1 != 0,
        alt: bits & 2 != 0,
        shift: bits & 4 != 0,
        logo: bits & 8 != 0,
        caps_lock: bits & 16 != 0,
    }
}

#[test]
fn native_key_mapping_preserves_repeat_and_filters_command_chords() {
    let state = ActivationState::try_new(true, true, None).unwrap();
    for (key, expected) in [
        (Key::Char(' '), ActivationKey::Space),
        (Key::Enter, ActivationKey::Enter),
    ] {
        for bits in 0..32 {
            for repeat in [false, true] {
                let event = Event::KeyDown {
                    key,
                    modifiers: modifiers(bits),
                    repeat,
                };
                let mapped = activation_key_event(&state, &event);
                if bits & 15 == 0 {
                    assert!(
                        matches!(mapped, Some(ActivationEvent::KeyDown { key, repeat: actual }) if key == expected && actual == repeat)
                    );
                } else {
                    assert!(mapped.is_none());
                }
            }
        }
    }
    for key in [
        Key::Tab,
        Key::Escape,
        Key::Char('x'),
        Key::Char('\n'),
        Key::Char('\u{a0}'),
    ] {
        assert!(
            activation_key_event(
                &state,
                &Event::KeyDown {
                    key,
                    modifiers: Modifiers::default(),
                    repeat: false
                }
            )
            .is_none()
        );
        assert!(
            activation_key_event(
                &state,
                &Event::KeyUp {
                    key,
                    modifiers: Modifiers::default()
                }
            )
            .is_none()
        );
    }
    assert!(activation_key_event(&state, &Event::FocusOut).is_none());
    assert!(
        activation_key_event(
            &state,
            &Event::mouse_down(1.0, 1.0, guido::widgets::MouseButton::Left)
        )
        .is_none()
    );
}

#[test]
fn held_command_keys_terminate_after_any_modifier_change() {
    for (key, held) in [
        (Key::Char(' '), ActivationKey::Space),
        (Key::Enter, ActivationKey::Enter),
    ] {
        let state =
            ActivationState::try_new(true, true, Some(PressHold::Key { key: held })).unwrap();
        for bits in 0..32 {
            let mapped = activation_key_event(
                &state,
                &Event::KeyUp {
                    key,
                    modifiers: modifiers(bits),
                },
            )
            .unwrap();
            assert!(matches!(mapped, ActivationEvent::KeyUp { key } if key == held));
            let result = resolve_activation(&state, &mapped).unwrap();
            assert!(result.state().hold().is_none());
            assert_eq!(result.activate(), held == ActivationKey::Space);
            assert!(result.capture().is_none());
        }
    }
    for hold in [
        None,
        Some(PressHold::Key {
            key: ActivationKey::Enter,
        }),
        Some(PressHold::Pointer {
            id: "primary".into(),
            inside: true,
        }),
    ] {
        let state = ActivationState::try_new(true, true, hold).unwrap();
        assert!(
            activation_key_event(
                &state,
                &Event::KeyUp {
                    key: Key::Char(' '),
                    modifiers: modifiers(1)
                }
            )
            .is_none()
        );
    }
}

#[test]
fn mapped_native_sequences_activate_command_and_toggle_once() {
    for key in [Key::Char(' '), Key::Enter] {
        let mut state = ActivationState::try_new(true, true, None).unwrap();
        let mut toggle_state = state.clone();
        let mut activations = 0;
        let mut checked = false;
        for event in [
            Event::KeyDown {
                key,
                modifiers: Modifiers::default(),
                repeat: false,
            },
            Event::KeyDown {
                key,
                modifiers: Modifiers::default(),
                repeat: true,
            },
            Event::KeyDown {
                key,
                modifiers: Modifiers::default(),
                repeat: true,
            },
            Event::KeyUp {
                key,
                modifiers: modifiers(15),
            },
            Event::KeyUp {
                key,
                modifiers: Modifiers::default(),
            },
        ] {
            let mapped = activation_key_event(&state, &event).unwrap();
            let result = resolve_activation(&state, &mapped).unwrap();
            activations += usize::from(result.activate());
            state = result.state().clone();
            let mapped = activation_key_event(&toggle_state, &event).unwrap();
            let result = resolve_toggle_activation(&toggle_state, checked, &mapped).unwrap();
            toggle_state = result.activation().state().clone();
            checked = result.checked();
        }
        assert_eq!(activations, 1);
        assert!(state.hold().is_none());
        assert!(checked);
        assert_eq!(toggle_state, state);
    }
}

#[test]
fn owner_cancellation_and_eligibility_remain_resolver_decisions() {
    for state in [
        ActivationState::try_new(false, true, None).unwrap(),
        ActivationState::try_new(true, false, None).unwrap(),
    ] {
        let event = activation_key_event(
            &state,
            &Event::KeyDown {
                key: Key::Enter,
                modifiers: Modifiers::default(),
                repeat: false,
            },
        )
        .unwrap();
        let result = resolve_activation(&state, &event).unwrap();
        assert!(!result.activate());
        assert_eq!(result.state(), &state);
    }
    for cancellation in [
        ActivationEvent::Focus { focused: false },
        ActivationEvent::Availability { enabled: false },
        ActivationEvent::Cancel {},
    ] {
        let state = ActivationState::try_new(
            true,
            true,
            Some(PressHold::Key {
                key: ActivationKey::Space,
            }),
        )
        .unwrap();
        let result = resolve_activation(&state, &cancellation).unwrap();
        assert!(!result.activate());
        assert!(result.state().hold().is_none());
        assert!(
            activation_key_event(
                result.state(),
                &Event::KeyUp {
                    key: Key::Char(' '),
                    modifiers: modifiers(1)
                }
            )
            .is_none()
        );
        let release = activation_key_event(
            result.state(),
            &Event::KeyUp {
                key: Key::Char(' '),
                modifiers: Modifiers::default(),
            },
        )
        .unwrap();
        assert!(
            !resolve_activation(result.state(), &release)
                .unwrap()
                .activate()
        );
    }
}
