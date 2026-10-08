use resina_model::{CornerTokenPaths, ShapeFallbackAssignments, ShapeFallbackProfile};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::fmt::Debug;

fn assert_members<T: DeserializeOwned + Serialize + Debug>(document: &Value, object: &Value) {
    let source = document.to_string();
    for (field, value) in object.as_object().unwrap() {
        let member = format!("\"{field}\":{value}");
        let escaped = format!("\\u{:04x}{}", field.as_bytes()[0], &field[1..]);
        let escaped_member = format!("\"{escaped}\":{value}");
        for duplicate in [&member, &escaped_member] {
            let changed = source.replacen(&member, &format!("{member},{duplicate}"), 1);
            assert!(changed.len() > source.len());
            let error = serde_json::from_str::<T>(&changed).unwrap_err();
            assert!(error.to_string().contains("duplicate"), "{field}: {error}");
        }
        let changed = source.replacen(&member, &escaped_member, 1);
        let decoded: T = serde_json::from_str(&changed).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), *document);
    }
}

#[test]
fn duplicate_and_escaped_members_preserve_validation() {
    let assignments: Value =
        serde_json::from_str(include_str!("../../../../definitions/tier0-shapes.json")).unwrap();
    assert_members::<ShapeFallbackAssignments>(&assignments, &assignments);
    assert_members::<ShapeFallbackAssignments>(&assignments, &assignments["profiles"]);
    for (_, profile) in assignments["profiles"].as_object().unwrap() {
        assert_members::<ShapeFallbackProfile>(profile, profile);
    }
    let corners = &assignments["profiles"]["organic"]["radii"];
    assert_members::<CornerTokenPaths>(corners, corners);
}

#[test]
fn standalone_profiles_and_corners_require_objects() {
    let radii = json!({"topStart":"radius.1","topEnd":"radius.2",
                      "bottomEnd":"radius.3","bottomStart":"radius.4"});
    let profiles = [
        json!({"kind":"uniform","radius":"radius.1"}),
        json!({"kind":"corners","radii":radii}),
        json!({"kind":"capsule"}),
    ];
    for profile in profiles {
        let positional = match profile["kind"].as_str().unwrap() {
            "uniform" => json!(["uniform", profile["radius"]]),
            "corners" => json!(["corners", profile["radii"]]),
            _ => json!(["capsule"]),
        };
        for invalid in [
            positional,
            Value::Null,
            json!(true),
            json!(1),
            json!("profile"),
            json!([]),
        ] {
            for result in [
                serde_json::from_str::<ShapeFallbackProfile>(&invalid.to_string()),
                serde_json::from_value::<ShapeFallbackProfile>(invalid.clone()),
            ] {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("assignment object")
                );
            }
        }
        let decoded: ShapeFallbackProfile = serde_json::from_value(profile.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), profile);
    }
    for invalid in [
        json!(["radius.1", "radius.2", "radius.3", "radius.4"]),
        Value::Null,
        json!(true),
        json!(1),
        json!("corners"),
        json!([]),
    ] {
        for result in [
            serde_json::from_str::<CornerTokenPaths>(&invalid.to_string()),
            serde_json::from_value::<CornerTokenPaths>(invalid.clone()),
        ] {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("assignment object")
            );
        }
    }
    let decoded: CornerTokenPaths = serde_json::from_value(radii.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), radii);
}

#[test]
fn standalone_profiles_preserve_required_and_checked_members() {
    for invalid in [
        json!({"kind":"capsule","radius":"radius.1"}),
        json!({"kind":"uniform","radius":""}),
        json!({"kind":"uniform"}),
        json!({"kind":"corners","radii":{}}),
        json!({"kind":"unknown"}),
        json!({"radius":"radius.1"}),
    ] {
        assert!(serde_json::from_str::<ShapeFallbackProfile>(&invalid.to_string()).is_err());
        assert!(serde_json::from_value::<ShapeFallbackProfile>(invalid).is_err());
    }
    for field in ["topStart", "topEnd", "bottomEnd", "bottomStart"] {
        let mut radii = json!({"topStart":"radius.1","topEnd":"radius.2",
                              "bottomEnd":"radius.3","bottomStart":"radius.4"});
        radii[field] = json!("");
        assert!(
            serde_json::from_value::<CornerTokenPaths>(radii)
                .unwrap_err()
                .to_string()
                .contains("shape token path must not be empty")
        );
    }
}
