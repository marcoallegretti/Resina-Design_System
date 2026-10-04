use resina_model::{
    ActivationEvent as E, ActivationState, InteractionState as S, StateLayer, resolve_command_phase,
};
use resina_resolver::{
    resolve_toggle_activation, resolve_toggle_states, resolve_toggle_states_source,
};
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn public_cases_preserve_exact_selection_and_interaction_or_fail() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/toggle-states-cases.json"
    ))
    .unwrap();
    for case in cases {
        let result = resolve_toggle_states_source(&case["request"].to_string());
        if let Some(expected) = case.get("expected") {
            assert_eq!(
                serde_json::to_value(result.unwrap()).unwrap(),
                *expected,
                "{}",
                case["name"]
            );
        } else {
            let error = result.unwrap_err().to_string();
            assert!(
                error.contains(case["errorContains"].as_str().unwrap()),
                "{}: {error}",
                case["name"]
            );
        }
    }
}

#[test]
fn checked_disabled_focus_and_hover_reach_separate_composition_layers() {
    let activation = ActivationState::try_new(false, true, None).unwrap();
    let states = resolve_toggle_states(&activation, true, true);
    assert_eq!(
        states.states(),
        &[S::Hover, S::Focused, S::Checked, S::Disabled]
    );
    let layers = states.compose();
    assert_eq!(layers.states(StateLayer::Selection), &[S::Checked]);
    assert_eq!(layers.states(StateLayer::Availability), &[S::Disabled]);
    assert_eq!(layers.states(StateLayer::Navigation), &[S::Focused]);
    assert_eq!(layers.states(StateLayer::Interaction), &[S::Hover]);
    assert!(layers.states(StateLayer::Base).is_empty());
    assert!(
        resolve_command_phase(&states)
            .unwrap_err()
            .contains("command paint supports only")
    );
}

#[test]
fn captured_pointer_exit_and_disable_preserve_checked_but_change_feedback() {
    let initial = ActivationState::try_new(true, false, None).unwrap();
    let down = resolve_toggle_activation(
        &initial,
        true,
        &E::PointerDown {
            id: "primary".into(),
            inside: true,
        },
    )
    .unwrap();
    assert_eq!(
        resolve_toggle_states(down.activation().state(), false, down.checked()).states(),
        &[S::Pressed, S::Checked]
    );
    let outside = resolve_toggle_activation(
        down.activation().state(),
        down.checked(),
        &E::PointerMove {
            id: "primary".into(),
            inside: false,
        },
    )
    .unwrap();
    assert!(outside.activation().state().hold().is_some());
    let idle = resolve_toggle_states(outside.activation().state(), false, outside.checked());
    assert_eq!(idle.states(), &[S::Rest, S::Checked]);
    assert_eq!(idle.compose().states(StateLayer::Base), &[S::Rest]);
    assert_eq!(idle.compose().states(StateLayer::Selection), &[S::Checked]);
    let focused = resolve_toggle_activation(
        outside.activation().state(),
        outside.checked(),
        &E::Focus { focused: true },
    )
    .unwrap();
    let disabled = resolve_toggle_activation(
        focused.activation().state(),
        focused.checked(),
        &E::Availability { enabled: false },
    )
    .unwrap();
    assert!(disabled.activation().capture().is_some());
    assert_eq!(
        resolve_toggle_states(disabled.activation().state(), true, disabled.checked()).states(),
        &[S::Hover, S::Focused, S::Checked, S::Disabled]
    );
}

#[test]
fn release_and_invoke_publish_current_selection_after_transition() {
    let initial = ActivationState::try_new(true, false, None).unwrap();
    let down = resolve_toggle_activation(
        &initial,
        false,
        &E::PointerDown {
            id: "primary".into(),
            inside: true,
        },
    )
    .unwrap();
    assert_eq!(
        resolve_toggle_states(down.activation().state(), false, down.checked()).states(),
        &[S::Pressed]
    );
    let released = resolve_toggle_activation(
        down.activation().state(),
        down.checked(),
        &E::PointerUp {
            id: "primary".into(),
            inside: true,
        },
    )
    .unwrap();
    assert_eq!(
        resolve_toggle_states(released.activation().state(), false, released.checked()).states(),
        &[S::Rest, S::Checked]
    );
    let invoked = resolve_toggle_activation(
        released.activation().state(),
        released.checked(),
        &E::Invoke {},
    )
    .unwrap();
    assert_eq!(
        resolve_toggle_states(invoked.activation().state(), false, invoked.checked()).states(),
        &[S::Rest]
    );
}

#[test]
fn cli_rejects_invalid_utf8_and_distinguishes_usage() {
    let binary = env!("CARGO_BIN_EXE_resina-toggle-states");
    let usage = Command::new(binary).output().unwrap();
    assert_eq!(usage.status.code(), Some(2));
    assert!(usage.stdout.is_empty());
    let mut child = Command::new(binary)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&[0xff]).unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}
