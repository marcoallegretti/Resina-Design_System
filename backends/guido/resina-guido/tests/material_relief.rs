#![cfg(feature = "testing")]

#[path = "common/label.rs"]
mod label_style;

use guido::{prelude::*, renderer::DrawCommand};
use resina_environment::{LayoutDirection, SafeArea};
use resina_guido::{
    ReliefContentPrepareError, measure_command_label, prepare_material_relief_content_at,
    prepare_material_relief_paint_at,
};
use resina_model::PhysicalVector;
use resina_resolver::{
    CommandContentError, CommandLabelInput, resolve_command_label,
    resolve_material_relief_paint_source,
};
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc};

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../../conformance/ir/material-relief-paint-vectors.json"
    ))
    .unwrap()
}

#[test]
fn authored_light_changes_are_ready_on_the_first_gpu_frame_at_fractional_scales() {
    let mut app = guido::testing::Headless::new().expect("GUIdo GPU rendering is required");
    let hold = app.hold_image_decodes();
    let slot = Rc::new(Cell::new(None));
    let captured = slot.clone();
    let initial = resolve_material_relief_paint_source(&cases()[0]["request"].to_string()).unwrap();
    let origin = PhysicalVector { x: 12.25, y: 7.125 };
    let first = prepare_material_relief_paint_at(&initial.paint, origin, 1.0, 1).unwrap();
    let surface = app.surface(
        SurfaceConfig::new()
            .width(160)
            .height(160)
            .background_color(Color::rgba(0.0, 0.0, 0.0, 0.0)),
        move || {
            let signal = create_signal(first.clone());
            captured.set(Some(signal));
            container().width(160.0).height(160.0).child(
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
    for scale in [1.0, 1.25, 1.5, 2.0, 3.0] {
        app.configure(surface, 160, 160, scale);
        for case in cases()
            .into_iter()
            .filter(|case| case.get("expected").is_some())
        {
            let paint = resolve_material_relief_paint_source(&case["request"].to_string())
                .unwrap()
                .paint;
            let prepared = prepare_material_relief_paint_at(&paint, origin, scale, 1).unwrap();
            let ImageSource::Rgba {
                width,
                height,
                pixels,
            } = prepared.image_source()
            else {
                unreachable!()
            };
            let placed = prepared.origin();
            let offset_x = (placed.0 * scale).round() as u32;
            let offset_y = (placed.1 * scale).round() as u32;
            slot.get().unwrap().update_always(|value| *value = prepared);
            app.step();
            let frame = app.read_frame(surface);
            let frame_width = (160.0 * scale).round() as u32;
            for index in 0..(width * height) as usize {
                let pixel = &pixels[index * 4..index * 4 + 4];
                let frame_index = ((offset_y + index as u32 / width) * frame_width
                    + offset_x
                    + index as u32 % width) as usize
                    * 4;
                let actual = &frame[frame_index..frame_index + 4];
                for channel in 0..4 {
                    let expected = if channel == 3 {
                        pixel[channel]
                    } else {
                        ((u16::from(pixel[channel]) * u16::from(pixel[3]) + 127) / 255) as u8
                    };
                    assert!(
                        actual[channel].abs_diff(expected) <= 1,
                        "{} scale={scale} pixel={index}: {actual:?} != {pixel:?}",
                        case["name"]
                    );
                }
            }
            assert_eq!(app.read_pixel(surface, 0, 0), [0; 4]);
        }
    }
    hold.release();
}

#[test]
fn material_light_never_relaxes_complete_label_containment() {
    guido::load_font(std::fs::read(std::env::var("RESINA_LABEL_FONT").unwrap()).unwrap());
    let family = guido::widgets::font::FontFamily::name("DejaVu Sans");
    for name in [
        "cast-convex-opaque-face-active-light",
        "cast-recessed-opaque-face-active-light",
    ] {
        let mut request = cases()
            .into_iter()
            .find(|case| case["name"] == name)
            .unwrap()["request"]
            .clone();
        request["outline"]["size"] = json!({"width":320,"height":160});
        let paint = resolve_material_relief_paint_source(&request.to_string())
            .unwrap()
            .paint;
        for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
            for text_scale in [1.0, 2.0] {
                let typography = label_style::style(text_scale, 0.25, 400.0, 1.4);
                for safe in [false, true] {
                    let size = paint.surface().outline().size();
                    let label = resolve_command_label(
                        CommandLabelInput {
                            text: "j Reconnect now",
                            typography: &typography,
                            minimum_size: size,
                            maximum_size: size,
                            padding: SafeArea {
                                start: if safe { 32.0 } else { 0.0 },
                                end: 40.0,
                                top: 0.0,
                                bottom: 0.0,
                            },
                            direction,
                        },
                        |input| measure_command_label(family, input),
                    )
                    .unwrap();
                    for scale in [1.0, 1.25, 1.5, 2.0, 3.0] {
                        let origin = PhysicalVector { x: 12.25, y: 7.125 };
                        let result = prepare_material_relief_content_at(
                            &paint, &label, family, origin, scale, 1,
                        );
                        if !safe {
                            assert!(matches!(
                                result,
                                Err(ReliefContentPrepareError::Content(
                                    CommandContentError::OutsideContent
                                ))
                            ));
                            continue;
                        }
                        let [material, text] = result.unwrap();
                        assert!(matches!(material, DrawCommand::Image { .. }));
                        let DrawCommand::Text {
                            text, rect, color, ..
                        } = text
                        else {
                            panic!("complete native text required")
                        };
                        assert_eq!(text.as_ref(), "j Reconnect now");
                        assert!(
                            (f64::from(rect.x) - origin.x - label.label_bounds().x).abs()
                                <= 1.0 / 1024.0
                        );
                        assert!(
                            (f64::from(rect.y) - origin.y - label.label_bounds().y).abs()
                                <= 1.0 / 1024.0
                        );
                        let [r, g, b] = paint.foreground().components().map(|value| value as f32);
                        assert_eq!(color, Color::rgba(r, g, b, 1.0));
                    }
                }
            }
        }
    }
}
