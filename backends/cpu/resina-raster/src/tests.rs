use super::*;
use resina_resolver::{
    resolve_command_paint_source, resolve_focus_ir_source, resolve_material_relief_paint_source,
    resolve_opaque_surface_source, resolve_relief_paint_source, resolve_surface_paint_source,
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
        |point| surface.sample_paint(point).map_err(RasterError::Paint),
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
                    |point| surface.sample_paint(point).map_err(RasterError::Paint),
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
                    |point| focus.sample_paint(point).map_err(RasterError::Paint),
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
                    |point| match paint
                        .body()
                        .sample_paint(point)
                        .map_err(RasterError::Paint)?
                    {
                        Some(color) => Ok(Some(color)),
                        None => paint
                            .focus()
                            .unwrap()
                            .sample_paint(point)
                            .map_err(RasterError::Paint),
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

struct ForcedWorkers;

impl Drop for ForcedWorkers {
    fn drop(&mut self) {
        FORCED_WORKERS.with(|forced| forced.set(None));
    }
}

fn with_workers<T>(workers: usize, render: impl FnOnce() -> T) -> T {
    FORCED_WORKERS.with(|forced| forced.set(Some(workers)));
    let _reset = ForcedWorkers;
    render()
}

fn assert_workers_reproduce_single_worker(
    name: &str,
    bounds: [f64; 4],
    render: impl Fn(Viewport, u8) -> Result<RasterImage, RasterError>,
) {
    let [x, y, width, height] = bounds;
    for (scale, samples) in [
        (1.0, 1),
        (1.0, 4),
        (1.25, 2),
        (1.25, 3),
        (2.0, 4),
        (0.75, 8),
    ] {
        let viewport = Viewport {
            origin: PhysicalVector { x, y },
            width: (width * scale).ceil() as u32,
            height: (height * scale).ceil() as u32,
            pixels_per_unit: scale,
        };
        let expected = with_workers(1, || render(viewport, samples)).unwrap();
        for workers in [2, 3, 8] {
            let actual = with_workers(workers, || render(viewport, samples)).unwrap();
            assert!(
                actual.rgba() == expected.rgba(),
                "{name} at scale {scale} with {samples} samples differs with {workers} workers"
            );
        }
    }
}

#[test]
fn row_workers_reproduce_single_worker_rasters() {
    let mut command: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-paint-request.json"
    ))
    .unwrap();
    for states in [json!(["rest"]), json!(["pressed", "focused"])] {
        command["surface"]["body"]["surface"]["states"]["states"] = states.clone();
        let command = resolve_command_paint_source(&command.to_string()).unwrap();
        assert_workers_reproduce_single_worker(
            &format!("command {states}"),
            [-6.0, -6.0, 132.0, 60.0],
            |viewport, samples| render_surface_paint(command.paint(), viewport, samples),
        );
    }
    let surface = resolve_opaque_surface_source(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json"
    ))
    .unwrap();
    assert_workers_reproduce_single_worker(
        "opaque surface",
        [-4.0, -4.0, 28.0, 20.0],
        |viewport, samples| render_surface(&surface, viewport, samples),
    );
    let focus = resolve_focus_ir_source(include_str!(
        "../../../../conformance/ir/focus-ir-request.json"
    ))
    .unwrap();
    assert_workers_reproduce_single_worker(
        "focus indicator",
        [-6.125, -6.375, 34.0, 28.0],
        |viewport, samples| render_focus(&focus, viewport, samples),
    );
    let relief: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/ir/relief-paint-vectors.json"
    ))
    .unwrap();
    let material: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/ir/material-relief-paint-vectors.json"
    ))
    .unwrap();
    let request = |cases: &[Value], name: &str| {
        let case = cases.iter().find(|case| case["name"] == name).unwrap();
        let size = &case["request"]["outline"]["size"];
        let bounds = [
            -2.0,
            -2.0,
            (size["width"].as_f64().unwrap() + 4.0).min(36.0),
            (size["height"].as_f64().unwrap() + 4.0).min(28.0),
        ];
        (case["request"].to_string(), bounds)
    };
    for name in ["cast-convex-opaque-face", "circular-closed-edge-18"] {
        let (source, bounds) = request(&relief, name);
        let paint = resolve_relief_paint_source(&source).unwrap().paint;
        assert_workers_reproduce_single_worker(name, bounds, |viewport, samples| {
            render_relief_paint(&paint, viewport, samples)
        });
    }
    for name in [
        "gel-recessed-opaque-face-active-light",
        "frost-convex-opaque-face-ambient-only",
        "static-quartic-closed-edge-120",
    ] {
        let (source, bounds) = request(&material, name);
        let paint = resolve_material_relief_paint_source(&source).unwrap().paint;
        assert_workers_reproduce_single_worker(name, bounds, |viewport, samples| {
            render_material_relief_paint(&paint, viewport, samples)
        });
    }
}

#[test]
fn row_workers_report_the_first_row_major_failure() {
    const FAILURES: [&str; 3] = ["row 3 column 7", "row 3 column 9", "row 40 column 1"];
    let color = resina_color::resolve_srgb_fallback(&json!({
        "colorSpace": "srgb", "components": [0.2, 0.4, 0.6], "alpha": 1
    }))
    .unwrap();
    let viewport = Viewport {
        origin: PhysicalVector { x: 0.0, y: 0.0 },
        width: 12,
        height: 48,
        pixels_per_unit: 1.0,
    };
    for workers in 1..=8 {
        let error = with_workers(workers, || {
            render(
                viewport,
                1,
                &color,
                |_| UniformRegion::Sample,
                |point| {
                    let failure = match (point.y as u32, point.x as u32) {
                        (3, 7) => FAILURES[0],
                        (3, 9) => FAILURES[1],
                        (40, 1) => FAILURES[2],
                        (3, _) => {
                            // Delays the earliest failing row behind later rows.
                            std::thread::sleep(std::time::Duration::from_millis(5));
                            return Ok(Some(color.clone()));
                        }
                        _ => return Ok(Some(color.clone())),
                    };
                    Err(RasterError::FocusPrecision(failure))
                },
            )
        })
        .unwrap_err();
        assert!(
            matches!(error, RasterError::FocusPrecision(failure) if failure == FAILURES[0]),
            "{workers} workers reported {error}"
        );
    }
}

#[test]
fn forced_workers_reset_when_a_render_panics() {
    let panicked = std::panic::catch_unwind(|| with_workers(3, || panic!("render failed")));
    assert!(panicked.is_err());
    assert_eq!(FORCED_WORKERS.with(std::cell::Cell::get), None);
}

#[test]
fn alternating_sample_colors_match_uncached_decoding_across_workers() {
    let colors: Vec<_> = [[0.9, 0.1, 0.2], [0.1, 0.8, 0.3], [0.2, 0.3, 0.95]]
        .into_iter()
        .map(|components| {
            resina_color::resolve_srgb_fallback(&json!({
                "colorSpace": "srgb", "components": components, "alpha": 1
            }))
            .unwrap()
        })
        .collect();
    let viewport = Viewport {
        origin: PhysicalVector { x: 0.0, y: 0.0 },
        width: 9,
        height: 7,
        pixels_per_unit: 1.0,
    };
    for samples in [2, 3] {
        let grid = f64::from(samples);
        let color_at = |point: PhysicalVector| {
            let cell = (point.x * grid) as usize + 2 * (point.y * grid) as usize;
            colors.get(cell % 4).cloned()
        };
        let mut expected = Vec::new();
        for y in 0..viewport.height {
            for x in 0..viewport.width {
                let mut sum = [0.0; 3];
                let mut covered = 0_u32;
                for sy in 0..samples {
                    for sx in 0..samples {
                        let point = PhysicalVector {
                            x: f64::from(x) + (f64::from(sx) + 0.5) / grid,
                            y: f64::from(y) + (f64::from(sy) + 0.5) / grid,
                        };
                        if let Some(color) = color_at(point) {
                            covered += 1;
                            let linear = srgb_to_linear_srgb(color.components()).unwrap();
                            for (sum, channel) in sum.iter_mut().zip(linear) {
                                *sum += channel;
                            }
                        }
                    }
                }
                if covered == 0 {
                    expected.extend([0; 4]);
                } else {
                    let rgb = linear_srgb_to_srgb(sum.map(|sum| sum / f64::from(covered))).unwrap();
                    expected.extend(rgb.map(quantize));
                    expected.push(quantize(f64::from(covered) / (grid * grid)));
                }
            }
        }
        for workers in [1, 8] {
            let image = with_workers(workers, || {
                render(
                    viewport,
                    samples,
                    &colors[0],
                    |_| UniformRegion::Sample,
                    |point| Ok(color_at(point)),
                )
            })
            .unwrap();
            assert!(
                image.rgba() == expected,
                "{samples} samples with {workers} workers differ from uncached decoding"
            );
        }
    }
}
