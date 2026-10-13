use resina_model::PhysicalVector;
use resina_raster::{RasterError, Viewport, render_outline_focus};
use resina_resolver::{OutlineFocusIr, resolve_outline_focus_source};
use serde_json::{Value, json};

fn request() -> Value {
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
    json!({"schemaVersion":"0.1.0", "focus":{
        "schemaVersion":"0.1.0", "scenario":{
            "schemaVersion":"0.4.0", "resolution":serde_json::from_str::<Value>(include_str!(
                "../../../../conformance/headless/valid-request.json"
            )).unwrap(), "surface":surface
        }, "surroundingColor":{"colorSpace":"srgb","components":[1,1,1],"alpha":1}
    }, "outline":outlines[0]["request"], "sideOffset":{"x":6,"y":-4}, "directions":[]})
}

fn resolve(source: &Value) -> OutlineFocusIr {
    resolve_outline_focus_source(&source.to_string())
        .unwrap()
        .focus()
        .clone()
}

fn section(x: f64, low: f64, high: f64, radius: f64, width: f64, height: f64) -> f64 {
    let horizontal = (-x).max(x - width).max(0.0);
    if horizontal >= radius {
        return 0.0;
    }
    let extension = (radius * radius - horizontal * horizontal).sqrt();
    (high.min(height + extension) - low.max(-extension)).max(0.0)
}

fn area_bounds(left: f64, top: f64, step: f64, radius: f64, core: (f64, f64)) -> (f64, f64) {
    let right = left + step;
    let mut cuts = vec![left, right];
    cuts.extend(
        [0.0, core.0]
            .into_iter()
            .filter(|x| *x > left && *x < right),
    );
    cuts.sort_by(f64::total_cmp);
    let mut low = 0.0;
    let mut high = 0.0;
    for interval in cuts.windows(2) {
        let width = (interval[1] - interval[0]) / 2048.0;
        for index in 0..2048 {
            let start = interval[0] + f64::from(index) * width;
            let end = interval[0] + f64::from(index + 1) * width;
            let a = section(start, top, top + step, radius, core.0, core.1);
            let b = section(end, top, top + step, radius, core.0, core.1);
            low += width * a.min(b);
            high += width * a.max(b);
        }
    }
    (low / step.powi(2), high / step.powi(2))
}

#[test]
fn rectangle_ring_coverage_agrees_with_independent_monotone_area_bounds() {
    let mut source = request();
    source["outline"]["size"] = json!({"width":20,"height":14});
    source["outline"]["radii"] = json!({
        "topStart":{"x":0,"y":0}, "topEnd":{"x":0,"y":0},
        "bottomEnd":{"x":0,"y":0}, "bottomStart":{"x":0,"y":0}
    });
    source["sideOffset"] = json!({"x":0,"y":0});
    let ir = resolve(&source);
    for scale in [1.0_f64, 1.25, 1.5, 2.0, 3.0] {
        let viewport = Viewport {
            origin: PhysicalVector {
                x: -6.25,
                y: -5.125,
            },
            width: (34.0 * scale).ceil() as u32,
            height: (26.0 * scale).ceil() as u32,
            pixels_per_unit: scale,
        };
        let image = render_outline_focus(&ir, viewport).unwrap();
        let step = 1.0 / scale;
        for y in 0..viewport.height {
            for x in 0..viewport.width {
                let left = viewport.origin.x + f64::from(x) * step;
                let top = viewport.origin.y + f64::from(y) * step;
                let inner = area_bounds(left, top, step, 2.0, (20.0, 14.0));
                let outer = area_bounds(left, top, step, 4.0, (20.0, 14.0));
                let low = (outer.0 - inner.1).max(0.0);
                let high = (outer.1 - inner.0).min(1.0);
                let alpha =
                    f64::from(image.rgba()[((y * viewport.width + x) * 4 + 3) as usize]) / 255.0;
                assert!(
                    alpha >= low - 1.0 / 255.0 - 1e-10 && alpha <= high + 1.0 / 255.0 + 1e-10,
                    "scale {scale}, pixel {x},{y}: alpha {alpha} outside {low}..{high}"
                );
            }
        }
    }
}

#[test]
fn circular_ring_fractional_coverage_agrees_with_independent_disk_area_bounds() {
    let mut source = request();
    source["outline"]["curvature"] = json!("circular");
    source["sideOffset"] = json!({"x":0,"y":0});
    for corner in ["topStart", "topEnd", "bottomEnd", "bottomStart"] {
        source["outline"]["radii"][corner] = json!({"x":24,"y":24});
    }
    let ir = resolve(&source);
    for scale in [1.25_f64, 3.0] {
        let viewport = Viewport {
            origin: PhysicalVector {
                x: 112.125,
                y: 51.25,
            },
            width: 18,
            height: 18,
            pixels_per_unit: scale,
        };
        let image = render_outline_focus(&ir, viewport).unwrap();
        let step = 1.0 / scale;
        let mut fractional = 0;
        for y in 0..viewport.height {
            for x in 0..viewport.width {
                let left = viewport.origin.x + f64::from(x) * step - 24.0;
                let top = viewport.origin.y + f64::from(y) * step - 24.0;
                let inner = area_bounds(left, top, step, 26.0, (72.0, 12.0));
                let outer = area_bounds(left, top, step, 28.0, (72.0, 12.0));
                let low = (outer.0 - inner.1).max(0.0);
                let high = (outer.1 - inner.0).min(1.0);
                let pixel = &image.rgba()[((y * viewport.width + x) * 4) as usize..][..4];
                let alpha = f64::from(pixel[3]) / 255.0;
                assert!(
                    alpha >= low - 1.0 / 255.0 - 1e-10 && alpha <= high + 1.0 / 255.0 + 1e-10,
                    "scale {scale}, pixel {x},{y}: {alpha} outside {low}..{high}"
                );
                fractional += usize::from(pixel[3] > 0 && pixel[3] < 255);
                if pixel[3] == 0 {
                    assert_eq!(pixel, &[0, 0, 0, 0]);
                } else {
                    assert_eq!(&pixel[..3], &[38, 51, 77]);
                }
            }
        }
        assert!(fractional > 0, "fixture must exercise partial coverage");
    }
}

#[test]
fn dark_adjacency_retains_the_resolved_bright_srgb_focus_assignment() {
    let mut source = request();
    source["sideOffset"] = json!({"x":0,"y":0});
    for corner in ["topStart", "topEnd", "bottomEnd", "bottomStart"] {
        source["outline"]["radii"][corner] = json!({"x":24,"y":24});
    }
    source["focus"]["surroundingColor"] =
        json!({"colorSpace":"srgb","components":[0.01,0.015,0.02],"alpha":1});
    source["focus"]["scenario"]["resolution"]["opaqueColorAssignments"]["roles"]["focus"] =
        json!("palette.opaqueAlt");
    let ir = resolve(&source);
    let normal = 1.0 / 2.0_f64.sqrt();
    let image = render_outline_focus(
        &ir,
        Viewport {
            origin: PhysicalVector {
                x: 116.18151396608916 + 2.2 * normal - 0.0625,
                y: 56.18151396608915 + 2.2 * normal - 0.0625,
            },
            width: 1,
            height: 1,
            pixels_per_unit: 8.0,
        },
    )
    .unwrap();
    assert_eq!(image.rgba(), &[204, 179, 153, 255]);
}

#[test]
fn quartic_diagonal_preserves_disk_offsets_instead_of_inflating_axes() {
    let mut source = request();
    source["sideOffset"] = json!({"x":0,"y":0});
    for corner in ["topStart", "topEnd", "bottomEnd", "bottomStart"] {
        source["outline"]["radii"][corner] = json!({"x":24,"y":24});
    }
    let ir = resolve(&source);
    let normal = 1.0 / 2.0_f64.sqrt();
    for (distance, expected) in [(2.2, 255), (4.3, 0)] {
        let viewport = Viewport {
            origin: PhysicalVector {
                x: 116.18151396608916 + distance * normal - 0.0625,
                y: 56.18151396608915 + distance * normal - 0.0625,
            },
            width: 1,
            height: 1,
            pixels_per_unit: 8.0,
        };
        assert_eq!(
            render_outline_focus(&ir, viewport).unwrap().rgba()[3],
            expected
        );
    }
}

#[test]
fn asymmetric_rtl_signed_sweep_retains_pixel_phase_when_cropped() {
    let mut source = request();
    source["sideOffset"] = json!({"x":-6,"y":4});
    source["outline"]["layoutDirection"] = json!("rtl");
    source["outline"]["radii"] = json!({
        "topStart":{"x":20,"y":12}, "topEnd":{"x":30,"y":15},
        "bottomEnd":{"x":24,"y":18}, "bottomStart":{"x":12,"y":20}
    });
    let ir = resolve(&source);
    let full = render_outline_focus(
        &ir,
        Viewport {
            origin: PhysicalVector { x: -10.0, y: -6.0 },
            width: 180,
            height: 100,
            pixels_per_unit: 1.25,
        },
    )
    .unwrap();
    let cropped = render_outline_focus(
        &ir,
        Viewport {
            origin: PhysicalVector { x: 110.0, y: 58.0 },
            width: 24,
            height: 20,
            pixels_per_unit: 1.25,
        },
    )
    .unwrap();
    let mut fractional = 0;
    for y in 0..20 {
        for x in 0..24 {
            let expected = &full.rgba()[(((y + 80) * 180 + x + 150) * 4) as usize..][..4];
            let actual = &cropped.rgba()[((y * 24 + x) * 4) as usize..][..4];
            assert_eq!(actual, expected, "cropped device pixel {x},{y}");
            fractional += usize::from(actual[3] > 0 && actual[3] < 255);
        }
    }
    assert!(fractional > 0, "crop must cross the focus boundary");
}

#[test]
fn outline_focus_rejects_invalid_or_unrepresentable_viewports_before_rendering() {
    let ir = resolve(&request());
    let mut viewport = Viewport {
        origin: PhysicalVector { x: 0.0, y: 0.0 },
        width: u32::MAX,
        height: u32::MAX,
        pixels_per_unit: 1.0,
    };
    assert!(matches!(
        render_outline_focus(&ir, viewport),
        Err(RasterError::ResourceLimit)
    ));
    viewport.width = 1;
    viewport.height = 1;
    viewport.origin.x = 1e20;
    assert!(matches!(
        render_outline_focus(&ir, viewport),
        Err(RasterError::InvalidViewport(_))
    ));
}
