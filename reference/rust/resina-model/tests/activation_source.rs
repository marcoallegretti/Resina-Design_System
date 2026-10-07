use resina_model::{ActivationEvent, ActivationKey, ActivationState, PressHold};
use serde_json::{Value, json};

#[test]
fn keys_require_exact_strings_in_every_shared_owner() {
    for (name, key) in [
        ("space", ActivationKey::Space),
        ("enter", ActivationKey::Enter),
    ] {
        assert_eq!(
            serde_json::from_value::<ActivationKey>(json!(name)).unwrap(),
            key
        );
        assert_eq!(serde_json::to_value(key).unwrap(), json!(name));
        for value in [json!({name: null}), json!([name]), json!(true), Value::Null] {
            assert!(serde_json::from_value::<ActivationKey>(value.clone()).is_err());
            assert!(
                serde_json::from_value::<PressHold>(json!({"kind":"key","key":value})).is_err()
            );
            for kind in ["keyDown", "keyUp"] {
                let mut event = json!({"kind":kind,"key":value});
                if kind == "keyDown" {
                    event["repeat"] = false.into();
                }
                assert!(serde_json::from_value::<ActivationEvent>(event).is_err());
            }
        }
    }
    for name in ["Space", "Enter", " space", "enter ", "", "tab"] {
        assert!(serde_json::from_value::<ActivationKey>(json!(name)).is_err());
    }
}

#[test]
fn checked_states_require_object_records_and_explicit_holds() {
    let valid = json!({"schemaVersion":"0.1.0","enabled":true,"focused":true,"hold":null});
    let expected = ActivationState::try_new(true, true, None).unwrap();
    assert_eq!(
        serde_json::from_value::<ActivationState>(valid.clone()).unwrap(),
        expected
    );
    let mut missing = valid.clone();
    missing.as_object_mut().unwrap().remove("hold");
    let mut unknown = valid.clone();
    unknown["unexpected"] = true.into();
    for value in [
        json!(["0.1.0", true, true, null]),
        json!(true),
        Value::Null,
        missing,
        unknown,
    ] {
        assert!(serde_json::from_str::<ActivationState>(&value.to_string()).is_err());
        assert!(serde_json::from_value::<ActivationState>(value).is_err());
    }
    for hold in [
        json!(["key", "space"]),
        json!({"kind":"key","key":{"space":null}}),
    ] {
        let mut state = valid.clone();
        state["hold"] = hold;
        assert!(serde_json::from_value::<ActivationState>(state).is_err());
    }
}

#[test]
fn escaped_state_members_and_duplicate_aliases_preserve_checked_decoding() {
    let source = r#"{"schemaVersion":"0.1.0","enabled":true,"focused":true,"hold":null}"#;
    let expected: ActivationState = serde_json::from_str(source).unwrap();
    for (field, alias) in [("enabled", "\\u0065nabled"), ("hold", "\\u0068old")] {
        let escaped = source.replace(&format!("\"{field}\""), &format!("\"{alias}\""));
        assert_eq!(
            serde_json::from_str::<ActivationState>(&escaped).unwrap(),
            expected
        );
    }
    for (field, value, alias) in [
        ("enabled", "true", "\\u0065nabled"),
        ("hold", "null", "\\u0068old"),
    ] {
        let member = format!("\"{field}\":{value}");
        let duplicate = source.replacen(&member, &format!("{member},\"{alias}\":{value}"), 1);
        let error = serde_json::from_str::<ActivationState>(&duplicate).unwrap_err();
        assert!(error.to_string().contains("duplicate field"), "{error}");
    }
    assert_eq!(
        serde_json::from_str::<ActivationKey>(r#""\u0065nter""#).unwrap(),
        ActivationKey::Enter
    );
}

#[test]
fn shared_record_variants_round_trip_and_reject_positional_representations() {
    let events = [
        ActivationEvent::PointerDown {
            id: "p".into(),
            inside: true,
        },
        ActivationEvent::PointerMove {
            id: "p".into(),
            inside: false,
        },
        ActivationEvent::PointerUp {
            id: "p".into(),
            inside: true,
        },
        ActivationEvent::PointerCancel { id: "p".into() },
        ActivationEvent::KeyDown {
            key: ActivationKey::Enter,
            repeat: false,
        },
        ActivationEvent::KeyUp {
            key: ActivationKey::Space,
        },
        ActivationEvent::Focus { focused: false },
        ActivationEvent::Availability { enabled: false },
        ActivationEvent::Invoke {},
        ActivationEvent::Cancel {},
    ];
    for event in events {
        let value = serde_json::to_value(&event).unwrap();
        assert_eq!(
            serde_json::from_value::<ActivationEvent>(value.clone()).unwrap(),
            event
        );
        assert!(serde_json::from_value::<ActivationEvent>(positional_record(&value)).is_err());
    }
    for hold in [
        PressHold::Pointer {
            id: "p".into(),
            inside: true,
        },
        PressHold::Key {
            key: ActivationKey::Space,
        },
        PressHold::Key {
            key: ActivationKey::Enter,
        },
    ] {
        let value = serde_json::to_value(&hold).unwrap();
        assert_eq!(
            serde_json::from_value::<PressHold>(value.clone()).unwrap(),
            hold
        );
        assert!(serde_json::from_value::<PressHold>(positional_record(&value)).is_err());
    }
}

fn positional_record(value: &Value) -> Value {
    let mut fields = vec![value["kind"].clone()];
    fields.extend(
        value
            .as_object()
            .unwrap()
            .iter()
            .filter_map(|(name, value)| (name != "kind").then_some(value.clone())),
    );
    Value::Array(fields)
}
