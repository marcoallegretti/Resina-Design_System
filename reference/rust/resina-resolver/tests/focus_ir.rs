mod support;

use resina_color::resolve_srgb_fallback;
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ElevationDepthAssignments, KeyLight, PhysicalVector, ShapeFallbackAssignments, SurfaceIntent,
    SurfaceSize,
};
use resina_resolver::{
    FocusIrInput, compile_theme_source_with_sources, resolve_focus_ir, resolve_focus_ir_source,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const REQUEST: &str = include_str!("../../../../conformance/ir/focus-ir-request.json");
const EXPECTED: &str = include_str!("../../../../conformance/ir/focus-ir-expected.json");
const CASES: &str = include_str!("../../../../conformance/ir/focus-ir-cases.json");

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

fn compare(actual: &Value, expected: &Value, path: &str) {
    match expected {
        Value::Object(fields) => {
            assert_eq!(actual.as_object().unwrap().len(), fields.len(), "{path}");
            for (name, value) in fields {
                compare(&actual[name], value, &format!("{path}/{name}"));
            }
        }
        Value::Array(values) => {
            assert_eq!(actual.as_array().unwrap().len(), values.len(), "{path}");
            for (i, (a, e)) in actual.as_array().unwrap().iter().zip(values).enumerate() {
                compare(a, e, &format!("{path}/{i}"));
            }
        }
        Value::Number(value) => assert!(
            (actual.as_f64().unwrap() - value.as_f64().unwrap()).abs() <= 1e-12,
            "{path}: {actual} != {expected}"
        ),
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn public_focus_ir_vectors() {
    let request: Value = serde_json::from_str(REQUEST).unwrap();
    let expected: Value = serde_json::from_str(EXPECTED).unwrap();
    let cases: Vec<Value> = serde_json::from_str(CASES).unwrap();
    for case in cases {
        let input = changes(&request, &case["requestChanges"]);
        let result = resolve_focus_ir_source(&input.to_string());
        if let Some(message) = case["errorContains"].as_str() {
            let error = result.unwrap_err().to_string();
            assert!(error.contains(message), "{}: {error}", case["name"]);
        } else {
            let expected = case
                .get("expected")
                .cloned()
                .unwrap_or_else(|| changes(&expected, &case["expectedChanges"]));
            compare(
                &serde_json::to_value(result.unwrap()).unwrap(),
                &expected,
                case["name"].as_str().unwrap(),
            );
        }
    }
}

#[test]
fn cached_theme_matches_source_focus_resolution() {
    let request: Value = serde_json::from_str(REQUEST).unwrap();
    let sources = serde_json::from_value(request["theme"]["externalSources"].clone()).unwrap();
    let theme = compile_theme_source_with_sources(
        request["theme"]["themeSource"].as_str().unwrap(),
        &sources,
    )
    .unwrap();
    let environment = serde_json::from_value(request["theme"]["environment"].clone()).unwrap();
    let surface = serde_json::from_value(request["surface"].clone()).unwrap();
    let shapes = serde_json::from_value(request["shapeAssignments"].clone()).unwrap();
    let depths = serde_json::from_value(request["depthAssignments"].clone()).unwrap();
    let light = serde_json::from_value(request["keyLight"].clone()).unwrap();
    let surround = resolve_srgb_fallback(&request["surroundingColor"]).unwrap();
    let result = resolve_focus_ir(
        &theme,
        &environment,
        FocusIrInput {
            surface: &surface,
            size: serde_json::from_value(request["size"].clone()).unwrap(),
            shape_assignments: &shapes,
            depth_assignments: &depths,
            key_light: &light,
            surrounding_color: &surround,
        },
    )
    .unwrap();
    assert_eq!(result, resolve_focus_ir_source(REQUEST).unwrap());
}

#[test]
fn authored_focus_ring_is_a_parallel_offset_of_the_actual_silhouette() {
    let shapes: ShapeFallbackAssignments =
        serde_json::from_str(include_str!("../../../../definitions/tier0-shapes.json")).unwrap();
    let depths: ElevationDepthAssignments =
        serde_json::from_str(include_str!("../../../../definitions/tier0-depth.json")).unwrap();
    let sources = BTreeMap::from([(
        "foundation.json".to_owned(),
        include_str!("../../../../tokens/foundation.json").to_owned(),
    )]);
    let template: Value = serde_json::from_str(REQUEST).unwrap();
    for source in [
        include_str!("../../../../tokens/themes/light.json"),
        include_str!("../../../../tokens/themes/dark.json"),
    ] {
        let theme = compile_theme_source_with_sources(source, &sources).unwrap();
        for direction in ["ltr", "rtl"] {
            let mut environment: Value = serde_json::from_str(include_str!(
                "../../../../conformance/environment/valid-minimal-capabilities.json"
            ))
            .unwrap();
            environment["layoutDirection"] = direction.into();
            environment["textScale"] = 3.into();
            let environment: EnvironmentSnapshot = serde_json::from_value(environment).unwrap();
            let resolution = theme.resolve(&environment).unwrap();
            let surrounding =
                &resolution.opaque_color_fallbacks()[&resina_model::ColorRole::SurfaceBase];
            for light in [
                PhysicalVector { x: -1.0, y: -1.0 },
                PhysicalVector { x: 1.0, y: -1.0 },
                PhysicalVector { x: 0.0, y: 1.0 },
                PhysicalVector { x: 1.0, y: 0.0 },
            ] {
                let light = KeyLight::try_new(light).unwrap();
                for role in [
                    "surface.base",
                    "surface.chrome",
                    "control.primary",
                    "feedback.selection",
                ] {
                    for shape in ["structural", "soft", "rounded", "capsule", "organic"] {
                        for (elevation, depth) in [
                            ("embedded", 0.0),
                            ("base", 0.0),
                            ("raised", 2.0),
                            ("floating", 4.0),
                            ("overlay", 6.0),
                            ("modal", 8.0),
                        ] {
                            let mut surface = template["surface"].clone();
                            surface["materialRole"] = role.into();
                            surface["form"]["shape"] = shape.into();
                            surface["form"]["elevation"] = elevation.into();
                            surface["states"]["states"] = json!([
                                "focused", "pressed", "selected", "disabled", "busy", "error"
                            ]);
                            let surface: SurfaceIntent = serde_json::from_value(surface).unwrap();
                            let ir = resolve_focus_ir(
                                &theme,
                                &environment,
                                FocusIrInput {
                                    surface: &surface,
                                    size: SurfaceSize {
                                        width: 80.0,
                                        height: 40.0,
                                    },
                                    shape_assignments: &shapes,
                                    depth_assignments: &depths,
                                    key_light: &light,
                                    surrounding_color: surrounding,
                                },
                            )
                            .unwrap();
                            assert_eq!(ir.indicator().binding().states(), surface.states());
                            assert!(ir.indicator().contrast_ratio() >= 3.0);
                            let geometry = ir.geometry();
                            let key_direction = light.direction();
                            let length = key_direction.x.hypot(key_direction.y);
                            let dx = -key_direction.x / length * depth;
                            let dy = -key_direction.y / length * depth;
                            let bounds = geometry.silhouette().bounds().unwrap();
                            for (actual, expected) in [
                                (bounds.x, dx.min(0.0)),
                                (bounds.y, dy.min(0.0)),
                                (bounds.width, 80.0 + dx.abs()),
                                (bounds.height, 40.0 + dy.abs()),
                            ] {
                                assert!(
                                    (actual - expected).abs() <= 1e-10,
                                    "{role} {shape} {elevation} {direction:?}"
                                );
                            }
                            for degrees in 0..360 {
                                let angle = f64::from(degrees).to_radians();
                                let n = PhysicalVector {
                                    x: angle.cos(),
                                    y: angle.sin(),
                                };
                                let silhouette = support::contour_support(geometry.silhouette(), n);
                                for (placed, extent) in
                                    [(geometry.inner(), 2.0), (geometry.outer(), 4.0)]
                                {
                                    let actual = support::contour_support(placed.contour(), n)
                                        + placed.offset().x * n.x
                                        + placed.offset().y * n.y;
                                    assert!(
                                        (actual - silhouette - extent).abs() <= 1e-10,
                                        "{role} {shape} {elevation} {direction}: {actual} {silhouette}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
