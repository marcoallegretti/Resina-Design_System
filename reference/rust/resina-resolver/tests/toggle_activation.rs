use resina_model::{ActivationEvent as E, ActivationKey as K, ActivationState};
use resina_resolver::{
    CaptureChange, ToggleActivationError, resolve_toggle_activation,
    resolve_toggle_activation_source,
};
use serde_json::Value;
use std::{
    error::Error,
    io::Write,
    process::{Command, Output, Stdio},
};

#[test]
fn public_cases_match_complete_results_and_diagnostics() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/toggle-activation-cases.json"
    ))
    .unwrap();
    for case in cases {
        let result = resolve_toggle_activation_source(&case["request"].to_string());
        if let Some(expected) = case.get("expected") {
            assert_eq!(
                serde_json::to_value(result.unwrap()).unwrap(),
                *expected,
                "{}",
                case["name"]
            );
        } else {
            let error = result.unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains(case["errorContains"].as_str().unwrap()),
                "{}: {error}",
                case["name"]
            );
        }
    }
}

#[test]
fn release_toggles_the_current_checked_value_after_external_change() {
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
    assert!(!down.checked());
    assert!(!down.activation().activate());
    assert!(down.activation().state().pressed());
    let up = resolve_toggle_activation(
        down.activation().state(),
        true,
        &E::PointerUp {
            id: "primary".into(),
            inside: true,
        },
    )
    .unwrap();
    assert!(!up.checked());
    assert!(up.activation().activate());
    assert!(up.activation().state().hold().is_none());
    assert_eq!(
        up.activation().capture(),
        Some(&CaptureChange::Release {
            id: "primary".into()
        })
    );
    let orphan = resolve_toggle_activation(
        up.activation().state(),
        up.checked(),
        &E::PointerUp {
            id: "primary".into(),
            inside: true,
        },
    )
    .unwrap();
    assert!(!orphan.checked());
    assert!(!orphan.activation().activate());
}

#[test]
fn cancellation_and_disable_do_not_reset_selection_or_toggle_on_release() {
    let initial = ActivationState::try_new(true, true, None).unwrap();
    for stop in [
        E::Cancel {},
        E::Focus { focused: false },
        E::Availability { enabled: false },
    ] {
        let armed = resolve_toggle_activation(
            &initial,
            true,
            &E::KeyDown {
                key: K::Space,
                repeat: false,
            },
        )
        .unwrap();
        assert!(armed.checked());
        let stopped =
            resolve_toggle_activation(armed.activation().state(), armed.checked(), &stop).unwrap();
        assert!(stopped.checked());
        assert!(!stopped.activation().activate());
        let available = resolve_toggle_activation(
            stopped.activation().state(),
            stopped.checked(),
            &E::Availability { enabled: true },
        )
        .unwrap();
        let released = resolve_toggle_activation(
            available.activation().state(),
            available.checked(),
            &E::KeyUp { key: K::Space },
        )
        .unwrap();
        assert!(released.checked());
        assert!(!released.activation().activate());
    }
}

#[test]
fn invoke_supersedes_enter_and_repeats_cannot_toggle_the_value_back() {
    let initial = ActivationState::try_new(true, true, None).unwrap();
    let down = resolve_toggle_activation(
        &initial,
        false,
        &E::KeyDown {
            key: K::Enter,
            repeat: false,
        },
    )
    .unwrap();
    assert!(down.checked());
    let invoked =
        resolve_toggle_activation(down.activation().state(), down.checked(), &E::Invoke {})
            .unwrap();
    assert!(!invoked.checked());
    assert!(invoked.activation().activate());
    for event in [
        E::KeyDown {
            key: K::Enter,
            repeat: true,
        },
        E::KeyUp { key: K::Enter },
    ] {
        let result =
            resolve_toggle_activation(invoked.activation().state(), invoked.checked(), &event)
                .unwrap();
        assert!(!result.checked());
        assert!(!result.activation().activate());
    }
    let next = resolve_toggle_activation(
        invoked.activation().state(),
        invoked.checked(),
        &E::KeyDown {
            key: K::Enter,
            repeat: false,
        },
    )
    .unwrap();
    assert!(next.checked());
    assert!(next.activation().activate());
}

#[test]
fn invalid_event_retains_its_cause_even_when_disabled() {
    let source = r#"{"schemaVersion":"0.1.0","state":{"schemaVersion":"0.1.0","enabled":false,"focused":true,"hold":null},"checked":true,"event":{"kind":"pointerDown","id":"","inside":true}}"#;
    let error = resolve_toggle_activation_source(source).unwrap_err();
    assert!(
        error
            .source()
            .unwrap()
            .to_string()
            .contains("pointer ID must not be empty")
    );
}

#[test]
fn non_object_requests_fail_before_activation_and_publish_no_result() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/toggle-activation-cases.json"
    ))
    .unwrap();
    let invalid: Vec<_> = cases
        .iter()
        .filter(|case| {
            case["name"]
                .as_str()
                .unwrap()
                .starts_with("invalid request root ")
        })
        .collect();
    assert_eq!(invalid.len(), 8);
    for case in invalid {
        let source = case["request"].to_string();
        let error = resolve_toggle_activation_source(&source).unwrap_err();
        assert!(matches!(error, ToggleActivationError::InvalidRequestShape));
        assert!(error.source().is_none());
        assert_eq!(
            error.to_string(),
            "toggle activation request must be a JSON object"
        );
        let output = run_cli(source.as_bytes());
        assert_eq!(output.status.code(), Some(1), "{}", case["name"]);
        assert!(output.stdout.is_empty(), "{}", case["name"]);
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains(&error.to_string()),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn object_requests_preserve_escaped_members_and_duplicate_detection() {
    let source = r#"{"event":{"kind":"invoke"},"che\u0063ked":true,"state":{"hold":null,"focused":true,"enabled":true,"schemaVersion":"0.1.0"},"schemaVersion":"0.1.0"}"#;
    let result = resolve_toggle_activation_source(source).unwrap();
    assert!(result.activation().activate());
    assert!(!result.checked());
    let output = run_cli(source.as_bytes());
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        serde_json::to_value(result).unwrap()
    );
    let duplicate = source.replace(
        "\"che\\u0063ked\":true",
        "\"checked\":false,\"che\\u0063ked\":true",
    );
    let error = resolve_toggle_activation_source(&duplicate).unwrap_err();
    assert!(matches!(error, ToggleActivationError::Parse(_)));
    assert!(
        error
            .source()
            .unwrap()
            .to_string()
            .contains("duplicate JSON member at #/checked")
    );
    let output = run_cli(duplicate.as_bytes());
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

fn run_cli(source: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resina-toggle-activation"))
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(source).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn cli_rejects_duplicate_checked_invalid_utf8_and_bad_usage() {
    let binary = env!("CARGO_BIN_EXE_resina-toggle-activation");
    for source in [b"{\"checked\":true,\"checked\":false}".as_slice(), &[0xff]] {
        let output = run_cli(source);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    let usage = Command::new(binary).output().unwrap();
    assert_eq!(usage.status.code(), Some(2));
    assert!(usage.stdout.is_empty());
    assert!(!usage.stderr.is_empty());
}
