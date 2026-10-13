use resina_resolver::resolve_outline_focus_source;
use serde_json::{Value, json};

fn baseline() -> Value {
    let bindings: Value = serde_json::from_str(include_str!(
        "../../../../conformance/surfaces/binding-vectors.json"
    ))
    .unwrap();
    let mut surface = bindings[0]["document"].clone();
    surface["treatmentStack"]["treatments"] = json!(["none"]);
    let outlines: Value = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/surface-outline-vectors.json"
    ))
    .unwrap();
    json!({
        "schemaVersion":"0.1.0",
        "focus": {
            "schemaVersion":"0.1.0",
            "scenario": {
                "schemaVersion":"0.4.0",
                "resolution":serde_json::from_str::<Value>(include_str!(
                    "../../../../conformance/headless/valid-request.json"
                )).unwrap(),
                "surface":surface
            },
            "surroundingColor":{"colorSpace":"srgb","components":[1,1,1],"alpha":1}
        },
        "outline":outlines[0]["request"],
        "sideOffset":{"x":6,"y":-4},
        "directions":[{"x":1,"y":0},{"x":0,"y":1},{"x":-1,"y":0},{"x":0,"y":-1}]
    })
}

#[test]
fn nested_outline_must_retain_original_numeric_source() {
    for literal in ["1e-400", "-1e-400"] {
        let mut request = baseline();
        request["outline"]["radii"]["topStart"]["x"] = json!(-9999.0);
        let source = request.to_string().replace("-9999.0", literal);
        assert!(resolve_outline_focus_source(&source).is_err(), "{literal}");
    }
}

fn resolve(request: &Value) -> Value {
    serde_json::to_value(resolve_outline_focus_source(&request.to_string()).unwrap()).unwrap()
}

#[test]
fn cardinal_support_includes_the_actual_signed_projection_and_exact_disk_extents() {
    let result = resolve(&baseline());
    let expected = [
        [(126.0, 44.0), (128.0, 44.0), (130.0, 44.0)],
        [(100.0, 60.0), (100.0, 62.0), (100.0, 64.0)],
        [(0.0, 48.0), (-2.0, 48.0), (-4.0, 48.0)],
        [(106.0, -4.0), (106.0, -6.0), (106.0, -8.0)],
    ];
    for (query, points) in result["supports"].as_array().unwrap().iter().zip(expected) {
        for (boundary, (x, y)) in ["silhouette", "inner", "outer"].into_iter().zip(points) {
            assert_eq!(query[boundary]["point"], json!({"x":x,"y":y}));
        }
        let support = |boundary| query[boundary]["support"].as_f64().unwrap();
        assert_eq!(support("inner") - support("silhouette"), 2.0);
        assert_eq!(support("outer") - support("inner"), 2.0);
    }
    assert_eq!(
        result["focus"]["geometry"]["silhouetteBounds"],
        json!({"x":0.0,"y":-4.0,"width":126.0,"height":64.0})
    );
    assert_eq!(
        result["focus"]["geometry"]["innerBounds"],
        json!({"x":-2.0,"y":-6.0,"width":130.0,"height":68.0})
    );
    assert_eq!(
        result["focus"]["geometry"]["outerBounds"],
        json!({"x":-4.0,"y":-8.0,"width":134.0,"height":72.0})
    );
}

#[test]
fn every_exposed_support_preserves_the_euclidean_gap_instead_of_inflating_quartic_axes() {
    let mut request = baseline();
    request["directions"] = json!(
        (0..360)
            .map(|degrees| {
                let angle = f64::from(degrees).to_radians();
                json!({"x":angle.cos(),"y":angle.sin()})
            })
            .collect::<Vec<_>>()
    );
    for curve in ["circular", "quartic"] {
        request["outline"]["curvature"] = json!(curve);
        for offset in [(0.0, 0.0), (6.0, -4.0), (-6.0, 4.0)] {
            request["sideOffset"] = json!({"x":offset.0,"y":offset.1});
            let result = resolve(&request);
            for query in result["supports"].as_array().unwrap() {
                let nx = query["normal"]["x"].as_f64().unwrap();
                let ny = query["normal"]["y"].as_f64().unwrap();
                let support = |boundary| query[boundary]["support"].as_f64().unwrap();
                assert!((support("inner") - support("silhouette") - 2.0).abs() < 1e-12);
                assert!((support("outer") - support("inner") - 2.0).abs() < 1e-12);
                for boundary in ["silhouette", "inner", "outer"] {
                    let point = &query[boundary]["point"];
                    let dot = point["x"].as_f64().unwrap() * nx + point["y"].as_f64().unwrap() * ny;
                    assert!((dot - support(boundary)).abs() < 1e-12);
                }
                let shifted = nx * offset.0 + ny * offset.1 > 0.0;
                let point = &query["silhouette"]["point"];
                let x = point["x"].as_f64().unwrap() - if shifted { offset.0 } else { 0.0 };
                let y = point["y"].as_f64().unwrap() - if shifted { offset.1 } else { 0.0 };
                let u = (x - if nx >= 0.0 { 100.0 } else { 20.0 }).abs() / 20.0;
                let v = (y - if ny >= 0.0 { 48.0 } else { 12.0 }).abs() / 12.0;
                let power = if curve == "circular" { 2 } else { 4 };
                assert!((u.powi(power) + v.powi(power) - 1.0).abs() < 1e-12);
                let gradient_x = nx.signum() * u.powi(power - 1) / 20.0;
                let gradient_y = ny.signum() * v.powi(power - 1) / 12.0;
                assert!((nx * gradient_y - ny * gradient_x).abs() < 1e-12);
            }
        }
    }
}

#[test]
fn quartic_diagonal_matches_an_independently_evaluated_power_boundary() {
    let mut request = baseline();
    for corner in ["topStart", "topEnd", "bottomEnd", "bottomStart"] {
        request["outline"]["radii"][corner] = json!({"x":24,"y":24});
    }
    request["sideOffset"] = json!({"x":0,"y":0});
    request["directions"] = json!([{"x":1,"y":1}]);
    let result = resolve(&request);
    let query = &result["supports"][0];
    for (boundary, x, y) in [
        ("silhouette", 116.18151396608916, 56.18151396608915),
        ("inner", 117.59572752846225, 57.59572752846224),
        ("outer", 119.00994109083534, 59.00994109083534),
    ] {
        assert!((query[boundary]["point"]["x"].as_f64().unwrap() - x).abs() < 1e-12);
        assert!((query[boundary]["point"]["y"].as_f64().unwrap() - y).abs() < 1e-12);
    }
    assert!((query["silhouette"]["support"].as_f64().unwrap() - 121.87906587668958).abs() < 1e-12);
}

#[test]
fn rounded_dot_agreement_does_not_certify_cancelled_projection_support() {
    let mut request = baseline();
    for corner in ["topStart", "topEnd", "bottomEnd", "bottomStart"] {
        request["outline"]["radii"][corner] = json!({"x":0,"y":0});
    }
    request["sideOffset"] = json!({"x":4e12,"y":(-3e12_f64).next_up()});
    request["directions"] = json!([{"x":3,"y":4}]);
    assert!(resolve_outline_focus_source(&request.to_string()).is_err());
}

#[test]
fn invalid_directions_unresolved_treatment_and_unrepresentable_ring_reject_atomically() {
    for direction in [json!({"x":0,"y":0}), json!([1, 0]), json!({"x":1})] {
        let mut request = baseline();
        request["directions"] = json!([direction]);
        assert!(resolve_outline_focus_source(&request.to_string()).is_err());
    }
    let mut request = baseline();
    request["focus"]["scenario"]["surface"]["treatmentStack"]["treatments"] = json!(["lens"]);
    assert!(resolve_outline_focus_source(&request.to_string()).is_err());
    let mut request = baseline();
    request["sideOffset"] = json!({"x":1e308,"y":0});
    assert!(resolve_outline_focus_source(&request.to_string()).is_err());
}

#[test]
fn public_cases_preserve_outline_color_and_state_contracts() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/states/outline-focus-cases.json"
    ))
    .unwrap();
    let outlines: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/surface-outline-vectors.json"
    ))
    .unwrap();
    for case in cases {
        let mut request = baseline();
        let outline = outlines
            .iter()
            .find(|item| item["name"] == case["outlineVector"])
            .unwrap();
        request["outline"] = outline["request"].clone();
        for change in case["changes"].as_array().unwrap() {
            *request
                .pointer_mut(change["path"].as_str().unwrap())
                .unwrap() = change["value"].clone();
        }
        let result = resolve_outline_focus_source(&request.to_string());
        if case.get("accuracyProbe").is_some() {
            assert!(
                result.is_err(),
                "reference precision gate: {}",
                case["name"]
            );
            continue;
        }
        if let Some(expected) = case.get("expected") {
            let result = result.unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
            let actual = serde_json::to_value(&result).unwrap();
            assert_numeric_json(
                &actual["focus"]["geometry"]["outline"],
                &outline["expected"],
            );
            assert_eq!(
                actual["focus"]["indicator"]["colorRole"],
                expected["colorRole"]
            );
            assert_eq!(
                actual["focus"]["indicator"]["fallbackApplied"],
                expected["fallbackApplied"]
            );
            if let Some(states) = expected.get("states") {
                assert_eq!(
                    &actual["focus"]["indicator"]["binding"]["states"]["states"],
                    states
                );
            }
            assert_eq!(
                actual["supports"].as_array().unwrap().len(),
                request["directions"].as_array().unwrap().len()
            );
            let repeated = resolve_outline_focus_source(&request.to_string()).unwrap();
            assert_eq!(result, repeated);
        } else {
            assert!(result.is_err(), "{}", case["name"]);
        }
    }
}

#[test]
fn typed_geometry_and_public_queries_agree_without_mutating_the_outline() {
    let request = baseline();
    let indicator =
        resina_resolver::resolve_focus_indicator_source(&request["focus"].to_string()).unwrap();
    let outline =
        resina_resolver::resolve_surface_outline_source(&request["outline"].to_string()).unwrap();
    let focus = resina_resolver::resolve_outline_focus(
        indicator.clone(),
        outline.clone(),
        resina_model::PhysicalVector { x: 6.0, y: -4.0 },
    )
    .unwrap();
    assert_eq!(focus.indicator(), &indicator);
    assert_eq!(focus.geometry().outline(), &outline);
    let public = resolve_outline_focus_source(&request.to_string()).unwrap();
    assert_eq!(public.focus(), &focus);
    for (direction, support) in request["directions"]
        .as_array()
        .unwrap()
        .iter()
        .zip(public.supports())
    {
        assert_eq!(
            &focus
                .geometry()
                .support_at(serde_json::from_value(direction.clone()).unwrap())
                .unwrap(),
            support
        );
    }
}

fn assert_numeric_json(actual: &Value, expected: &Value) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!((a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.0));
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len());
            for (key, value) in b {
                assert_numeric_json(&a[key], value);
            }
        }
        _ => assert_eq!(actual, expected),
    }
}
