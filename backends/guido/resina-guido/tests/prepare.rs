use guido::prelude::ImageSource;
use resina_guido::{PrepareError, prepare_focus, prepare_surface, prepare_surface_paint};
use resina_model::PhysicalVector;
use resina_raster::{RasterError, Viewport, render_surface_paint};
use resina_resolver::{
    resolve_focus_ir_source, resolve_opaque_surface_source, resolve_surface_paint_source,
};
use serde_json::{Value, json};
use std::sync::Arc;

const FOCUS: &str = include_str!("../../../../conformance/ir/focus-ir-request.json");
const SURFACE: &str = include_str!("../../../../conformance/ir/opaque-surface-request.json");

#[test]
fn complete_paint_retains_body_ring_bounds_and_shared_pixels() {
    let mut body: Value = serde_json::from_str(SURFACE).unwrap();
    for states in [json!(["rest"]), json!(["focused"])] {
        body["surface"]["states"]["states"] = states;
        let ir = resolve_surface_paint_source(
            &json!({"schemaVersion": "0.1.0", "body": body,
            "surroundingColor": {"colorSpace": "srgb", "components": [0, 0, 0], "alpha": 1}})
            .to_string(),
        )
        .unwrap();
        for scale in [0.125, 0.5, 1.0, 1.25, 2.0, 3.0] {
            let paint = prepare_surface_paint(&ir, scale, 4).unwrap();
            let bounds_only = match ir.focus() {
                Some(focus) => prepare_focus(focus, scale, 4).unwrap(),
                None => prepare_surface(ir.body(), scale, 4).unwrap(),
            };
            assert_eq!(paint.origin(), bounds_only.origin());
            assert_eq!(paint.logical_size(), bounds_only.logical_size());
            let ImageSource::Rgba {
                width,
                height,
                pixels,
            } = paint.image_source()
            else {
                unreachable!()
            };
            let expected = render_surface_paint(
                &ir,
                Viewport {
                    origin: PhysicalVector {
                        x: f64::from(paint.origin().0),
                        y: f64::from(paint.origin().1),
                    },
                    width,
                    height,
                    pixels_per_unit: f64::from(scale),
                },
                4,
            )
            .unwrap();
            assert_eq!(&*pixels, expected.rgba());
            let ImageSource::Rgba { pixels: cloned, .. } = paint.clone().image_source() else {
                unreachable!()
            };
            assert!(Arc::ptr_eq(&pixels, &cloned));
        }
        assert!(matches!(
            prepare_surface_paint(&ir, 0.0, 4),
            Err(PrepareError::InvalidScale)
        ));
        assert!(matches!(
            prepare_surface_paint(&ir, 1.0, 0),
            Err(PrepareError::Raster(RasterError::InvalidSampling))
        ));
    }
}

#[test]
fn complete_focus_bounds_and_straight_pixels_survive_fractional_scaling() {
    let ir = resolve_focus_ir_source(FOCUS).unwrap();
    for scale in [0.5, 1.0, 1.25, 2.0, 3.0] {
        let paint = prepare_focus(&ir, scale, 4).unwrap();
        assert_eq!(paint.origin(), (-4.0, -4.0));
        assert_eq!(paint.logical_size().width, 28.0);
        assert_eq!(
            paint.logical_size().height,
            (22.0_f32 * scale).ceil() / scale
        );
        let ImageSource::Rgba {
            width,
            height,
            pixels,
        } = paint.image_source()
        else {
            panic!("prepared paint must contain raw pixels")
        };
        assert_eq!(width, (28.0_f32 * scale).ceil() as u32);
        assert_eq!(height, (22.0_f32 * scale).ceil() as u32);
        assert!(pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 255));
        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .any(|p| p[3] > 0 && p[3] < 255)
        );
        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| p[3] == 0 || p[..3] == [255; 3])
        );
        let center = ((height / 2 * width + width / 2) * 4) as usize;
        assert_eq!(&pixels[center..center + 4], &[0; 4]);
        let ImageSource::Rgba { pixels: cloned, .. } = paint.clone().image_source() else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(&pixels, &cloned));
    }
}

#[test]
fn surface_bounds_include_extrusion_and_reject_invalid_preparation() {
    let ir = resolve_opaque_surface_source(SURFACE).unwrap();
    let bounds = ir.geometry().silhouette().bounds().unwrap();
    let paint = prepare_surface(&ir, 1.25, 4).unwrap();
    let (x, y) = paint.origin();
    assert!(f64::from(x) <= bounds.x && f64::from(y) <= bounds.y);
    assert!(f64::from(x + paint.logical_size().width) >= bounds.x + bounds.width);
    assert!(f64::from(y + paint.logical_size().height) >= bounds.y + bounds.height);
    for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(matches!(
            prepare_surface(&ir, scale, 4),
            Err(PrepareError::InvalidScale)
        ));
    }
    assert!(matches!(
        prepare_surface(&ir, 1.0, 0),
        Err(PrepareError::Raster(RasterError::InvalidSampling))
    ));
    assert!(matches!(
        prepare_surface(&ir, 300.0, 4),
        Err(PrepareError::Raster(RasterError::ResourceLimit))
    ));
    assert!(matches!(
        prepare_surface(&ir, 1000.0, 4),
        Err(PrepareError::TextureLimit { width, .. }) if width > 8192
    ));
    assert!(matches!(
        prepare_surface(&ir, f32::MAX, 4),
        Err(PrepareError::UnrepresentableBounds)
    ));
}

#[test]
fn diagonal_focus_is_aligned_outward_and_large_coordinates_fail_explicitly() {
    let mut request: Value = serde_json::from_str(FOCUS).unwrap();
    request["keyLight"]["direction"] = json!({"x": 1, "y": -1});
    let ir = resolve_focus_ir_source(&request.to_string()).unwrap();
    let outer = ir.geometry().outer();
    let bounds = outer.contour().bounds().unwrap();
    let min_x = bounds.x + outer.offset().x;
    let min_y = bounds.y + outer.offset().y;
    let paint = prepare_focus(&ir, 1.25, 4).unwrap();
    assert_eq!(paint.origin().0, ((min_x * 1.25).floor() / 1.25) as f32);
    assert_eq!(paint.origin().1, ((min_y * 1.25).floor() / 1.25) as f32);
    request["size"]["width"] = json!(100_000_001.0);
    let huge = resolve_focus_ir_source(&request.to_string()).unwrap();
    assert!(matches!(
        prepare_focus(&huge, 1.0, 1),
        Err(PrepareError::CoordinatePrecision(_))
    ));
}
