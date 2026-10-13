use resina_resolver::resolve_material_light_source;
use serde_json::Value;

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../../conformance/lighting/material-light-vectors.json"
    ))
    .unwrap()
}

fn compare(actual: &Value, expected: &Value) {
    match expected {
        Value::Object(object) => {
            let actual = actual.as_object().unwrap();
            assert_eq!(actual.len(), object.len());
            for (key, value) in object {
                compare(&actual[key], value);
            }
        }
        Value::Array(values) => {
            let actual = actual.as_array().unwrap();
            assert_eq!(actual.len(), values.len());
            for (a, e) in actual.iter().zip(values) {
                compare(a, e);
            }
        }
        Value::Number(number) => {
            assert!((actual.as_f64().unwrap() - number.as_f64().unwrap()).abs() <= 1e-12)
        }
        _ => assert_eq!(actual, expected),
    }
}

#[test]
fn authored_light_vectors() {
    for case in cases() {
        let result = resolve_material_light_source(&case["request"].to_string());
        if let Some(expected) = case.get("expected") {
            let actual =
                serde_json::to_value(result.unwrap_or_else(|e| panic!("{}: {e}", case["name"])))
                    .unwrap();
            compare(&actual, expected);
            for sample in actual["samples"].as_array().unwrap() {
                if sample["weight"].as_f64().unwrap() == 0.0 {
                    assert_eq!(sample["color"], actual["pigment"]["body"]);
                }
            }
        } else {
            let error = result.unwrap_err().to_string();
            assert!(
                error.contains(case["errorContains"].as_str().unwrap()),
                "{}: {error}",
                case["name"]
            );
        }
    }
}

#[test]
fn negative_source_channels_fail_even_when_suppressed() {
    let mut request = cases()[0]["request"].clone();
    request["light"]["ambient"] = serde_json::json!(0.5);
    request["light"]["interaction"] =
        serde_json::json!({"direction":{"x":-1,"y":0},"strength":0.25});
    request["enabled"] = serde_json::json!(false);
    request["environment"]["accessibilityPreferences"]["highContrast"] = serde_json::json!(true);
    for (member, literal) in [("ambient", "0.5"), ("strength", "0.25")] {
        let fragment = format!("\"{member}\":{literal}");
        let source = request
            .to_string()
            .replacen(&fragment, &format!("\"{member}\":-1e-400"), 1);
        assert!(
            resolve_material_light_source(&source)
                .unwrap_err()
                .to_string()
                .contains("before numeric rounding")
        );
        for zero in ["-0", "-0e-400", "1e-400"] {
            assert!(resolve_material_light_source(&source.replace("-1e-400", zero)).is_ok());
        }
    }
}
