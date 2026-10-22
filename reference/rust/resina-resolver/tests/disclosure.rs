use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{ActivationEvent, ActivationState, SurfaceSize, TypographyRole};
use resina_resolver::{
    CommandAccessibilityInput, CommandLabelInput, CommandLabelIr, DisclosureError, DisclosureInput,
    compile_theme_source, resolve_command_label, resolve_disclosure, resolve_disclosure_source,
};
use serde_json::Value;
use std::{
    convert::Infallible,
    io::Write,
    process::{Command, Stdio},
};

fn label(text: &str) -> CommandLabelIr {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap();
    let theme = compile_theme_source(
        source["surface"]["body"]["theme"]["themeSource"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let environment =
        serde_json::from_value(source["surface"]["body"]["theme"]["environment"].clone()).unwrap();
    let typography =
        theme.resolve(&environment).unwrap().typography()[&TypographyRole::Label].clone();
    resolve_command_label(
        CommandLabelInput {
            text,
            typography: &typography,
            minimum_size: SurfaceSize {
                width: 64.0,
                height: 32.0,
            },
            maximum_size: SurfaceSize {
                width: 200.0,
                height: 160.0,
            },
            padding: SafeArea {
                start: 12.0,
                end: 8.0,
                top: 6.0,
                bottom: 10.0,
            },
            direction: LayoutDirection::Ltr,
        },
        |_| {
            Ok::<_, Infallible>(SurfaceSize {
                width: 60.0,
                height: 54.0,
            })
        },
    )
    .unwrap()
}

#[test]
fn public_cases_match_exact_results_and_fail_for_the_expected_reason() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/disclosure-cases.json"
    ))
    .unwrap();
    for case in cases {
        let result = resolve_disclosure_source(&case["request"].to_string());
        if let Some(expected) = case.get("expected") {
            assert_eq!(
                serde_json::to_value(result.unwrap()).unwrap(),
                *expected,
                "{}",
                case["name"]
            );
        } else {
            let message = result.unwrap_err().to_string();
            assert!(
                message.contains(case["errorContains"].as_str().unwrap()),
                "{}: {message}",
                case["name"]
            );
        }
    }
}

#[test]
fn expansion_changes_exactly_when_activation_fires() {
    let label = label("Details");
    let states = [
        ActivationState::try_new(true, true, None).unwrap(),
        ActivationState::try_new(false, false, None).unwrap(),
    ];
    let events = [
        ActivationEvent::Invoke {},
        ActivationEvent::Cancel {},
        ActivationEvent::Availability { enabled: false },
        ActivationEvent::Focus { focused: false },
    ];
    for state in &states {
        for event in &events {
            for expanded in [false, true] {
                let result = resolve_disclosure(DisclosureInput {
                    command: CommandAccessibilityInput {
                        label: &label,
                        description: None,
                        activation: state,
                        focusable: state.enabled() || state.focused(),
                    },
                    expanded,
                    controlled_content_identity: "details-panel",
                    controlled_content_present: true,
                    event,
                })
                .unwrap();
                assert_eq!(result.expanded(), expanded ^ result.activation().activate());
                assert_eq!(result.accessibility().state().expanded(), result.expanded());
                assert_eq!(
                    result.accessibility().state().focused(),
                    result.activation().state().focused()
                );
            }
        }
    }
}

#[test]
fn validation_precedes_activation_and_publishes_no_partial_result() {
    let label = label("Details");
    let state = ActivationState::try_new(true, true, None).unwrap();
    let event = ActivationEvent::Invoke {};
    let base = DisclosureInput {
        command: CommandAccessibilityInput {
            label: &label,
            description: None,
            activation: &state,
            focusable: true,
        },
        expanded: false,
        controlled_content_identity: "details-panel",
        controlled_content_present: true,
        event: &event,
    };
    assert!(resolve_disclosure(base).unwrap().activation().activate());
    assert!(matches!(
        resolve_disclosure(DisclosureInput {
            controlled_content_present: false,
            ..base
        }),
        Err(DisclosureError::StaleControlledContent)
    ));
    assert!(matches!(
        resolve_disclosure(DisclosureInput {
            controlled_content_identity: "",
            ..base
        }),
        Err(DisclosureError::EmptyControlledContentIdentity)
    ));
}

#[test]
fn cli_rejects_duplicate_members_invalid_utf8_and_bad_usage() {
    for source in [
        b"{\"schemaVersion\":\"0.1.0\",\"schemaVersion\":\"0.1.0\"}".as_slice(),
        &[0xff],
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_resina-disclosure"))
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
    let output = Command::new(env!("CARGO_BIN_EXE_resina-disclosure"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr).unwrap().contains("Usage:"));
}
