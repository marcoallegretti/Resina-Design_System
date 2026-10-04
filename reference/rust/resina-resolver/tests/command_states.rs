use resina_model::{
    ActivationEvent, ActivationKey, ActivationState, CommandPhase, InteractionState, SurfaceIntent,
};
use resina_resolver::{
    resolve_activation, resolve_command_paint_source, resolve_command_states,
    resolve_command_states_source,
};
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn command_boundary_rejects_invalid_utf8_and_distinguishes_usage() {
    let binary = env!("CARGO_BIN_EXE_resina-command-states");
    let usage = Command::new(binary).output().unwrap();
    assert_eq!(usage.status.code(), Some(2));
    assert!(usage.stdout.is_empty());
    assert!(!usage.stderr.is_empty());
    let mut child = Command::new(binary)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&[0xff]).unwrap();
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert!(!result.stderr.is_empty());
}

#[test]
fn public_projection_cases_preserve_current_signals_and_reject_invalid_inputs() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/command-states-cases.json"
    ))
    .unwrap();
    for case in cases {
        let result = resolve_command_states_source(&case["request"].to_string());
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

fn assert_paint(state: &ActivationState, hovered: bool, phase: CommandPhase) {
    let states = resolve_command_states(state, hovered);
    let mut request: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-paint-request.json"
    ))
    .unwrap();
    let original = request["surface"]["body"]["surface"].clone();
    let surface: SurfaceIntent = serde_json::from_value(original.clone()).unwrap();
    let updated = serde_json::to_value(surface.with_states(states.clone())).unwrap();
    for (key, value) in original.as_object().unwrap() {
        if key != "states" {
            assert_eq!(&updated[key], value);
        }
    }
    request["surface"]["body"]["surface"] = updated;
    let paint = resolve_command_paint_source(&request.to_string()).unwrap();
    assert_eq!(paint.phase(), phase);
    assert_eq!(paint.paint().focus().is_some(), state.focused());
    assert_eq!(
        serde_json::to_value(&paint).unwrap()["paint"]["body"]["states"],
        serde_json::to_value(states).unwrap()
    );
}

#[test]
fn semantic_invoke_release_and_disable_update_paint_from_the_same_activation_state() {
    let state = ActivationState::try_new(true, true, None).unwrap();
    let held = resolve_activation(
        &state,
        &ActivationEvent::KeyDown {
            key: ActivationKey::Space,
            repeat: false,
        },
    )
    .unwrap();
    assert_paint(held.state(), false, CommandPhase::Pressed);
    let invoked = resolve_activation(held.state(), &ActivationEvent::Invoke {}).unwrap();
    assert!(invoked.activate());
    assert_paint(invoked.state(), false, CommandPhase::Rest);
    let released = resolve_activation(
        invoked.state(),
        &ActivationEvent::KeyUp {
            key: ActivationKey::Space,
        },
    )
    .unwrap();
    assert!(!released.activate());
    assert_paint(released.state(), true, CommandPhase::Hover);
    let disabled = resolve_activation(
        released.state(),
        &ActivationEvent::Availability { enabled: false },
    )
    .unwrap();
    assert_paint(disabled.state(), true, CommandPhase::Disabled);
    assert!(resolve_command_states(disabled.state(), true).contains(InteractionState::Hover));
    assert!(resolve_command_states(disabled.state(), true).contains(InteractionState::Focused));
    let unfocused =
        resolve_activation(disabled.state(), &ActivationEvent::Focus { focused: false }).unwrap();
    assert_paint(unfocused.state(), false, CommandPhase::Disabled);
}

#[test]
fn pointer_reentry_and_semantic_invoke_preserve_capture_and_feedback_boundaries() {
    let initial = ActivationState::try_new(true, false, None).unwrap();
    let held = resolve_activation(
        &initial,
        &ActivationEvent::PointerDown {
            id: "primary".into(),
            inside: true,
        },
    )
    .unwrap();
    assert_paint(held.state(), false, CommandPhase::Pressed);
    let outside = resolve_activation(
        held.state(),
        &ActivationEvent::PointerMove {
            id: "primary".into(),
            inside: false,
        },
    )
    .unwrap();
    assert!(outside.state().hold().is_some());
    assert_paint(outside.state(), false, CommandPhase::Rest);
    let reentered = resolve_activation(
        outside.state(),
        &ActivationEvent::PointerMove {
            id: "primary".into(),
            inside: true,
        },
    )
    .unwrap();
    assert_paint(reentered.state(), true, CommandPhase::Pressed);
    let invoked = resolve_activation(reentered.state(), &ActivationEvent::Invoke {}).unwrap();
    assert!(invoked.activate());
    assert!(invoked.capture().is_some());
    assert_paint(invoked.state(), false, CommandPhase::Rest);
    let released = resolve_activation(
        invoked.state(),
        &ActivationEvent::PointerUp {
            id: "primary".into(),
            inside: true,
        },
    )
    .unwrap();
    assert!(!released.activate());
    assert_paint(released.state(), false, CommandPhase::Rest);
}
