use super::*;
use resina_resolver::resolve_surface_paint_source;

fn focused() -> SurfacePaintIr {
    let mut body: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json"
    ))
    .unwrap();
    body["surface"]["states"]["states"] = serde_json::json!(["focused"]);
    resolve_surface_paint_source(
        &serde_json::json!({"schemaVersion":"0.1.0","body":body,
            "surroundingColor":{"colorSpace":"srgb","components":[0,0,0],"alpha":1}})
        .to_string(),
    )
    .unwrap()
}

#[test]
fn worker_pixels_preserve_the_complete_rgba_frame() {
    fn transferable<T: Send + Sync>() {}
    transferable::<PreparedPaint>();
    let prepared = std::thread::spawn(|| {
        prepare_surface_paint(&focused(), PhysicalVector { x: 0.0, y: 0.0 }, 1.25, 4)
    })
    .join()
    .unwrap()
    .unwrap();
    assert_eq!(prepared.origin, [-4.0, -4.0]);
    let expected = resina_raster::render_surface_paint(
        &focused(),
        resina_raster::Viewport {
            origin: PhysicalVector { x: -4.0, y: -4.0 },
            width: prepared.pixels.width(),
            height: prepared.pixels.height(),
            pixels_per_unit: 1.25,
        },
        4,
    )
    .unwrap();
    for (native, straight) in prepared
        .pixels
        .as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(expected.rgba().as_chunks::<4>().0)
    {
        assert_eq!(native[3], straight[3]);
        for channel in 0..3 {
            assert_eq!(
                u32::from(native[channel]),
                (u32::from(straight[channel]) * u32::from(straight[3]) + 127) / 255
            );
        }
    }
    let snapshot = prepared.into_snapshot();
    assert_eq!(snapshot.image.size().width, expected.width());
    assert_eq!(snapshot.image.size().height, expected.height());
    assert_eq!(snapshot.device_scale, 1.25);
}

#[test]
fn invalid_native_geometry_and_resource_requests_fail_explicitly() {
    let ir = focused();
    let origin = PhysicalVector { x: 0.0, y: 0.0 };
    for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(matches!(
            prepare_surface_paint(&ir, origin, scale, 4),
            Err(PrepareError::InvalidScale)
        ));
    }
    assert!(matches!(
        prepare_surface_paint(
            &ir,
            PhysicalVector {
                x: f64::NAN,
                y: 0.0
            },
            1.0,
            4
        ),
        Err(PrepareError::UnrepresentableBounds)
    ));
    for samples in [0, 9] {
        let error = prepare_surface_paint(&ir, origin, 1.0, samples).unwrap_err();
        assert!(matches!(
            error,
            PrepareError::Raster(RasterError::InvalidSampling)
        ));
        assert!(error.source().is_some());
    }
    assert!(matches!(
        prepare_surface_paint(&ir, origin, 1000.0, 8),
        Err(PrepareError::Raster(RasterError::ResourceLimit))
    ));
    let bounds = ir
        .focus()
        .unwrap()
        .geometry()
        .outer()
        .contour()
        .bounds()
        .unwrap();
    assert!(matches!(
        prepare_surface_paint(
            &ir,
            PhysicalVector {
                x: 40001.125 / 1.25 - bounds.x,
                y: 0.0
            },
            1.25,
            4,
        ),
        Err(PrepareError::CoordinatePrecision(_))
    ));
}
