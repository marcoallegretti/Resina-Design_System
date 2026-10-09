use resina_model::KeyLight;
use serde_json::{Value, json};

fn record(x: f64, y: f64) -> Value {
    json!({"schemaVersion":"0.1.0","direction":{"x":x,"y":y}})
}

#[test]
fn checked_light_requires_named_members_in_raw_and_value_decoding() {
    for (x, y) in [(0.0, -1.0), (1.0, 0.0), (1e-308, -1e-308), (1e308, -1e308)] {
        let named = record(x, y);
        let expected: KeyLight = serde_json::from_value(named.clone()).unwrap();
        assert_eq!(
            serde_json::from_str::<KeyLight>(&named.to_string()).unwrap(),
            expected
        );
        let positional = json!([named["schemaVersion"], named["direction"]]);
        for error in [
            serde_json::from_value::<KeyLight>(positional.clone()).unwrap_err(),
            serde_json::from_str::<KeyLight>(&positional.to_string()).unwrap_err(),
        ] {
            assert!(error.to_string().contains("key light object"));
        }
    }
}

#[test]
fn named_order_and_escaped_members_preserve_light_but_duplicates_fail() {
    let expected: KeyLight = serde_json::from_value(record(0.0, -1.0)).unwrap();
    for source in [
        r#"{"direction":{"x":0,"y":-1},"schemaVersion":"0.1.0"}"#,
        r#"{"schema\u0056ersion":"0.1.0","\u0064irection":{"x":0,"y":-1}}"#,
    ] {
        assert_eq!(serde_json::from_str::<KeyLight>(source).unwrap(), expected);
    }
    for source in [
        r#"{"schemaVersion":"0.1.0","schema\u0056ersion":"0.1.0","direction":{"x":0,"y":-1}}"#,
        r#"{"schemaVersion":"0.1.0","direction":{"x":0,"y":-1},"\u0064irection":{"x":0,"y":-1}}"#,
    ] {
        assert!(
            serde_json::from_str::<KeyLight>(source)
                .unwrap_err()
                .to_string()
                .contains("duplicate field")
        );
    }
}

#[test]
fn strict_members_version_and_direction_validation_remain_checked() {
    let named = record(0.0, -1.0);
    let mut invalid = vec![
        json!(null),
        json!([]),
        json!(true),
        json!(1),
        json!("light"),
    ];
    for member in ["schemaVersion", "direction"] {
        let mut missing = named.clone();
        missing.as_object_mut().unwrap().remove(member);
        invalid.push(missing);
        let mut null = named.clone();
        null[member] = json!(null);
        invalid.push(null);
    }
    for version in [json!("9.9.9"), json!(1), json!(true)] {
        let mut changed = named.clone();
        changed["schemaVersion"] = version;
        invalid.push(changed);
    }
    let mut extra = named.clone();
    extra["renderer"] = json!("native");
    invalid.push(extra);
    invalid.push(record(0.0, -0.0));
    let mut positional_direction = named;
    positional_direction["direction"] = json!([0, -1]);
    invalid.push(positional_direction);
    for value in invalid {
        assert!(serde_json::from_str::<KeyLight>(&value.to_string()).is_err());
        assert!(serde_json::from_value::<KeyLight>(value).is_err());
    }
    assert!(
        serde_json::from_str::<KeyLight>(
            r#"{"schemaVersion":"0.1.0","direction":{"x":1e400,"y":1}}"#
        )
        .is_err()
    );
}
