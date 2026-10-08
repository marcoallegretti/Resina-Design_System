use resina_model::SurfaceIntent;
use serde_json::{Value, json};

fn baseline() -> Value {
    let vectors: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/surfaces/binding-vectors.json"
    ))
    .unwrap();
    let mut document = vectors[0]["document"].clone();
    document["states"] = vectors[0]["expected"]["states"].clone();
    document
}

fn rejects(document: Value) {
    assert!(
        serde_json::from_value::<SurfaceIntent>(document.clone()).is_err(),
        "accepted value: {document}"
    );
    assert!(
        serde_json::from_str::<SurfaceIntent>(&document.to_string()).is_err(),
        "accepted source: {document}"
    );
}

#[test]
fn every_canonical_surface_role_round_trips() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../schemas/surface-binding.schema.json"
    ))
    .unwrap();
    for field in ["materialRole", "colorRole"] {
        for role in schema["properties"][field]["enum"].as_array().unwrap() {
            let mut document = baseline();
            document[field] = role.clone();
            for intent in [
                serde_json::from_value::<SurfaceIntent>(document.clone()).unwrap(),
                serde_json::from_str::<SurfaceIntent>(&document.to_string()).unwrap(),
            ] {
                assert_eq!(serde_json::to_value(intent).unwrap(), document);
            }
            let name = role.as_str().unwrap();
            let member = format!("\"{field}\":\"{name}\"");
            let source = document.to_string();
            assert_eq!(source.matches(&member).count(), 1);
            let escaped = source.replace(
                &member,
                &format!(
                    "\"\\u{:04x}{}\":\"\\u{:04x}{}\"",
                    field.as_bytes()[0],
                    &field[1..],
                    name.as_bytes()[0],
                    &name[1..]
                ),
            );
            let intent: SurfaceIntent = serde_json::from_str(&escaped).unwrap();
            assert_eq!(serde_json::to_value(intent).unwrap(), document);
        }
    }
}

#[test]
fn positional_surface_records_are_rejected() {
    let document = baseline();
    rejects(json!([
        document["schemaVersion"],
        document["materialRole"],
        document["colorRole"],
        document["form"],
        document["states"],
        document["treatmentStack"],
    ]));
}

fn rejects_role_objects(field: &str) {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../schemas/surface-binding.schema.json"
    ))
    .unwrap();
    for role in schema["properties"][field]["enum"].as_array().unwrap() {
        let mut document = baseline();
        document[field] = json!({role.as_str().unwrap(): null});
        rejects(document);
    }
}

#[test]
fn material_roles_require_strings() {
    rejects_role_objects("materialRole");
}

#[test]
fn color_roles_require_strings() {
    rejects_role_objects("colorRole");
}

#[test]
fn required_members_unknown_members_and_versions_remain_checked() {
    let baseline = baseline();
    for field in baseline.as_object().unwrap().keys() {
        let mut document = baseline.clone();
        document.as_object_mut().unwrap().remove(field);
        rejects(document);
    }
    let mut unknown = baseline.clone();
    unknown["extra"] = json!(true);
    rejects(unknown);
    for version in [json!("0.1.0"), json!("0.3.0"), json!(null), json!(1)] {
        let mut document = baseline.clone();
        document["schemaVersion"] = version;
        rejects(document);
    }
    for field in ["materialRole", "colorRole"] {
        for value in [
            json!(null),
            json!(true),
            json!(1),
            json!([]),
            json!(""),
            json!("unknown"),
        ] {
            let mut document = baseline.clone();
            document[field] = value;
            rejects(document);
        }
    }
}

#[test]
fn duplicate_members_remain_rejected_including_escaped_names() {
    let baseline = baseline();
    let source = baseline.to_string();
    for (field, value) in baseline.as_object().unwrap() {
        let member = format!("\"{field}\":{value}");
        assert_eq!(source.matches(&member).count(), 1);
        for duplicate in [
            field.clone(),
            format!("\\u{:04x}{}", field.as_bytes()[0], &field[1..]),
        ] {
            let duplicate = source.replace(&member, &format!("{member},\"{duplicate}\":{value}"));
            let error = serde_json::from_str::<SurfaceIntent>(&duplicate).unwrap_err();
            assert!(
                error.to_string().contains("duplicate field"),
                "{field}: {error}"
            );
        }
    }
}
