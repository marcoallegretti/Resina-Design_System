use resina_model::{ActivationEvent as E, ActivationKey as K, ActivationState, PressHold as H};
use resina_resolver::{CaptureChange, resolve_activation, resolve_activation_source};
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn public_cases_match_exact_results_and_diagnostics() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/activation-cases.json"
    ))
    .unwrap();
    for case in cases {
        let result = resolve_activation_source(&case["request"].to_string());
        if let Some(expected) = case.get("expected") {
            assert_eq!(
                serde_json::to_value(result.unwrap()).unwrap(),
                *expected,
                "{}",
                case["name"]
            );
        } else {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains(case["errorContains"].as_str().unwrap()),
                "{}",
                case["name"]
            );
        }
    }
}

#[test]
fn cancellation_and_supersession_prevent_later_release_activation() {
    let initial = ActivationState::try_new(true, true, None).unwrap();
    let pointer_down = E::PointerDown {
        id: "p".into(),
        inside: true,
    };
    let pointer_up = E::PointerUp {
        id: "p".into(),
        inside: true,
    };
    let space_down = E::KeyDown {
        key: K::Space,
        repeat: false,
    };
    let space_up = E::KeyUp { key: K::Space };
    for (down, up) in [(pointer_down, pointer_up), (space_down, space_up)] {
        for stop in [
            E::Cancel {},
            E::Availability { enabled: false },
            E::Invoke {},
        ] {
            let armed = resolve_activation(&initial, &down).unwrap();
            let stopped = resolve_activation(armed.state(), &stop).unwrap();
            assert_eq!(stopped.activate(), stop == E::Invoke {});
            assert!(stopped.state().hold().is_none());
            let reenabled =
                resolve_activation(stopped.state(), &E::Availability { enabled: true }).unwrap();
            assert!(
                !resolve_activation(reenabled.state(), &up)
                    .unwrap()
                    .activate()
            );
        }
    }
}

#[test]
fn every_small_state_and_event_obeys_activation_and_capture_invariants() {
    let mut states = Vec::new();
    for enabled in [false, true] {
        for focused in [false, true] {
            for hold in [
                None,
                Some(H::Pointer {
                    id: "p".into(),
                    inside: true,
                }),
                Some(H::Pointer {
                    id: "p".into(),
                    inside: false,
                }),
                Some(H::Key { key: K::Space }),
                Some(H::Key { key: K::Enter }),
            ] {
                if let Ok(state) = ActivationState::try_new(enabled, focused, hold) {
                    states.push(state);
                }
            }
        }
    }
    let mut events = vec![
        E::Cancel {},
        E::Invoke {},
        E::Focus { focused: true },
        E::Focus { focused: false },
        E::Availability { enabled: true },
        E::Availability { enabled: false },
    ];
    for id in ["p", "other"] {
        events.push(E::PointerCancel { id: id.into() });
        for inside in [false, true] {
            events.extend([
                E::PointerDown {
                    id: id.into(),
                    inside,
                },
                E::PointerMove {
                    id: id.into(),
                    inside,
                },
                E::PointerUp {
                    id: id.into(),
                    inside,
                },
            ]);
        }
    }
    for key in [K::Space, K::Enter] {
        events.extend([
            E::KeyDown { key, repeat: false },
            E::KeyDown { key, repeat: true },
            E::KeyUp { key },
        ]);
    }
    let pointer = |state: &ActivationState| match state.hold() {
        Some(H::Pointer { id, .. }) => Some(id.clone()),
        _ => None,
    };
    for state in states {
        for first in &events {
            let current = resolve_activation(&state, first).unwrap();
            for event in &events {
                let state = current.state();
                let expected = state.enabled()
                    && match event {
                        E::Invoke {} => true,
                        E::PointerUp { id, inside } => {
                            *inside && pointer(state).as_ref() == Some(id)
                        }
                        E::KeyDown {
                            key: K::Enter,
                            repeat: false,
                        } => state.focused() && state.hold().is_none(),
                        E::KeyUp { key: K::Space } => {
                            state.hold() == Some(&H::Key { key: K::Space })
                        }
                        _ => false,
                    };
                let result = resolve_activation(state, event).unwrap();
                assert_eq!(result.activate(), expected, "{state:?}, {event:?}");
                assert_eq!(
                    serde_json::from_value::<ActivationState>(
                        serde_json::to_value(result.state()).unwrap()
                    )
                    .unwrap(),
                    *result.state()
                );
                let before = pointer(state);
                let after = pointer(result.state());
                let expected_capture = if before == after {
                    None
                } else if let Some(id) = before {
                    Some(CaptureChange::Release { id })
                } else {
                    after.map(|id| CaptureChange::Acquire { id })
                };
                assert_eq!(result.capture(), expected_capture.as_ref());
            }
        }
    }
}

#[test]
fn typed_inputs_cannot_bypass_identity_and_hold_invariants() {
    assert!(ActivationState::try_new(false, true, Some(H::Key { key: K::Space })).is_err());
    assert!(ActivationState::try_new(true, false, Some(H::Key { key: K::Enter })).is_err());
    assert!(
        ActivationState::try_new(
            true,
            false,
            Some(H::Pointer {
                id: String::new(),
                inside: true
            })
        )
        .is_err()
    );
    let disabled = ActivationState::try_new(false, false, None).unwrap();
    assert!(
        resolve_activation(
            &disabled,
            &E::PointerDown {
                id: String::new(),
                inside: true
            }
        )
        .is_err()
    );
}

#[test]
fn cli_rejects_duplicate_members_invalid_utf8_and_bad_usage() {
    for source in [
        b"{\"schemaVersion\":\"0.1.0\",\"schemaVersion\":\"0.1.0\"}".as_slice(),
        &[0xff],
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_resina-activation"))
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(source).unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    assert_eq!(
        Command::new(env!("CARGO_BIN_EXE_resina-activation"))
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
}
