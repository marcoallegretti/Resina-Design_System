use resina_model::{StateSet, StateSetInput};
use serde_json::{Value, json};

fn schema() -> Value {
    serde_json::from_str(include_str!("../../../../schemas/state-set.schema.json")).unwrap()
}

fn rejects(document: Value) {
    let source = document.to_string();
    assert!(
        serde_json::from_value::<StateSet>(document.clone()).is_err(),
        "{document}"
    );
    assert!(
        serde_json::from_str::<StateSet>(&source).is_err(),
        "{source}"
    );
}

#[test]
fn state_set_inputs_require_named_records() {
    let positional = json!(["0.1.0", ["rest"]]);
    rejects(positional.clone());
    assert!(serde_json::from_value::<StateSetInput>(positional.clone()).is_err());
    assert!(serde_json::from_str::<StateSetInput>(&positional.to_string()).is_err());
}

#[test]
fn every_state_requires_a_string_name() {
    for state in schema()["properties"]["states"]["items"]["enum"]
        .as_array()
        .unwrap()
    {
        let document = json!({"schemaVersion":"0.1.0", "states":[{state.as_str().unwrap():null}]});
        rejects(document.clone());
        assert!(serde_json::from_value::<StateSetInput>(document.clone()).is_err());
        assert!(serde_json::from_str::<StateSetInput>(&document.to_string()).is_err());
    }
}

#[test]
fn canonical_and_escaped_names_preserve_all_states_and_normalization() {
    let states = schema()["properties"]["states"]["items"]["enum"]
        .as_array()
        .unwrap()
        .clone();
    for values in states
        .iter()
        .map(|state| vec![state.clone()])
        .chain(std::iter::once(states.clone()))
    {
        let canonical = json!({"schemaVersion":"0.1.0", "states":values});
        let mut reversed = values.clone();
        reversed.reverse();
        let source = json!({"schemaVersion":"0.1.0", "states":reversed}).to_string();
        let mut escaped = source.replace("\"states\"", "\"\\u0073tates\"");
        for state in &values {
            let name = state.as_str().unwrap();
            escaped = escaped.replace(
                &format!("\"{name}\""),
                &format!("\"\\u{:04x}{}\"", name.as_bytes()[0], &name[1..]),
            );
        }
        for decoded in [
            serde_json::from_value::<StateSet>(canonical.clone()).unwrap(),
            serde_json::from_str::<StateSet>(&source).unwrap(),
            serde_json::from_str::<StateSet>(&escaped).unwrap(),
            StateSet::try_from(serde_json::from_str::<StateSetInput>(&escaped).unwrap()).unwrap(),
        ] {
            assert_eq!(serde_json::to_value(decoded).unwrap(), canonical);
        }
    }
}

#[test]
fn invalid_sets_and_members_keep_diagnostics() {
    for (document, fragment) in [
        (
            json!({"schemaVersion":"0.1.0", "states":[]}),
            "must not be empty",
        ),
        (
            json!({"schemaVersion":"0.1.0", "states":["rest","rest"]}),
            "duplicates",
        ),
        (
            json!({"schemaVersion":"0.1.0", "states":["unknown"]}),
            "unknown variant",
        ),
        (
            json!({"schemaVersion":"0.2.0", "states":["rest"]}),
            "schemaVersion",
        ),
        (json!({"states":["rest"]}), "missing field"),
        (json!({"schemaVersion":"0.1.0"}), "missing field"),
        (
            json!({"schemaVersion":"0.1.0", "states":["rest"], "extra":true}),
            "unknown field",
        ),
    ] {
        for error in [
            serde_json::from_value::<StateSet>(document.clone()).unwrap_err(),
            serde_json::from_str::<StateSet>(&document.to_string()).unwrap_err(),
        ] {
            assert!(error.to_string().contains(fragment), "{document}: {error}");
        }
    }
    for value in [
        json!(null),
        json!(true),
        json!(1),
        json!([]),
        json!(""),
        json!("REST"),
    ] {
        rejects(json!({"schemaVersion":"0.1.0", "states":[value]}));
    }
    let document = json!({"schemaVersion":"0.1.0", "states":["rest"]});
    let source = document.to_string();
    for (field, value) in document.as_object().unwrap() {
        let member = format!("\"{field}\":{value}");
        for name in [
            field.clone(),
            format!("\\u{:04x}{}", field.as_bytes()[0], &field[1..]),
        ] {
            let duplicate = source.replace(&member, &format!("{member},\"{name}\":{value}"));
            assert!(
                serde_json::from_str::<StateSet>(&duplicate)
                    .unwrap_err()
                    .to_string()
                    .contains("duplicate field")
            );
        }
    }
}
