use super::*;
use resina_resolver::{
    resolve_focus_ir_source, resolve_opaque_surface_source, resolve_surface_paint_source,
};
use serde_json::{Value, json};

#[test]
fn boundary_and_extreme_boxes_require_point_sampling() {
    let surface = resolve_opaque_surface_source(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json"
    ))
    .unwrap();
    let content = surface.geometry().content();
    let bounds = content.contour().bounds().unwrap();
    let point = PhysicalVector {
        x: bounds.x + content.offset().x,
        y: bounds.y + content.offset().y + bounds.height / 2.0,
    };
    for x in [point.x.next_down(), point.x, point.x.next_up()] {
        let point = PhysicalVector { x, ..point };
        assert!(!uniform::placed_inside(
            SampleBox {
                min: point,
                max: point
            },
            content
        ));
    }
    let exterior = PhysicalVector {
        x: -1e120,
        y: -1e120,
    };
    let region = SampleBox {
        min: exterior,
        max: exterior,
    };
    assert!(!uniform::placed_inside(region, content));
    assert!(!uniform::disjoint(region, surface.geometry().silhouette()));
    let viewport = Viewport {
        origin: exterior,
        width: 1,
        height: 1,
        pixels_per_unit: 1e-118,
    };
    let expected = render(
        viewport,
        8,
        surface.pigment().body(),
        |_| UniformRegion::Sample,
        |point| surface.sample_paint(point),
    )
    .unwrap();
    assert_eq!(
        render_surface(&surface, viewport, 8).unwrap().rgba(),
        expected.rgba()
    );
}

#[test]
fn uniform_regions_match_point_sampling_across_geometry_and_sampling_grids() {
    for (shape, direction, light) in [
        ("structural", "ltr", [0.0, -1.0]),
        ("rounded", "rtl", [0.6, -0.8]),
        ("organic", "ltr", [-0.8, 0.6]),
        ("organic", "rtl", [0.8, 0.6]),
        ("capsule", "rtl", [-1.0, 0.0]),
        ("capsule", "ltr", [0.0, 1.0]),
    ] {
        let mut request: Value = serde_json::from_str(include_str!(
            "../../../../conformance/ir/focus-ir-request.json"
        ))
        .unwrap();
        request["surface"]["form"]["shape"] = json!(shape);
        request["theme"]["environment"]["layoutDirection"] = json!(direction);
        request["keyLight"]["direction"] = json!({"x": light[0], "y": light[1]});
        request["shapeAssignments"]["profiles"]["rounded"]["radius"] = json!("radius.1");
        request["shapeAssignments"]["profiles"]["organic"]["radii"] = json!({
            "topStart": "radius.1", "topEnd": "radius.0",
            "bottomEnd": "radius.2", "bottomStart": "radius.1"
        });
        let mut surface_request: Value = serde_json::from_str(include_str!(
            "../../../../conformance/ir/opaque-surface-request.json"
        ))
        .unwrap();
        surface_request["surface"] = request["surface"].clone();
        surface_request["surface"]["states"]["states"] = json!(["rest"]);
        surface_request["theme"] = request["theme"].clone();
        surface_request["appearance"]["shapeAssignments"] = request["shapeAssignments"].clone();
        surface_request["appearance"]["keyLight"] = request["keyLight"].clone();
        let surface = resolve_opaque_surface_source(&surface_request.to_string()).unwrap();
        let focus = resolve_focus_ir_source(&request.to_string()).unwrap();
        surface_request["surface"]["states"]["states"] = json!(["focused"]);
        let paint = resolve_surface_paint_source(
            &json!({
                "schemaVersion": "0.1.0", "body": surface_request,
                "surroundingColor": request["surroundingColor"]
            })
            .to_string(),
        )
        .unwrap();
        assert!(uniform::placed_supported(surface.geometry().content()));
        assert!(uniform::placed_supported(focus.geometry().inner()));
        for samples in 1..=8 {
            for scale in [0.75, 1.25] {
                let viewport = Viewport {
                    origin: PhysicalVector {
                        x: -6.125,
                        y: -6.375,
                    },
                    width: (34.0 * scale) as u32,
                    height: (28.0 * scale) as u32,
                    pixels_per_unit: scale,
                };
                let expected = render(
                    viewport,
                    samples,
                    surface.pigment().body(),
                    |_| UniformRegion::Sample,
                    |point| surface.sample_paint(point),
                )
                .unwrap();
                assert_eq!(
                    render_surface(&surface, viewport, samples).unwrap().rgba(),
                    expected.rgba(),
                    "surface {shape} {direction} {light:?} {scale} {samples}"
                );
                let expected = render(
                    viewport,
                    samples,
                    focus.indicator().color(),
                    |_| UniformRegion::Sample,
                    |point| focus.sample_paint(point),
                )
                .unwrap();
                assert_eq!(
                    render_focus(&focus, viewport, samples).unwrap().rgba(),
                    expected.rgba(),
                    "focus {shape} {direction} {light:?} {scale} {samples}"
                );
                let expected = render(
                    viewport,
                    samples,
                    paint.body().pigment().body(),
                    |_| UniformRegion::Sample,
                    |point| match paint.body().sample_paint(point)? {
                        Some(color) => Ok(Some(color)),
                        None => paint.focus().unwrap().sample_paint(point),
                    },
                )
                .unwrap();
                assert_eq!(
                    render_surface_paint(&paint, viewport, samples)
                        .unwrap()
                        .rgba(),
                    expected.rgba(),
                    "complete paint {shape} {direction} {light:?} {scale} {samples}"
                );
            }
        }
    }
}

#[test]
fn uniform_color_preserves_rounding_at_every_rgba8_threshold() {
    let viewport = Viewport {
        origin: PhysicalVector { x: 0.0, y: 0.0 },
        width: 1,
        height: 1,
        pixels_per_unit: 1.0,
    };
    for boundary in 0..255 {
        let value = (f64::from(boundary) + 0.5) / 255.0;
        for value in [value.next_down(), value, value.next_up()] {
            let color = resina_color::resolve_srgb_fallback(&json!({
                "colorSpace": "srgb", "components": [value, value, value], "alpha": 1
            }))
            .unwrap();
            for samples in 3..=8 {
                let expected = render(
                    viewport,
                    samples,
                    &color,
                    |_| UniformRegion::Sample,
                    |_| Ok(Some(color.clone())),
                )
                .unwrap();
                let actual = render(
                    viewport,
                    samples,
                    &color,
                    |_| UniformRegion::Solid,
                    |_| panic!("uniform pixel was sampled"),
                )
                .unwrap();
                assert_eq!(actual.rgba(), expected.rgba(), "{value} {samples}");
            }
        }
    }
}
