use resina_resolver::resolve_slider_layout_source;
use serde_json::{Value, json};

fn request() -> Value {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/slider-layout-cases.json"
    ))
    .unwrap();
    let mut document = cases["cases"][0]["input"].clone();
    document["schemaVersion"] = json!("0.1.0");
    document
}

#[test]
fn typed_geometry_vectors_keep_complete_results_through_the_source_boundary() {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/slider-layout-cases.json"
    ))
    .unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let mut request = case["input"].clone();
        request["schemaVersion"] = json!("0.1.0");
        let result = resolve_slider_layout_source(&request.to_string());
        if let Some(expected) = case.get("expected") {
            assert_eq!(
                serde_json::to_value(result.unwrap()).unwrap(),
                *expected,
                "{}",
                case["name"]
            );
        } else {
            assert!(result.is_err(), "{}", case["name"]);
        }
    }
}

#[test]
fn current_value_keeps_strict_source_precision_and_rejects_supplied_progress() {
    let mut document = request();
    document["value"] =
        json!({"schemaVersion":"0.1.0", "minimum":0,"maximum":1e30,"value":9007199254740993_u64});
    let error = resolve_slider_layout_source(&document.to_string()).unwrap_err();
    assert!(error.to_string().contains("integer"));
    document["value"]["value"] = json!(9007199254740994_u64);
    let result = resolve_slider_layout_source(&document.to_string()).unwrap();
    assert_eq!(result.value().value().value(), 9007199254740994.0);
    document["value"]["progress"] = json!(0.5);
    assert!(resolve_slider_layout_source(&document.to_string()).is_err());
}

#[test]
fn all_inputs_are_required_and_nested_shapes_are_strict() {
    let baseline = request();
    for field in baseline.as_object().unwrap().keys() {
        let mut document = baseline.clone();
        document.as_object_mut().unwrap().remove(field);
        assert!(
            resolve_slider_layout_source(&document.to_string()).is_err(),
            "{field}"
        );
    }
    for field in ["allocationSize", "thumbSize", "insets", "value"] {
        let mut document = baseline.clone();
        document[field]["backend"] = json!("guido");
        assert!(
            resolve_slider_layout_source(&document.to_string()).is_err(),
            "{field}"
        );
    }
    for field in ["layoutDirection", "orientation", "minimumPosition"] {
        let mut document = baseline.clone();
        document[field] = json!("unknown");
        assert!(
            resolve_slider_layout_source(&document.to_string()).is_err(),
            "{field}"
        );
    }
}

#[test]
fn duplicate_nested_members_fail_before_value_precision_or_geometry_resolution() {
    let source = request().to_string();
    for (member, value) in [("width", "160.0"), ("start", "12.0"), ("minimum", "-10.0")] {
        let original = format!("\"{member}\":{value}");
        assert!(source.contains(&original), "{source}");
        let duplicate = source.replacen(&original, &format!("{original},{original}"), 1);
        let error = resolve_slider_layout_source(&duplicate).unwrap_err();
        assert!(
            error.to_string().contains("duplicate JSON member"),
            "{error}"
        );
    }
}

#[test]
fn public_protocol_cases_preserve_results_and_diagnostic_failures() {
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
        "../../../../conformance/geometry/slider-layout-protocol-cases.json"
    ))
    .unwrap();
    for case in cases {
        let result = resolve_slider_layout_source(case.request.get());
        if let Some(expected) = case.expected {
            let actual = serde_json::to_value(result.unwrap()).unwrap();
            for field in [
                "schemaVersion",
                "layoutDirection",
                "orientation",
                "minimumPosition",
            ] {
                assert_eq!(actual[field], expected[field], "{}: {field}", case.name);
            }
            for field in ["minimum", "maximum", "value"] {
                assert_eq!(
                    actual["value"][field].as_f64(),
                    expected["value"][field].as_f64(),
                    "{}: {field}",
                    case.name
                );
            }
            let progress = actual["value"]["progress"].as_f64().unwrap();
            let target = expected["value"]["progress"].as_f64().unwrap();
            if target == 0.0 || target == 1.0 {
                assert_eq!(progress, target, "{}", case.name);
            } else {
                let ulp = f64::from_bits(target.to_bits() + 1) - target;
                assert!(
                    progress > 0.0 && progress < 1.0 && (progress - target).abs() <= 4.0 * ulp,
                    "{}",
                    case.name
                );
            }
            for coordinate in ["x", "y", "width", "height"] {
                assert_eq!(
                    actual["allocationBounds"][coordinate].as_f64(),
                    expected["allocationBounds"][coordinate].as_f64(),
                    "{}",
                    case.name
                );
            }
            for field in [
                "trackBounds",
                "minimumThumbBounds",
                "maximumThumbBounds",
                "thumbBounds",
            ] {
                for coordinate in ["x", "y", "width", "height"] {
                    let value = actual[field][coordinate].as_f64().unwrap();
                    let target = expected[field][coordinate].as_f64().unwrap();
                    assert!(
                        (value - target).abs()
                            <= 1e-12_f64.max(1e-12 * value.abs().max(target.abs())),
                        "{}: {field}/{coordinate}",
                        case.name
                    );
                }
            }
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
