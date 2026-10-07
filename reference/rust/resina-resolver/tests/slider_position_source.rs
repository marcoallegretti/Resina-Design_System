use resina_resolver::resolve_slider_position_source;
use serde_json::{Value, json};

fn request() -> Value {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-position-protocol-cases.json"
    ))
    .unwrap();
    cases[0]["request"].clone()
}

#[test]
fn public_protocol_preserves_typed_results_and_strict_rejections() {
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
        "../../../../conformance/interaction/slider-position-protocol-cases.json"
    ))
    .unwrap();
    for case in cases {
        let result = resolve_slider_position_source(case.request.get());
        if let Some(expected) = case.expected {
            assert_eq!(
                serde_json::to_value(result.expect(&case.name)).unwrap(),
                expected,
                "{}",
                case.name
            );
        } else {
            let error = result.expect_err(&case.name);
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

#[test]
fn nested_source_retains_integer_literals_beyond_machine_integer_range() {
    let mut document = request();
    document["layout"]["value"] =
        json!({"schemaVersion":"0.1.0","minimum":0,"maximum":1e30,"value":0});
    let source = document
        .to_string()
        .replace("\"value\":0", "\"value\":18446744073709551617");
    assert!(
        resolve_slider_position_source(&source)
            .unwrap_err()
            .to_string()
            .contains("integer")
    );
    let exact = source.replace("18446744073709551617", "18446744073709551616");
    assert!(resolve_slider_position_source(&exact).is_ok());
    assert!(resolve_slider_position_source(&source).is_err());
}

#[test]
fn escaped_members_are_valid_and_decoded_duplicates_fail_before_mapping() {
    let source = request().to_string();
    for (member, escaped) in [
        ("layout", "\\u006cayout"),
        ("desiredOrigin", "\\u0064esiredOrigin"),
        ("value", "\\u0076alue"),
    ] {
        let escaped = source.replace(&format!("\"{member}\""), &format!("\"{escaped}\""));
        assert_eq!(
            resolve_slider_position_source(&escaped).unwrap(),
            resolve_slider_position_source(&source).unwrap()
        );
    }
    for (member, value, escaped) in [
        ("enabled", "true", "\\u0065nabled"),
        ("width", "160.0", "\\u0077idth"),
        ("minimum", "-10.0", "\\u006dinimum"),
    ] {
        let original = format!("\"{member}\":{value}");
        assert!(source.contains(&original));
        let duplicate = source.replacen(&original, &format!("{original},\"{escaped}\":{value}"), 1);
        assert!(
            resolve_slider_position_source(&duplicate)
                .unwrap_err()
                .to_string()
                .contains("duplicate JSON member")
        );
    }
}

#[test]
fn rebuilding_current_layout_preserves_stationary_values_under_live_permission() {
    let mut document = request();
    document["desiredOrigin"] = json!(72);
    for (minimum, maximum, current, target) in [
        (-10.0, 30.0, 0.0, 10.0),
        (0.0, 100.0, 25.0, 50.0),
        (-100.0, 0.0, -25.0, -50.0),
    ] {
        document["layout"]["value"] = json!({
            "schemaVersion":"0.1.0", "minimum":minimum,"maximum":maximum,"value":current
        });
        for (enabled, read_only) in [(true, false), (false, false), (true, true), (false, true)] {
            document["enabled"] = json!(enabled);
            document["readOnly"] = json!(read_only);
            let result = resolve_slider_position_source(&document.to_string()).unwrap();
            assert_eq!(result.accepted(), enabled && !read_only);
            assert_eq!(result.changed(), enabled && !read_only);
            assert_eq!(
                result.value().value().value(),
                if result.accepted() { target } else { current }
            );
        }
    }
    document["enabled"] = json!(true);
    document["readOnly"] = json!(false);
    document["layout"]["value"]["value"] = json!(-50.0);
    let stationary = resolve_slider_position_source(&document.to_string()).unwrap();
    assert!(stationary.accepted());
    assert!(!stationary.changed());
    assert_eq!(stationary.value().value().value(), -50.0);
}
