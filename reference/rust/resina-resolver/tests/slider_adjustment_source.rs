use resina_resolver::{resolve_slider_adjustment_source, resolve_slider_value_source};
use serde_json::{Value, json};

fn request() -> Value {
    json!({
        "schemaVersion": "0.1.0",
        "current": {"schemaVersion": "0.1.0", "minimum": 0, "maximum": 100, "value": 25},
        "enabled": true,
        "readOnly": false,
        "adjustment": {"kind": "increase", "amount": 10}
    })
}

fn assert_result(actual: &Value, expected: &Value, name: &str) {
    for field in ["schemaVersion", "accepted", "changed"] {
        assert_eq!(actual[field], expected[field], "{name}: {field}");
    }
    assert_eq!(
        actual["value"]["schemaVersion"],
        expected["value"]["schemaVersion"]
    );
    for field in ["minimum", "maximum", "value"] {
        assert_eq!(
            actual["value"][field].as_f64(),
            expected["value"][field].as_f64(),
            "{name}: {field}"
        );
    }
    let progress = actual["value"]["progress"].as_f64().unwrap();
    let target = expected["value"]["progress"].as_f64().unwrap();
    if target == 0.0 || target == 1.0 {
        assert_eq!(progress, target, "{name}");
    } else {
        let ulp = f64::from_bits(target.to_bits() + 1) - target;
        assert!(progress > 0.0 && progress < 1.0, "{name}");
        assert!(
            (progress - target).abs() <= 4.0 * ulp,
            "{name}: {progress} != {target}"
        );
    }
}

#[test]
fn public_typed_vectors_keep_complete_results_through_the_source_boundary() {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-adjustment-cases.json"
    ))
    .unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let source = json!({
            "schemaVersion": "0.1.0", "current": case["current"],
            "enabled": case["enabled"], "readOnly": case["readOnly"],
            "adjustment": case["adjustment"]
        });
        let result = resolve_slider_adjustment_source(&source.to_string());
        if let Some(expected) = case.get("expected") {
            assert_result(
                &serde_json::to_value(result.unwrap()).unwrap(),
                expected,
                case["name"].as_str().unwrap(),
            );
        } else {
            assert!(result.is_err(), "{}", case["name"]);
        }
    }
}

#[test]
fn source_rejects_precision_loss_in_current_and_intent_even_without_permission() {
    for enabled in [true, false] {
        for (kind, field) in [
            ("increase", "amount"),
            ("decrease", "amount"),
            ("setValue", "value"),
        ] {
            for literal in [
                "9007199254740993",
                "18446744073709551617",
                "-18446744073709551617",
            ] {
                let source = format!(
                    r#"{{"schemaVersion":"0.1.0","current":{{"schemaVersion":"0.1.0","minimum":-1e30,"maximum":1e30,"value":0}},"enabled":{enabled},"readOnly":false,"adjustment":{{"kind":"{kind}","{field}":{literal}}}}}"#
                );
                let error = resolve_slider_adjustment_source(&source).unwrap_err();
                assert!(error.to_string().contains("integer"), "{source}: {error}");
            }
        }
        for field in ["minimum", "maximum", "value"] {
            let mut document = request();
            document["enabled"] = json!(enabled);
            document["current"] =
                json!({"schemaVersion":"0.1.0", "minimum":-1e30,"maximum":1e30,"value":0});
            document["current"][field] = json!(9007199254740993_u64);
            assert!(resolve_slider_adjustment_source(&document.to_string()).is_err());
        }
    }
}

#[test]
fn source_preserves_representable_large_integers_and_endpoint_permission() {
    for literal in [
        "9007199254740994",
        "18446744073709551616",
        "-18446744073709551616",
    ] {
        let source = format!(
            r#"{{"schemaVersion":"0.1.0","current":{{"schemaVersion":"0.1.0","minimum":-1e30,"maximum":1e30,"value":0}},"enabled":true,"readOnly":false,"adjustment":{{"kind":"setValue","value":{literal}}}}}"#
        );
        let result = resolve_slider_adjustment_source(&source).unwrap();
        assert_eq!(
            result.value().value().value(),
            literal.parse::<f64>().unwrap()
        );
    }
    let mut document = request();
    document["enabled"] = json!(false);
    let result = resolve_slider_adjustment_source(&document.to_string()).unwrap();
    assert!(!result.accepted());
    assert!(!result.changed());
    assert_eq!(
        *result.value(),
        resolve_slider_value_source(&document["current"].to_string()).unwrap()
    );
}

#[test]
fn source_gate_rejects_duplicates_before_numeric_conversion() {
    let source = r#"{"schemaVersion":"0.1.0","current":{"schemaVersion":"0.1.0","minimum":0,"maximum":100,"value":25},"enabled":false,"readOnly":false,"adjustment":{"kind":"increase","amount":9007199254740993,"amount":1}}"#;
    let error = resolve_slider_adjustment_source(source).unwrap_err();
    assert!(error.to_string().contains("duplicate JSON member"));
}

#[test]
fn escaped_member_names_preserve_numeric_intents_and_duplicate_detection() {
    let baseline = request().to_string();
    let expected = resolve_slider_adjustment_source(&baseline).unwrap();
    for (field, escaped) in [("amount", "\\u0061mount"), ("kind", "\\u006bind")] {
        let source = baseline.replace(&format!("\"{field}\""), &format!("\"{escaped}\""));
        assert_eq!(resolve_slider_adjustment_source(&source).unwrap(), expected);
    }
    let mut absolute = request();
    absolute["adjustment"] = json!({"kind":"setValue", "value":35});
    let source = absolute.to_string().replace("\"value\"", "\"\\u0076alue\"");
    assert_eq!(resolve_slider_adjustment_source(&source).unwrap(), expected);
    let duplicate = baseline.replace("\"amount\":10", "\"amount\":10,\"\\u0061mount\":10");
    assert!(
        resolve_slider_adjustment_source(&duplicate)
            .unwrap_err()
            .to_string()
            .contains("duplicate")
    );
}

#[test]
fn strict_shapes_reject_unknown_missing_and_wrongly_typed_members() {
    let baseline = request();
    for field in [
        "schemaVersion",
        "current",
        "enabled",
        "readOnly",
        "adjustment",
    ] {
        let mut document = baseline.clone();
        document.as_object_mut().unwrap().remove(field);
        assert!(
            resolve_slider_adjustment_source(&document.to_string()).is_err(),
            "{field}"
        );
    }
    for adjustment in [
        json!({"kind":"minimum", "amount":1}),
        json!({"kind":"maximum", "value":1}),
        json!({"kind":"increase"}),
        json!({"kind":"increase", "amount":true}),
        json!({"kind":"setValue", "value":"50"}),
        json!({"kind":"increase", "amount":10, "value":50}),
        json!({"kind":"unknown"}),
    ] {
        let mut document = baseline.clone();
        document["adjustment"] = adjustment;
        assert!(
            resolve_slider_adjustment_source(&document.to_string()).is_err(),
            "{document}"
        );
    }
}

#[test]
fn public_protocol_cases_preserve_raw_literals_and_diagnostic_failures() {
    #[derive(serde::Deserialize)]
    struct Case<'a> {
        name: String,
        #[serde(borrow)]
        request: &'a serde_json::value::RawValue,
        expected: Option<Value>,
        #[serde(rename = "errorContains")]
        error_contains: Option<String>,
    }
    let cases: Vec<Case<'_>> = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-adjustment-protocol-cases.json"
    ))
    .unwrap();
    for case in cases {
        let result = resolve_slider_adjustment_source(case.request.get());
        if let Some(expected) = case.expected {
            assert_result(
                &serde_json::to_value(result.unwrap()).unwrap(),
                &expected,
                &case.name,
            );
        } else {
            let error = result.unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains(case.error_contains.as_ref().unwrap()),
                "{}: {error}",
                case.name
            );
        }
    }
}
