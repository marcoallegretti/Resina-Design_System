use resina_model::PhysicalVector;
use resina_raster::{RasterError, Viewport, render_relief_paint};
use resina_resolver::resolve_relief_paint_source;
use serde_json::{Value, json};

fn request(index: usize) -> Value {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/ir/relief-paint-vectors.json"
    ))
    .unwrap();
    cases[index]["request"].clone()
}

fn decode(channel: f64) -> f64 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}
fn encode(channel: f64) -> u8 {
    let value = if channel <= 0.0031308 {
        12.92 * channel
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    };
    (value * 255.0).round() as u8
}

#[test]
fn flat_face_matches_independent_box_coverage_and_linear_light() {
    let mut request = request(0);
    request["outline"]["size"] = json!({"width":8,"height":6});
    request["outline"]["minimumInset"] = json!(0);
    let paint = resolve_relief_paint_source(&request.to_string())
        .unwrap()
        .paint;
    for scale in [0.5, 1.0, 1.25, 1.5, 2.0] {
        for samples in [1, 2, 3, 4, 8] {
            let viewport = Viewport {
                origin: PhysicalVector { x: -1.25, y: -0.75 },
                width: (11.0_f64 * scale).ceil() as u32,
                height: (9.0_f64 * scale).ceil() as u32,
                pixels_per_unit: scale,
            };
            let image = render_relief_paint(&paint, viewport, samples).unwrap();
            for y in 0..viewport.height {
                for x in 0..viewport.width {
                    let (mut coverage, mut body) = (0_u32, 0_u32);
                    for sy in 0..samples {
                        for sx in 0..samples {
                            let px = viewport.origin.x
                                + (f64::from(x) + (f64::from(sx) + 0.5) / f64::from(samples))
                                    / scale;
                            let py = viewport.origin.y
                                + (f64::from(y) + (f64::from(sy) + 0.5) / f64::from(samples))
                                    / scale;
                            if (0.0..=8.0).contains(&px) && (0.0..=6.0).contains(&py) {
                                coverage += 1;
                                body += u32::from(
                                    (1.0..=7.0).contains(&px) && (1.0..=5.0).contains(&py),
                                );
                            }
                        }
                    }
                    let expected = if coverage == 0 {
                        [0; 4]
                    } else {
                        let channels = [0.5, 0.4, 0.3].map(|v| {
                            let mut sum = 0.0;
                            for _ in 0..body {
                                sum += decode(v);
                            }
                            encode(sum / f64::from(coverage))
                        });
                        [
                            channels[0],
                            channels[1],
                            channels[2],
                            (255.0 * f64::from(coverage) / f64::from(u32::from(samples).pow(2)))
                                .round() as u8,
                        ]
                    };
                    let offset = ((y * viewport.width + x) * 4) as usize;
                    assert_eq!(
                        &image.rgba()[offset..offset + 4],
                        &expected,
                        "scale={scale} samples={samples} ({x},{y})"
                    );
                }
            }
        }
    }
}

#[test]
fn raised_and_recessed_single_pixels_match_independent_signed_normal() {
    for index in [1, 2] {
        let paint = resolve_relief_paint_source(&request(index).to_string())
            .unwrap()
            .paint;
        let derivative = if index == 1 {
            0.234375_f64
        } else {
            -0.234375_f64
        };
        let q = derivative / (1.0 + derivative * derivative).sqrt() / 2.0_f64.sqrt();
        let profile = paint.pigment().profile();
        let body = [0.5, 0.4, 0.3];
        let endpoint = if q > 0.0 {
            body.map(|v| profile.highlight_lift() + v * (1.0 - profile.highlight_lift()))
        } else {
            body.map(|v| v * (1.0 - profile.side_shade()))
        };
        let expected = std::array::from_fn::<_, 3, _>(|i| {
            ((body[i] * (1.0 - q.abs()) + endpoint[i] * q.abs()) * 255.0).round() as u8
        });
        let image = render_relief_paint(
            &paint,
            Viewport {
                origin: PhysicalVector { x: 59.5, y: 1.5 },
                width: 1,
                height: 1,
                pixels_per_unit: 1.0,
            },
            1,
        )
        .unwrap();
        assert_eq!(image.rgba(), &[expected[0], expected[1], expected[2], 255]);
    }
}

#[test]
fn invalid_raster_requests_fail_with_existing_resource_guards() {
    let paint = resolve_relief_paint_source(&request(1).to_string())
        .unwrap()
        .paint;
    let viewport = Viewport {
        origin: PhysicalVector { x: 0.0, y: 0.0 },
        width: 1,
        height: 1,
        pixels_per_unit: 1.0,
    };
    assert!(matches!(
        render_relief_paint(&paint, viewport, 0),
        Err(RasterError::InvalidSampling)
    ));
    let enormous = Viewport {
        width: 8192,
        height: 8192,
        ..viewport
    };
    assert!(matches!(
        render_relief_paint(&paint, enormous, 1),
        Err(RasterError::ResourceLimit)
    ));
}
