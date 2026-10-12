#![cfg(feature = "testing")]

use guido::prelude::*;
use resina_guido::prepare_relief_paint_at;
use resina_model::PhysicalVector;
use resina_resolver::resolve_relief_paint_source;
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc};

#[test]
fn relief_is_ready_on_first_frame_and_after_curve_profile_and_scale_changes() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/ir/relief-paint-vectors.json"
    ))
    .unwrap();
    let mut app = guido::testing::Headless::new().expect("GUIdo GPU rendering is required");
    let hold = app.hold_image_decodes();
    let slot = Rc::new(Cell::new(None));
    let captured = slot.clone();
    let initial = resolve_relief_paint_source(&cases[1]["request"].to_string()).unwrap();
    let first = prepare_relief_paint_at(
        &initial.paint,
        PhysicalVector { x: 12.25, y: 12.25 },
        1.0,
        4,
    )
    .unwrap();
    let surface = app.surface(
        SurfaceConfig::new()
            .width(160)
            .height(96)
            .background_color(Color::rgba(0.0, 0.0, 0.0, 0.0)),
        move || {
            let signal = create_signal(first.clone());
            captured.set(Some(signal));
            container().width(160.0).height(96.0).child(
                container()
                    .width(move || signal.get().logical_size().width)
                    .height(move || signal.get().logical_size().height)
                    .translate(move || signal.get().origin())
                    .child(
                        image(move || signal.get().image_source()).content_fit(ContentFit::Fill),
                    ),
            )
        },
    );
    for scale in [1.0, 1.25, 1.5, 2.0] {
        app.configure(surface, 160, 96, scale);
        for curve in ["circular", "quartic"] {
            for index in [0, 1, 2, 9, 10, 11] {
                let mut request = cases[index]["request"].clone();
                request["outline"]["curvature"] = json!(curve);
                request["outline"]["radii"] = json!({
                    "topStart":{"x":20,"y":12},"topEnd":{"x":30,"y":15},
                    "bottomEnd":{"x":24,"y":18},"bottomStart":{"x":12,"y":20}});
                request["edgeWidth"] = json!(0.25);
                if request["profile"]["kind"] != "flat" {
                    request["profile"]["amplitude"] = json!(0.25);
                    request["profile"]["shoulderWidth"] = json!(1.0);
                }
                request["points"] = json!([]);
                let paint = resolve_relief_paint_source(&request.to_string())
                    .unwrap()
                    .paint;
                let prepared = prepare_relief_paint_at(
                    &paint,
                    PhysicalVector { x: 12.25, y: 12.25 },
                    scale,
                    4,
                )
                .unwrap();
                let ImageSource::Rgba {
                    width,
                    height,
                    pixels,
                } = prepared.image_source()
                else {
                    unreachable!()
                };
                let origin = prepared.origin();
                let offset_x = (origin.0 * scale).round() as u32;
                let offset_y = (origin.1 * scale).round() as u32;
                slot.get().unwrap().update_always(|value| *value = prepared);
                app.step();
                let frame = app.read_frame(surface);
                let frame_width = (160.0 * scale).round() as u32;
                for coverage in [0, 128, 255] {
                    assert!(
                        pixels
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .any(|pixel| match coverage {
                                128 => pixel[3] > 0 && pixel[3] < 255,
                                _ => pixel[3] == coverage,
                            }),
                        "curved scene must contain clear, partial and opaque coverage"
                    );
                }
                for index in 0..(width * height) as usize {
                    let pixel = &pixels[index * 4..index * 4 + 4];
                    let expected = [
                        ((u16::from(pixel[0]) * u16::from(pixel[3]) + 127) / 255) as u8,
                        ((u16::from(pixel[1]) * u16::from(pixel[3]) + 127) / 255) as u8,
                        ((u16::from(pixel[2]) * u16::from(pixel[3]) + 127) / 255) as u8,
                        pixel[3],
                    ];
                    let frame_index = ((offset_y + index as u32 / width) * frame_width
                        + offset_x
                        + index as u32 % width) as usize
                        * 4;
                    let actual = &frame[frame_index..frame_index + 4];
                    for channel in 0..4 {
                        assert!(
                            actual[channel].abs_diff(expected[channel]) <= 1,
                            "curve={curve} case={index} scale={scale}: {actual:?} != {expected:?}"
                        );
                    }
                }
                assert_eq!(app.read_pixel(surface, 0, 0), [0; 4]);
            }
        }
    }
    hold.release();
}
