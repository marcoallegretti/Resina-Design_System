use resina_model::SurfaceForm;
use serde_json::{Value, json};

fn baseline() -> Value {
    json!({"schemaVersion":"0.1.0", "shape":"structural", "elevation":"raised"})
}

fn rejects(document: Value) {
    assert!(
        serde_json::from_value::<SurfaceForm>(document.clone()).is_err(),
        "{document}"
    );
    assert!(
        serde_json::from_str::<SurfaceForm>(&document.to_string()).is_err(),
        "{document}"
    );
}

#[test]
fn surface_forms_require_named_records() {
    rejects(json!(["0.1.0", "structural", "raised"]));
}

#[test]
fn elevation_members_require_strings() {
    for name in ["embedded", "base", "raised", "floating", "overlay", "modal"] {
        let mut document = baseline();
        document["elevation"] = json!({name: null});
        rejects(document);
    }
}

#[test]
fn all_shape_elevation_pairs_preserve_canonical_and_escaped_forms() {
    let schema: Value =
        serde_json::from_str(include_str!("../../../../schemas/surface-form.schema.json")).unwrap();
    for shape in schema["properties"]["shape"]["enum"].as_array().unwrap() {
        for elevation in schema["properties"]["elevation"]["enum"]
            .as_array()
            .unwrap()
        {
            let document = json!({"schemaVersion":"0.1.0", "shape":shape, "elevation":elevation});
            let source = document.to_string();
            let mut escaped = source.clone();
            for field in ["shape", "elevation"] {
                let name = document[field].as_str().unwrap();
                let member = format!("\"{field}\":\"{name}\"");
                assert_eq!(escaped.matches(&member).count(), 1);
                escaped = escaped.replace(
                    &member,
                    &format!(
                        "\"\\u{:04x}{}\":\"\\u{:04x}{}\"",
                        field.as_bytes()[0],
                        &field[1..],
                        name.as_bytes()[0],
                        &name[1..]
                    ),
                );
            }
            for intent in [
                serde_json::from_value::<SurfaceForm>(document.clone()).unwrap(),
                serde_json::from_str::<SurfaceForm>(&source).unwrap(),
                serde_json::from_str::<SurfaceForm>(&escaped).unwrap(),
            ] {
                assert_eq!(serde_json::to_value(intent).unwrap(), document);
            }
        }
    }
}

#[test]
fn surface_form_member_checks_remain_strict() {
    let document = baseline();
    let source = document.to_string();
    for (field, value) in document.as_object().unwrap() {
        let mut missing = document.clone();
        missing.as_object_mut().unwrap().remove(field);
        rejects(missing);
        let member = format!("\"{field}\":{value}");
        assert_eq!(source.matches(&member).count(), 1);
        for name in [
            field.clone(),
            format!("\\u{:04x}{}", field.as_bytes()[0], &field[1..]),
        ] {
            let duplicate = source.replace(&member, &format!("{member},\"{name}\":{value}"));
            assert!(
                serde_json::from_str::<SurfaceForm>(&duplicate)
                    .unwrap_err()
                    .to_string()
                    .contains("duplicate field")
            );
        }
    }
    let mut unknown = document.clone();
    unknown["extra"] = json!(true);
    rejects(unknown);
    for value in [
        json!(null),
        json!(true),
        json!(1),
        json!([]),
        json!(""),
        json!("unknown"),
    ] {
        let mut invalid = document.clone();
        invalid["elevation"] = value;
        rejects(invalid);
    }
    for version in [json!("0.2.0"), json!(null), json!(1)] {
        let mut invalid = document.clone();
        invalid["schemaVersion"] = version;
        rejects(invalid);
    }
}
