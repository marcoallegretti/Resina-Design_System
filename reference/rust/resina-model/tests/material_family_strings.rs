use resina_model::{MaterialAssignments, MaterialFamily, MaterialRole};
use serde_json::{Value, json};

#[test]
fn families_require_strings_and_keep_their_serialized_names() {
    for (name, family) in [
        ("cast", MaterialFamily::Cast),
        ("frost", MaterialFamily::Frost),
        ("elastomer", MaterialFamily::Elastomer),
        ("gel", MaterialFamily::Gel),
    ] {
        let raw = serde_json::to_string(name).unwrap();
        assert_eq!(
            serde_json::from_str::<MaterialFamily>(&raw).unwrap(),
            family
        );
        assert_eq!(
            serde_json::from_value::<MaterialFamily>(json!(name)).unwrap(),
            family
        );
        assert_eq!(serde_json::to_value(family).unwrap(), json!(name));
        assert!(
            serde_json::from_value::<MaterialFamily>(json!({name: null})).is_err(),
            "{name}"
        );
    }
}

#[test]
fn raw_tagged_families_and_nonstrings_are_rejected() {
    for source in [
        r#"{"cast":null}"#,
        r#"{"frost":null}"#,
        r#"{"elastomer":null}"#,
        r#"{"gel":null}"#,
        r#"{"\u0063ast":null}"#,
        "null",
        "true",
        "1",
        "[]",
        "{}",
        r#""Cast""#,
        r#""unknown""#,
    ] {
        assert!(
            serde_json::from_str::<MaterialFamily>(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn escaped_names_and_assignment_restrictions_remain_validated() {
    assert_eq!(
        serde_json::from_str::<MaterialFamily>(r#""\u0063ast""#).unwrap(),
        MaterialFamily::Cast
    );
    let vectors: Value = serde_json::from_str(include_str!(
        "../../../../conformance/materials/role-assignment-vectors.json"
    ))
    .unwrap();
    let baseline = vectors[0]["document"].clone();
    let assignments: MaterialAssignments = serde_json::from_value(baseline.clone()).unwrap();
    for (pointer, role) in [
        ("/surface/base", MaterialRole::SurfaceBase),
        ("/control/interactive", MaterialRole::ControlInteractive),
        ("/feedback/focus", MaterialRole::FeedbackFocus),
    ] {
        let family: MaterialFamily =
            serde_json::from_value(baseline.pointer(pointer).unwrap().clone()).unwrap();
        assert_eq!(assignments.material_for(role), family);
        let mut tagged = baseline.clone();
        *tagged.pointer_mut(pointer).unwrap() =
            json!({baseline.pointer(pointer).unwrap().as_str().unwrap(): null});
        assert!(serde_json::from_value::<MaterialAssignments>(tagged).is_err());
    }
    for pointer in ["/surface/base", "/control/interactive"] {
        let mut document = baseline.clone();
        *document.pointer_mut(pointer).unwrap() = json!("gel");
        assert!(serde_json::from_value::<MaterialAssignments>(document).is_err());
    }
    for pointer in ["/surface/transient", "/feedback/focus"] {
        let mut document = baseline.clone();
        *document.pointer_mut(pointer).unwrap() = json!("gel");
        assert!(serde_json::from_value::<MaterialAssignments>(document).is_ok());
    }
}

#[test]
fn every_family_accepts_escaped_names_and_rejects_alternate_spellings() {
    for name in ["cast", "frost", "elastomer", "gel"] {
        let expected: MaterialFamily = serde_json::from_value(json!(name)).unwrap();
        let escaped = format!("\"\\u{:04x}{}\"", name.as_bytes()[0], &name[1..]);
        assert_eq!(
            serde_json::from_str::<MaterialFamily>(&escaped).unwrap(),
            expected
        );
        for alternative in [
            name.to_uppercase(),
            format!(" {name}"),
            format!("{name} "),
            format!("{name}\0"),
        ] {
            assert!(serde_json::from_value::<MaterialFamily>(json!(alternative)).is_err());
        }
        for record in [
            json!({name: null}),
            json!({name: []}),
            json!([name]),
            json!({}),
        ] {
            assert!(serde_json::from_value::<MaterialFamily>(record).is_err());
        }
    }
}
