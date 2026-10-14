use resina_model::PhysicalVector;
use resina_raster::{Viewport, render_material_relief_paint, render_relief_paint};
use resina_resolver::{resolve_material_relief_paint_source, resolve_relief_paint_source};
use serde_json::Value;

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../../conformance/ir/material-relief-paint-vectors.json"
    ))
    .unwrap()
}

#[test]
fn absent_channels_preserve_static_rasters_at_fractional_scales() {
    for case in cases().into_iter().filter(|case| {
        case["name"].as_str().unwrap().starts_with("static-") && case.get("expected").is_some()
    }) {
        let mut old = case["request"].clone();
        let request = old.as_object_mut().unwrap();
        let light = request.remove("light").unwrap();
        request.insert("keyLight".into(), light["keyLight"].clone());
        for name in ["environment", "enabled", "interactionActive"] {
            request.remove(name);
        }
        let static_paint = resolve_relief_paint_source(&old.to_string()).unwrap().paint;
        let material_paint = resolve_material_relief_paint_source(&case["request"].to_string())
            .unwrap()
            .paint;
        for scale in [1.0, 1.25, 1.5, 2.0, 3.0] {
            let viewport = Viewport {
                origin: PhysicalVector { x: -1.0, y: -1.0 },
                width: 12,
                height: 8,
                pixels_per_unit: scale,
            };
            let original = render_relief_paint(&static_paint, viewport, 3).unwrap();
            let actual = render_material_relief_paint(&material_paint, viewport, 3).unwrap();
            assert_eq!(
                actual.rgba(),
                original.rgba(),
                "{} scale={scale}",
                case["name"]
            );
        }
    }
}

#[test]
fn active_light_pixels_match_independent_half_shoulder_arithmetic() {
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
            [0.54, 0.448, 0.356]
        } else {
            [0.45, 0.36, 0.27]
        };
        let body = [0.5, 0.4, 0.3];
        let expected = std::array::from_fn::<_, 3, _>(|i| {
            ((body[i] + q.abs() * (tip[i] - body[i])) * 255.0).round() as u8
        });
        for scale in [1.0, 1.25, 1.5, 2.0, 3.0] {
            let image = render_material_relief_paint(
                &paint,
                Viewport {
                    origin: PhysicalVector {
                        x: 60.0 - 0.5 / scale,
                        y: 2.0 - 0.5 / scale,
                    },
                    width: 1,
                    height: 1,
                    pixels_per_unit: scale,
                },
                1,
            )
            .unwrap();
            assert_eq!(
                image.rgba(),
                &[expected[0], expected[1], expected[2], 255],
                "{name} scale={scale}"
            );
        }
    }
}
