use resina_resolver::{OpaqueSurfaceError, resolve_opaque_surface_source};
use serde_json::Value;
use std::collections::BTreeMap;

const REQUEST: &str = include_str!("../../../../conformance/ir/opaque-surface-request.json");
const EXPECTED: &str = include_str!("../../../../conformance/ir/opaque-surface-expected.json");
const CASES: &str = include_str!("../../../../conformance/ir/opaque-surface-cases.json");

#[test]
fn cached_theme_matches_source_resolution() {
    let mut request: Value = serde_json::from_str(REQUEST).unwrap();
    request["surface"]["states"]["states"] = serde_json::json!(["focused", "rest"]);
    let sources: BTreeMap<String, String> =
        serde_json::from_value(request["theme"]["externalSources"].clone()).unwrap();
    let theme = resina_resolver::compile_theme_source_with_sources(
        request["theme"]["themeSource"].as_str().unwrap(),
        &sources,
    )
    .unwrap();
    let environment = serde_json::from_value(request["theme"]["environment"].clone()).unwrap();
    let surface = serde_json::from_value(request["surface"].clone()).unwrap();
    let appearance = serde_json::from_value(request["appearance"].clone()).unwrap();
    let backdrop = resina_color::resolve_srgb_fallback(&request["postTreatmentBackdrop"]).unwrap();
    let adjacent = resina_color::resolve_srgb_fallback(&request["adjacentColor"]).unwrap();
    let actual = resina_resolver::resolve_opaque_surface(
        &theme,
        &environment,
        resina_resolver::OpaqueSurfaceInput {
            surface: &surface,
            size: serde_json::from_value(request["size"].clone()).unwrap(),
            appearance: &appearance,
            foreground_role: serde_json::from_value(request["foregroundRole"].clone()).unwrap(),
            post_treatment_backdrop: Some(&backdrop),
            adjacent_color: &adjacent,
            minimum_content_contrast: request["minimumContentContrast"].as_f64().unwrap(),
            minimum_edge_contrast: request["minimumEdgeContrast"].as_f64().unwrap(),
        },
    )
    .unwrap();
    assert_eq!(
        actual,
        resolve_opaque_surface_source(&request.to_string()).unwrap()
    );
    let surrounding = resina_color::resolve_srgb_fallback(&serde_json::json!({
        "colorSpace": "srgb", "components": [0, 0, 0], "alpha": 1
    }))
    .unwrap();
    let focus = resina_resolver::resolve_focus_ir(
        &theme,
        &environment,
        resina_resolver::FocusIrInput {
            surface: &surface,
            size: serde_json::from_value(request["size"].clone()).unwrap(),
            shape_assignments: appearance.shape_assignments(),
            depth_assignments: appearance.depth_assignments(),
            key_light: appearance.key_light(),
            surrounding_color: &surrounding,
        },
    )
    .unwrap();
    assert_eq!(
        actual.geometry().silhouette(),
        focus.geometry().silhouette()
    );
    let body_value = serde_json::to_value(&actual).unwrap();
    let focus_value = serde_json::to_value(&focus).unwrap();
    assert_eq!(
        body_value["states"],
        focus_value["indicator"]["binding"]["states"]
    );
}

fn changes(document: &Value, changes: &Value) -> Value {
    let mut document = document.clone();
    if let Some(changes) = changes.as_array() {
        for change in changes {
            *document
                .pointer_mut(change["path"].as_str().unwrap())
                .unwrap() = change["value"].clone();
        }
    }
    document
}

fn compare(actual: &Value, expected: &Value) {
    match expected {
        Value::Object(fields) => {
            assert_eq!(actual.as_object().unwrap().len(), fields.len());
            for (name, value) in fields {
                compare(&actual[name], value);
            }
        }
        Value::Array(values) => {
            assert_eq!(actual.as_array().unwrap().len(), values.len());
            for (a, e) in actual.as_array().unwrap().iter().zip(values) {
                compare(a, e);
            }
        }
        Value::Number(value) => {
            assert!((actual.as_f64().unwrap() - value.as_f64().unwrap()).abs() <= 1e-12)
        }
        _ => assert_eq!(actual, expected),
    }
}

#[test]
fn public_opaque_surface_vectors() {
    let request: Value = serde_json::from_str(REQUEST).unwrap();
    let expected: Value = serde_json::from_str(EXPECTED).unwrap();
    let cases: Vec<Value> = serde_json::from_str(CASES).unwrap();
    for case in cases {
        let input = changes(&request, &case["requestChanges"]);
        let result = resolve_opaque_surface_source(&input.to_string());
        if let Some(message) = case["errorContains"].as_str() {
            let error = result.unwrap_err().to_string();
            assert!(error.contains(message), "{}: {error}", case["name"]);
        } else {
            let result = serde_json::to_value(result.unwrap()).unwrap();
            let expected = case
                .get("expected")
                .cloned()
                .unwrap_or_else(|| changes(&expected, &case["expectedChanges"]));
            compare(&result, &expected);
        }
    }
}

#[test]
fn real_translucency_is_rejected_without_changing_the_environment() {
    let mut request: Value = serde_json::from_str(REQUEST).unwrap();
    request["surface"]["materialRole"] = "surface.chrome".into();
    request["theme"]["environment"]["accessibilityPreferences"]["reducedTransparency"] =
        false.into();
    request["theme"]["environment"]["accessibilityPreferences"]["highContrast"] = false.into();
    request["theme"]["environment"]["rendererCapabilities"]["translucentSurfaces"] = true.into();
    request["minimumContentContrast"] = 1.into();
    assert!(matches!(
        resolve_opaque_surface_source(&request.to_string()),
        Err(OpaqueSurfaceError::TranslucentBody)
    ));
}

#[test]
fn missing_frost_backdrop_and_unknown_root_fields_fail() {
    let mut request: Value = serde_json::from_str(REQUEST).unwrap();
    request["surface"]["materialRole"] = "surface.chrome".into();
    request
        .as_object_mut()
        .unwrap()
        .remove("postTreatmentBackdrop");
    assert!(
        resolve_opaque_surface_source(&request.to_string())
            .unwrap_err()
            .to_string()
            .contains("requires postTreatmentBackdrop")
    );
    request["backend"] = "qt".into();
    assert!(
        resolve_opaque_surface_source(&request.to_string())
            .unwrap_err()
            .to_string()
            .contains("unknown field")
    );
}
