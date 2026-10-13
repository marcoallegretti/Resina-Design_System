use resina_model::PhysicalVector;
use resina_resolver::resolve_material_relief_paint_source;
use serde_json::{Value, json};

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../../conformance/ir/material-relief-paint-vectors.json"
    ))
    .unwrap()
}

#[test]
fn every_authored_light_policy_preserves_complete_content_and_edge_guards() {
    for case in cases()
        .into_iter()
        .filter(|case| case.get("expected").is_some())
    {
        let paint = resolve_material_relief_paint_source(&case["request"].to_string())
            .unwrap()
            .paint;
        let content = paint.content_bounds();
        for ix in 0..=10 {
            for iy in 0..=10 {
                let point = PhysicalVector {
                    x: content.x + content.width * f64::from(ix) / 10.0,
                    y: content.y + content.height * f64::from(iy) / 10.0,
                };
                assert_eq!(
                    paint.sample_paint(point).unwrap().as_ref(),
                    Some(paint.pigment().body()),
                    "{}: {point:?}",
                    case["name"]
                );
            }
        }
        let bounds = paint.paint_color_ranges();
        assert_eq!(bounds[2].lower(), paint.edge().color());
        assert_eq!(bounds[2].upper(), paint.edge().color());
    }
}

#[test]
fn full_tilt_and_recess_modulate_only_the_interior_face() {
    for name in [
        "cast-convex-opaque-face-active-light",
        "cast-recessed-opaque-face-active-light",
    ] {
        let case = cases()
            .into_iter()
            .find(|case| case["name"] == name)
            .unwrap();
        let paint = resolve_material_relief_paint_source(&case["request"].to_string())
            .unwrap()
            .paint;
        let slope = if name.contains("recessed") {
            -0.234375_f64
        } else {
            0.234375_f64
        };
        let q = 0.5 * 0.75 * slope / (1.0 + slope * slope).sqrt() / 2.0_f64.sqrt();
        let tip = if q > 0.0 {
            paint.pigment().highlight()
        } else {
            paint.pigment().side()
        };
        let color = paint
            .sample_paint(PhysicalVector { x: 60.0, y: 2.0 })
            .unwrap()
            .unwrap();
        for ((actual, body), endpoint) in color
            .components()
            .into_iter()
            .zip(paint.pigment().body().components())
            .zip(tip.components())
        {
            assert!((actual - (body + q.abs() * (endpoint - body))).abs() <= 1e-12);
        }
        assert_eq!(
            paint
                .sample_paint(PhysicalVector { x: 60.0, y: 0.5 })
                .unwrap()
                .as_ref(),
            Some(paint.edge().color())
        );
        assert!(
            paint
                .sample_paint(PhysicalVector { x: -1.0, y: 30.0 })
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn suppressed_channels_retain_original_source_admission() {
    let mut request = cases()
        .into_iter()
        .find(|case| case["name"] == "cast-convex-opaque-face-active-light")
        .unwrap()["request"]
        .clone();
    request["enabled"] = json!(false);
    request["environment"]["accessibilityPreferences"]["highContrast"] = json!(true);
    for (member, value) in [("ambient", "0.5"), ("strength", "0.25")] {
        let fragment = format!("\"{member}\":{value}");
        for literal in ["-1e-400", "-4e-324"] {
            let source = request
                .to_string()
                .replace(&fragment, &format!("\"{member}\":{literal}"));
            assert!(
                resolve_material_relief_paint_source(&source).is_err(),
                "{member}: {literal}"
            );
        }
        for literal in ["-0e-400", "1e-400"] {
            let source = request
                .to_string()
                .replace(&fragment, &format!("\"{member}\":{literal}"));
            assert!(
                resolve_material_relief_paint_source(&source).is_ok(),
                "{member}: {literal}"
            );
        }
    }
}
