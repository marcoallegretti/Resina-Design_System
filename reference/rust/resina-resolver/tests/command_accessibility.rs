use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{ActivationEvent, ActivationState, SurfaceSize, TypographyRole};
use resina_resolver::{
    CommandAccessibilityError, CommandAccessibilityInput, CommandLabelInput, CommandLabelIr,
    compile_theme_source, resolve_activation, resolve_command_accessibility, resolve_command_label,
};
use serde_json::{Value, json};
use std::convert::Infallible;

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
fn public_semantics_cases_match_without_platform_objects() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/accessibility/command-cases.json"
    ))
    .unwrap();
    for case in cases {
        let input = &case["input"];
        let label = label(input["labelText"].as_str().unwrap());
        let activation: ActivationState =
            serde_json::from_value(input["activation"].clone()).unwrap();
        let result = resolve_command_accessibility(CommandAccessibilityInput {
            label: &label,
            activation: &activation,
            description: input["description"].as_str(),
            focusable: input["focusable"].as_bool().unwrap(),
        });
        if let Some(error) = case.get("error") {
            assert_eq!(
                format!("{:?}", result.unwrap_err()),
                error.as_str().unwrap(),
                "{}",
                case["name"]
            );
        } else {
            let ir = result.unwrap();
            assert_eq!(
                serde_json::to_value(&ir).unwrap(),
                case["expected"],
                "{}",
                case["name"]
            );
            assert_eq!(ir.role(), "button");
            assert_eq!(ir.name(), label.text());
            assert_eq!(ir.description(), input["description"].as_str());
            assert_eq!(ir.state().enabled(), activation.enabled());
            assert_eq!(ir.state().focused(), activation.focused());
            assert_eq!(ir.focusable(), input["focusable"].as_bool().unwrap());
            assert_eq!(ir.actions()[0].kind(), "invoke");
            assert_eq!(ir.actions()[0].available(), activation.enabled());
        }
    }
}

#[test]
fn semantic_invoke_uses_live_availability_and_supersedes_held_press() {
    let label = label("Reconnect");
    let state = ActivationState::try_new(true, true, None).unwrap();
    let armed = resolve_activation(
        &state,
        &ActivationEvent::KeyDown {
            key: resina_model::ActivationKey::Space,
            repeat: false,
        },
    )
    .unwrap();
    let before = resolve_command_accessibility(CommandAccessibilityInput {
        label: &label,
        activation: &state,
        description: None,
        focusable: true,
    })
    .unwrap();
    let held = resolve_command_accessibility(CommandAccessibilityInput {
        label: &label,
        activation: armed.state(),
        description: None,
        focusable: true,
    })
    .unwrap();
    assert_eq!(before, held);
    assert!(held.actions()[0].available());
    let invoked = resolve_activation(armed.state(), &ActivationEvent::Invoke {}).unwrap();
    assert!(invoked.activate());
    let released = resolve_activation(
        invoked.state(),
        &ActivationEvent::KeyUp {
            key: resina_model::ActivationKey::Space,
        },
    )
    .unwrap();
    assert!(!released.activate());
    let disabled = resolve_activation(
        armed.state(),
        &ActivationEvent::Availability { enabled: false },
    )
    .unwrap();
    let unavailable = resolve_command_accessibility(CommandAccessibilityInput {
        label: &label,
        activation: disabled.state(),
        description: None,
        focusable: true,
    })
    .unwrap();
    assert!(!unavailable.actions()[0].available());
    assert!(
        !resolve_activation(disabled.state(), &ActivationEvent::Invoke {})
            .unwrap()
            .activate()
    );
    assert!(held.actions()[0].available());
}

#[test]
fn descriptions_preserve_content_and_blank_values_fail_diagnostically() {
    let label = label("Save");
    let state = ActivationState::try_new(true, false, None).unwrap();
    for text in ["", " ", "\n\t", "\u{a0}"] {
        let error = resolve_command_accessibility(CommandAccessibilityInput {
            label: &label,
            activation: &state,
            description: Some(text),
            focusable: true,
        })
        .unwrap_err();
        assert_eq!(error, CommandAccessibilityError::BlankDescription);
        assert!(!error.to_string().is_empty());
    }
    let ir = resolve_command_accessibility(CommandAccessibilityInput {
        label: &label,
        activation: &state,
        description: Some("  Save the complete document.\n"),
        focusable: true,
    })
    .unwrap();
    assert_eq!(ir.description(), Some("  Save the complete document.\n"));
    let encoded = serde_json::to_value(&ir).unwrap();
    assert_eq!(encoded["value"], Value::Null);
    assert_eq!(encoded["relationships"], json!([]));
    assert!(encoded["state"].get("pressed").is_none());
}
