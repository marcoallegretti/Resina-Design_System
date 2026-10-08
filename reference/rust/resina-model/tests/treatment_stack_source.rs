use resina_model::{TreatmentStack, TreatmentStackInput};
use serde_json::{Value, json};

fn schema() -> Value {
    serde_json::from_str(include_str!(
        "../../../../schemas/treatment-stack.schema.json"
    ))
    .unwrap()
}

fn rejects(document: Value) {
    let source = document.to_string();
    assert!(
        serde_json::from_value::<TreatmentStack>(document.clone()).is_err(),
        "{document}"
    );
    assert!(
        serde_json::from_str::<TreatmentStack>(&source).is_err(),
        "{source}"
    );
}

#[test]
fn treatment_stack_inputs_require_named_records() {
    let positional = json!(["0.1.0", ["none"]]);
    rejects(positional.clone());
    assert!(serde_json::from_value::<TreatmentStackInput>(positional.clone()).is_err());
    assert!(serde_json::from_str::<TreatmentStackInput>(&positional.to_string()).is_err());
}

#[test]
fn every_treatment_requires_a_string_name() {
    for treatment in schema()["properties"]["treatments"]["items"]["enum"]
        .as_array()
        .unwrap()
    {
        let document =
            json!({"schemaVersion":"0.1.0", "treatments":[{treatment.as_str().unwrap():null}]});
        rejects(document.clone());
        assert!(serde_json::from_value::<TreatmentStackInput>(document.clone()).is_err());
        assert!(serde_json::from_str::<TreatmentStackInput>(&document.to_string()).is_err());
    }
}

#[test]
fn canonical_and_escaped_names_preserve_ancestor_order() {
    let treatments = schema()["properties"]["treatments"]["items"]["enum"]
        .as_array()
        .unwrap()
        .clone();
    for values in treatments
        .iter()
        .map(|treatment| vec![treatment.clone()])
        .chain(treatments.iter().map(|treatment| {
            vec![
                json!("none"),
                treatment.clone(),
                json!("none"),
                json!("none"),
            ]
        }))
    {
        let canonical = json!({"schemaVersion":"0.1.0", "treatments":values});
        let source = canonical.to_string();
        let mut escaped = source.replace("\"treatments\"", "\"\\u0074reatments\"");
        for treatment in &values {
            let name = treatment.as_str().unwrap();
            escaped = escaped.replace(
                &format!("\"{name}\""),
                &format!("\"\\u{:04x}{}\"", name.as_bytes()[0], &name[1..]),
            );
        }
        for decoded in [
            serde_json::from_value::<TreatmentStack>(canonical.clone()).unwrap(),
            serde_json::from_str::<TreatmentStack>(&source).unwrap(),
            serde_json::from_str::<TreatmentStack>(&escaped).unwrap(),
            TreatmentStack::try_from(
                serde_json::from_str::<TreatmentStackInput>(&escaped).unwrap(),
            )
            .unwrap(),
        ] {
            assert_eq!(serde_json::to_value(decoded).unwrap(), canonical);
        }
    }
}

#[test]
fn invalid_stacks_and_members_keep_diagnostics() {
    for (document, fragment) in [
        (
            json!({"schemaVersion":"0.1.0", "treatments":[]}),
            "must not be empty",
        ),
        (
            json!({"schemaVersion":"0.1.0", "treatments":["lens","none","focusLens"]}),
            "cannot be nested",
        ),
        (
            json!({"schemaVersion":"0.1.0", "treatments":["unknown"]}),
            "unknown variant",
        ),
        (
            json!({"schemaVersion":"0.2.0", "treatments":["none"]}),
            "schemaVersion",
        ),
        (json!({"treatments":["none"]}), "missing field"),
        (json!({"schemaVersion":"0.1.0"}), "missing field"),
        (
            json!({"schemaVersion":"0.1.0", "treatments":["none"], "extra":true}),
            "unknown field",
        ),
    ] {
        for error in [
            serde_json::from_value::<TreatmentStack>(document.clone()).unwrap_err(),
            serde_json::from_str::<TreatmentStack>(&document.to_string()).unwrap_err(),
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
        json!("NONE"),
    ] {
        rejects(json!({"schemaVersion":"0.1.0", "treatments":[value]}));
    }
    let document = json!({"schemaVersion":"0.1.0", "treatments":["none"]});
    let source = document.to_string();
    for (field, value) in document.as_object().unwrap() {
        let member = format!("\"{field}\":{value}");
        for name in [
            field.clone(),
            format!("\\u{:04x}{}", field.as_bytes()[0], &field[1..]),
        ] {
            let duplicate = source.replace(&member, &format!("{member},\"{name}\":{value}"));
            assert!(
                serde_json::from_str::<TreatmentStack>(&duplicate)
                    .unwrap_err()
                    .to_string()
                    .contains("duplicate field")
            );
        }
    }
}
