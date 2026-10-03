use resina_model::PhysicalVector;
use resina_resolver::{SurfacePaintError, resolve_focus_ir_source};
use serde_json::{Value, json};

const REQUEST: &str = include_str!("../../../../conformance/ir/focus-ir-request.json");

#[test]
fn focus_paint_uses_authored_color_and_guarded_fallback() {
    let mut request: Value = serde_json::from_str(REQUEST).unwrap();
    let mut theme: Value =
        serde_json::from_str(request["theme"]["themeSource"].as_str().unwrap()).unwrap();
    theme["tokens"]["palette"]["focusPaint"] =
        json!({"$value": {"colorSpace":"srgb", "components":[0.2,0.6,0.8], "alpha":1}});
    for section in ["colorAssignments", "opaqueColorAssignments"] {
        theme[section]["roles"]["focus"] = json!("palette.focusPaint");
    }
    request["theme"]["themeSource"] = json!(theme.to_string());
    let ir = resolve_focus_ir_source(&request.to_string()).unwrap();
    assert!(!ir.indicator().fallback_applied());
    let point = PhysicalVector { x: 10.0, y: -3.0 };
    let color = ir.sample_paint(point).unwrap().unwrap();
    assert_eq!(color.components(), [0.2, 0.6, 0.8]);
    assert_eq!(color.alpha(), 1.0);
    for section in ["colorAssignments", "opaqueColorAssignments"] {
        theme[section]["roles"]["focus"] = json!("palette.black");
        theme[section]["roles"]["outline.strong"] = json!("palette.base");
    }
    request["theme"]["themeSource"] = json!(theme.to_string());
    let fallback_ir = resolve_focus_ir_source(&request.to_string()).unwrap();
    assert!(fallback_ir.indicator().fallback_applied());
    let fallback = fallback_ir.sample_paint(point).unwrap().unwrap();
    assert_eq!(&fallback, fallback_ir.indicator().color());
    assert_ne!(fallback.components(), color.components());
    assert_eq!(fallback.alpha(), 1.0);
}

#[test]
fn public_focus_paint_vectors_preserve_color_and_hole() {
    let ir = resolve_focus_ir_source(REQUEST).unwrap();
    let before = serde_json::to_value(&ir).unwrap();
    let vectors: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/ir/focus-paint-vectors.json"
    ))
    .unwrap();
    for vector in vectors {
        let point = serde_json::from_value(vector["point"].clone()).unwrap();
        let actual = ir.sample_paint(point).unwrap();
        assert_eq!(
            actual.is_some(),
            vector["covered"].as_bool().unwrap(),
            "{}",
            vector["name"]
        );
        if let Some(color) = actual {
            assert_eq!(&color, ir.indicator().color());
            assert_eq!(color.alpha(), 1.0);
        }
    }
    assert_eq!(serde_json::to_value(&ir).unwrap(), before);
}

#[test]
fn nonfinite_focus_points_fail_explicitly() {
    let ir = resolve_focus_ir_source(REQUEST).unwrap();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for point in [
            PhysicalVector { x: value, y: 0.0 },
            PhysicalVector { x: 0.0, y: value },
        ] {
            assert_eq!(ir.sample_paint(point), Err(SurfacePaintError::InvalidPoint));
        }
    }
}

#[test]
fn circular_focus_coverage_matches_distance_to_swept_center() {
    let mut request: Value = serde_json::from_str(REQUEST).unwrap();
    request["size"] = json!({"width":10,"height":10});
    request["surface"]["form"]["shape"] = json!("capsule");
    for direction in [
        PhysicalVector { x: 0.0, y: -1.0 },
        PhysicalVector { x: -1.0, y: 0.0 },
        PhysicalVector { x: -0.6, y: -0.8 },
        PhysicalVector { x: 0.6, y: 0.8 },
    ] {
        request["keyLight"]["direction"] = serde_json::to_value(direction).unwrap();
        for elevation in ["base", "raised"] {
            request["surface"]["form"]["elevation"] = json!(elevation);
            let ir = resolve_focus_ir_source(&request.to_string()).unwrap();
            let depth = if elevation == "base" { 0.0 } else { 2.0 };
            let offset = PhysicalVector {
                x: -depth * direction.x,
                y: -depth * direction.y,
            };
            for y in -60..180 {
                for x in -60..180 {
                    let point = PhysicalVector {
                        x: f64::from(x) / 10.0 + 0.037,
                        y: f64::from(y) / 10.0 + 0.029,
                    };
                    let delta = PhysicalVector {
                        x: point.x - 5.0,
                        y: point.y - 5.0,
                    };
                    let t = if depth == 0.0 {
                        0.0
                    } else {
                        ((delta.x * offset.x + delta.y * offset.y) / (depth * depth))
                            .clamp(0.0, 1.0)
                    };
                    let distance = (delta.x - t * offset.x).hypot(delta.y - t * offset.y);
                    let covered = distance > 7.0 && distance <= 9.0;
                    assert_eq!(
                        ir.sample_paint(point).unwrap().is_some(),
                        covered,
                        "{point:?}, {offset:?}"
                    );
                }
            }
        }
    }
}
