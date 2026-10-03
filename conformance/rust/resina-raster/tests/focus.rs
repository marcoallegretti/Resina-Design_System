use resina_model::PhysicalVector;
use resina_raster::{RasterError, Viewport, render_focus};
use resina_resolver::resolve_focus_ir_source;

const REQUEST: &str = include_str!("../../../ir/focus-ir-request.json");

#[test]
fn ring_coverage_matches_distance_to_raised_rectangle() {
    let ir = resolve_focus_ir_source(REQUEST).unwrap();
    for scale in [0.5, 1.0, 2.0] {
        let viewport = Viewport {
            origin: PhysicalVector { x: -6.0, y: -6.0 },
            width: (32.0 * scale) as u32,
            height: (26.0 * scale) as u32,
            pixels_per_unit: scale,
        };
        let image = render_focus(&ir, viewport, 4).unwrap();
        for y in 0..image.height() {
            for x in 0..image.width() {
                let mut covered = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let px = -6.0 + (f64::from(x) + (f64::from(sx) + 0.5) / 4.0) / scale;
                        let py = -6.0 + (f64::from(y) + (f64::from(sy) + 0.5) / 4.0) / scale;
                        let dx = (-px).max(px - 20.0).max(0.0);
                        let dy = (-py).max(py - 14.0).max(0.0);
                        let distance = dx.hypot(dy);
                        covered += u32::from(distance > 2.0 && distance <= 4.0);
                    }
                }
                let offset = ((y * image.width() + x) * 4) as usize;
                let expected = if covered == 0 {
                    [0; 4]
                } else {
                    [
                        255,
                        255,
                        255,
                        (f64::from(covered) * 255.0 / 16.0).round() as u8,
                    ]
                };
                assert_eq!(
                    &image.rgba()[offset..offset + 4],
                    &expected,
                    "scale {scale} ({x},{y})"
                );
            }
        }
    }
}

#[test]
fn focus_raster_uses_the_same_resource_and_precision_limits() {
    let ir = resolve_focus_ir_source(REQUEST).unwrap();
    let mut viewport = Viewport {
        origin: PhysicalVector { x: 0.0, y: 0.0 },
        width: 1,
        height: 1,
        pixels_per_unit: 1.0,
    };
    assert!(matches!(
        render_focus(&ir, viewport, 0),
        Err(RasterError::InvalidSampling)
    ));
    viewport.width = u32::MAX;
    viewport.height = u32::MAX;
    assert!(matches!(
        render_focus(&ir, viewport, 8),
        Err(RasterError::ResourceLimit)
    ));
    viewport.width = 1;
    viewport.height = 1;
    viewport.origin.x = 1e20;
    assert!(matches!(
        render_focus(&ir, viewport, 4),
        Err(RasterError::InvalidViewport(_))
    ));
}
