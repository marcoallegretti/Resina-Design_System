use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{ActivationState, SurfaceSize, TypographyRole};
use resina_resolver::{
    CommandLabelInput, CommandLabelIr, ToggleAccessibilityInput, compile_theme_source,
    resolve_command_label, resolve_toggle_accessibility,
};
use serde_json::Value;
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
        "../../../../conformance/accessibility/toggle-cases.json"
    ))
    .unwrap();
    for case in cases {
        let input = &case["input"];
        let label = label(input["labelText"].as_str().unwrap());
        let activation: ActivationState =
            serde_json::from_value(input["activation"].clone()).unwrap();
        let result = resolve_toggle_accessibility(ToggleAccessibilityInput {
            label: &label,
            activation: &activation,
            description: input["description"].as_str(),
            focusable: input["focusable"].as_bool().unwrap(),
            checked: input["checked"].as_bool().unwrap(),
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
            assert_eq!(ir.role(), "switch");
            assert_eq!(ir.state().checked(), input["checked"].as_bool().unwrap());
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
fn semantic_toggle_updates_checked_without_changing_name_or_actual_focus() {
    use resina_model::{ActivationEvent as E, ActivationKey};
    use resina_resolver::{ToggleAccessibilityError, resolve_toggle_activation};
    let label = label("Notifications");
    let initial = ActivationState::try_new(true, true, None).unwrap();
    let snapshot = |activation: &ActivationState, checked| {
        resolve_toggle_accessibility(ToggleAccessibilityInput {
            label: &label,
            activation,
            description: Some("  Receive updates.\n"),
            focusable: true,
            checked,
        })
        .unwrap()
    };
    let off = snapshot(&initial, false);
    let armed = resolve_toggle_activation(
        &initial,
        false,
        &E::KeyDown {
            key: ActivationKey::Space,
            repeat: false,
        },
    )
    .unwrap();
    assert_eq!(off, snapshot(armed.activation().state(), armed.checked()));
    let invoked =
        resolve_toggle_activation(armed.activation().state(), armed.checked(), &E::Invoke {})
            .unwrap();
    assert!(invoked.activation().activate());
    let on = snapshot(invoked.activation().state(), invoked.checked());
    assert!(on.state().checked());
    assert_eq!(on.name(), off.name());
    assert_eq!(on.description(), Some("  Receive updates.\n"));
    assert!(on.state().focused());
    let released = resolve_toggle_activation(
        invoked.activation().state(),
        invoked.checked(),
        &E::KeyUp {
            key: ActivationKey::Space,
        },
    )
    .unwrap();
    assert!(!released.activation().activate());
    assert_eq!(
        on,
        snapshot(released.activation().state(), released.checked())
    );
    let disabled = resolve_toggle_activation(
        released.activation().state(),
        released.checked(),
        &E::Availability { enabled: false },
    )
    .unwrap();
    let unavailable = snapshot(disabled.activation().state(), disabled.checked());
    assert!(unavailable.state().checked());
    assert!(unavailable.state().focused());
    assert!(!unavailable.actions()[0].available());
    let denied = resolve_toggle_activation(
        disabled.activation().state(),
        disabled.checked(),
        &E::Invoke {},
    )
    .unwrap();
    assert!(!denied.activation().activate());
    assert_eq!(
        unavailable,
        snapshot(denied.activation().state(), denied.checked())
    );
    for text in ["", " ", "\n\t", "\u{a0}"] {
        assert_eq!(
            resolve_toggle_accessibility(ToggleAccessibilityInput {
                label: &label,
                activation: &initial,
                checked: true,
                description: Some(text),
                focusable: true,
            })
            .unwrap_err(),
            ToggleAccessibilityError::BlankDescription
        );
    }
    let encoded = serde_json::to_value(on).unwrap();
    assert_eq!(encoded["value"], Value::Null);
    assert_eq!(encoded["relationships"], serde_json::json!([]));
    assert!(encoded["state"].get("pressed").is_none());
}
