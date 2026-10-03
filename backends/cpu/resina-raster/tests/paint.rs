use resina_model::PhysicalVector;
use resina_raster::{RasterError, Viewport, render_surface, render_surface_paint};
use resina_resolver::resolve_surface_paint_source;
use serde_json::{Value, json};

fn request() -> Value {
    json!({"schemaVersion": "0.1.0", "body": serde_json::from_str::<Value>(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json")).unwrap(),
        "surroundingColor": {"colorSpace": "srgb", "components": [0, 0, 0], "alpha": 1}})
}

fn solid_black_rectangle() -> Value {
    let mut request = request();
    request["body"]["surface"]["states"]["states"] = json!(["focused"]);
    request["body"]["surface"]["form"]["elevation"] = json!("base");
    request["body"]["appearance"]["bands"]["cast"]["highlightWidth"] = json!(0);
    let mut theme: Value =
        serde_json::from_str(request["body"]["theme"]["themeSource"].as_str().unwrap()).unwrap();
    theme["opaqueColorAssignments"]["roles"]["surface.base"] = json!("palette.black");
    theme["opaqueColorAssignments"]["roles"]["content.primary"] = json!("palette.base");
    request["body"]["theme"]["themeSource"] = json!(theme.to_string());
    request
}

fn viewport(scale: f64) -> Viewport {
    Viewport {
        origin: PhysicalVector { x: -8.0, y: -8.0 },
        width: (40.0 * scale).ceil() as u32,
        height: (32.0 * scale).ceil() as u32,
        pixels_per_unit: scale,
    }
}

#[test]
fn body_and_ring_share_coverage_and_linear_light_including_mixed_pixels() {
    let paint = resolve_surface_paint_source(&solid_black_rectangle().to_string()).unwrap();
    let mut mixed_pixels = 0;
    for scale in [0.125, 0.25, 0.5, 1.0, 1.25, 2.0, 3.0] {
        for samples in 1..=8 {
            let viewport = viewport(scale);
            let image = render_surface_paint(&paint, viewport, samples).unwrap();
            for y in 0..image.height() {
                for x in 0..image.width() {
                    let mut body_count = 0;
                    let mut ring_count = 0;
                    for sy in 0..samples {
                        for sx in 0..samples {
                            let px = -8.0
                                + (f64::from(x) + (f64::from(sx) + 0.5) / f64::from(samples))
                                    / scale;
                            let py = -8.0
                                + (f64::from(y) + (f64::from(sy) + 0.5) / f64::from(samples))
                                    / scale;
                            let body = (0.0..=20.0).contains(&px) && (0.0..=12.0).contains(&py);
                            let distance = (-px)
                                .max(px - 20.0)
                                .max(0.0)
                                .hypot((-py).max(py - 12.0).max(0.0));
                            let ring = distance > 2.0 && distance <= 4.0;
                            assert!(!(body && ring));
                            body_count += u32::from(body);
                            ring_count += u32::from(ring);
                        }
                    }
                    mixed_pixels += u32::from(body_count > 0 && ring_count > 0);
                    let count = body_count + ring_count;
                    let expected = if count == 0 {
                        [0; 4]
                    } else {
                        let linear = f64::from(ring_count) / f64::from(count);
                        let srgb = if linear <= 0.0031308 {
                            linear * 12.92
                        } else {
                            1.055 * linear.powf(1.0 / 2.4) - 0.055
                        };
                        let channel = (srgb * 255.0).round() as u8;
                        [
                            channel,
                            channel,
                            channel,
                            (f64::from(count) * 255.0 / f64::from(u32::from(samples).pow(2)))
                                .round() as u8,
                        ]
                    };
                    let offset = ((y * image.width() + x) * 4) as usize;
                    assert_eq!(
                        &image.rgba()[offset..offset + 4],
                        &expected,
                        "scale={scale} samples={samples} ({x},{y})"
                    );
                }
            }
        }
    }
    assert!(mixed_pixels > 0);
}

#[test]
fn unfocused_paint_is_byte_identical_and_focused_limits_remain_bounded() {
    let paint = resolve_surface_paint_source(&request().to_string()).unwrap();
    for samples in 1..=8 {
        let viewport = viewport(1.25);
        assert_eq!(
            render_surface_paint(&paint, viewport, samples)
                .unwrap()
                .rgba(),
            render_surface(paint.body(), viewport, samples)
                .unwrap()
                .rgba()
        );
    }
    let paint = resolve_surface_paint_source(&solid_black_rectangle().to_string()).unwrap();
    assert!(matches!(
        render_surface_paint(&paint, viewport(1.0), 0),
        Err(RasterError::InvalidSampling)
    ));
    let mut view = viewport(1.0);
    view.width = u32::MAX;
    assert!(matches!(
        render_surface_paint(&paint, view, 4),
        Err(RasterError::ResourceLimit)
    ));
    view = viewport(1.0);
    view.origin.x = f64::NAN;
    assert!(matches!(
        render_surface_paint(&paint, view, 4),
        Err(RasterError::InvalidViewport(_))
    ));
}
