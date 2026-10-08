use resina_model::{ShapeIntent, SurfaceForm};
use serde_json::{Value, json};

#[test]
fn shape_intents_require_canonical_strings() {
    for (name, expected) in [
        ("structural", ShapeIntent::Structural),
        ("soft", ShapeIntent::Soft),
        ("rounded", ShapeIntent::Rounded),
        ("capsule", ShapeIntent::Capsule),
        ("organic", ShapeIntent::Organic),
    ] {
        let value = json!(name);
        assert_eq!(
            serde_json::from_value::<ShapeIntent>(value.clone()).unwrap(),
            expected
        );
        assert_eq!(
            serde_json::from_str::<ShapeIntent>(&value.to_string()).unwrap(),
            expected
        );
        assert_eq!(serde_json::to_value(expected).unwrap(), value);
        let escaped = format!("\"\\u{:04x}{}\"", name.as_bytes()[0], &name[1..]);
        assert_eq!(
            serde_json::from_str::<ShapeIntent>(&escaped).unwrap(),
            expected
        );
    }
    let vectors: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/surface-form-vectors.json"
    ))
    .unwrap();
    let cases: Vec<_> = vectors
        .iter()
        .filter(|v| {
            v["name"]
                .as_str()
                .unwrap()
                .starts_with("shape intent form ")
        })
        .collect();
    assert_eq!(cases.len(), 14);
    for case in cases {
        let shape = &case["document"]["shape"];
        let fragment = case["error"].as_str().unwrap();
        for error in [
            serde_json::from_value::<ShapeIntent>(shape.clone()).unwrap_err(),
            serde_json::from_str::<ShapeIntent>(&shape.to_string()).unwrap_err(),
            serde_json::from_value::<SurfaceForm>(case["document"].clone()).unwrap_err(),
            serde_json::from_str::<SurfaceForm>(&case["document"].to_string()).unwrap_err(),
        ] {
            assert!(
                error.to_string().contains(fragment),
                "{}: {error}",
                case["name"]
            );
        }
    }
}

#[test]
fn surface_form_shape_members_preserve_duplicate_checks() {
    let source = r#"{"schemaVersion":"0.1.0","shape":"structural","elevation":"raised"}"#;
    let escaped = source.replace("\"shape\"", "\"\\u0073hape\"");
    let form: SurfaceForm = serde_json::from_str(&escaped).unwrap();
    assert_eq!(
        serde_json::to_value(form).unwrap(),
        serde_json::from_str::<Value>(source).unwrap()
    );
    for extra in [r#""shape":"structural""#, r#""\u0073hape":"structural""#] {
        let duplicate = source.replace(
            r#""shape":"structural""#,
            &format!(r#""shape":"structural",{extra}"#),
        );
        assert!(
            serde_json::from_str::<SurfaceForm>(&duplicate)
                .unwrap_err()
                .to_string()
                .contains("duplicate field")
        );
    }
}
