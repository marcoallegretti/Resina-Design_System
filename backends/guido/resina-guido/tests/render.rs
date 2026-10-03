#![cfg(feature = "testing")]

use guido::prelude::*;
use resina_guido::{PreparedPaint, prepare_focus, prepare_surface};
use resina_resolver::{resolve_focus_ir_source, resolve_opaque_surface_source};
use serde_json::Value;
use std::{cell::Cell, rc::Rc};

#[test]
fn authored_paint_is_ready_on_first_frame_and_after_source_and_scale_changes() {
    let path = std::env::var("RESINA_SCENES")
        .expect("RESINA_SCENES must name the generated public material scene bundle");
    let bundle: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let scenes = bundle["scenarios"].as_array().unwrap();
    assert_eq!(scenes.len(), 16);
    let mut app = guido::testing::Headless::new().expect("GUIdo GPU rendering is required");
    let hold = app.hold_image_decodes();
    let slot = Rc::new(Cell::new(None));
    let captured = slot.clone();
    let first = prepare(&scenes[0], 1.0);
    let surface = app.surface(
        SurfaceConfig::new()
            .height(96)
            .background_color(Color::rgba(0.0, 0.0, 0.0, 0.0)),
        move || {
            let signal = create_signal(first.clone());
            captured.set(Some(signal));
            container().width(204.0).height(96.0).child(
                container()
                    .width(move || signal.get().logical_size().width)
                    .height(move || signal.get().logical_size().height)
                    .translate(move || {
                        let origin = signal.get().origin();
                        (12.0 + origin.0, 12.0 + origin.1)
                    })
                    .child(
                        image(move || signal.get().image_source()).content_fit(ContentFit::Fill),
                    ),
            )
        },
    );
    for scale in [1.0, 1.25, 2.0, 3.0] {
        app.configure(surface, 204, 96, scale);
        for scene in scenes {
            let paint = prepare(scene, scale);
            let ImageSource::Rgba {
                width,
                height,
                pixels,
            } = paint.image_source()
            else {
                unreachable!()
            };
            let origin = paint.origin();
            let offset_x = ((12.0 + origin.0) * scale).round() as u32;
            let offset_y = ((12.0 + origin.1) * scale).round() as u32;
            slot.get().unwrap().update_always(|value| *value = paint);
            app.step();
            let mut indices = Vec::new();
            for alpha in [255, 128, 0] {
                indices.push(
                    pixels
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| match alpha {
                            128 => p[3] > 0 && p[3] < 255,
                            _ => p[3] == alpha,
                        })
                        .map(|(index, _)| index)
                        .next()
                        .expect("scene must cover opaque, partial and clear pixels"),
                );
            }
            indices.push((height / 2 * width + width / 2) as usize);
            for index in indices {
                let pixel = &pixels[index * 4..index * 4 + 4];
                let expected = [
                    ((u16::from(pixel[0]) * u16::from(pixel[3]) + 127) / 255) as u8,
                    ((u16::from(pixel[1]) * u16::from(pixel[3]) + 127) / 255) as u8,
                    ((u16::from(pixel[2]) * u16::from(pixel[3]) + 127) / 255) as u8,
                    pixel[3],
                ];
                let actual = app.read_pixel(
                    surface,
                    offset_x + index as u32 % width,
                    offset_y + index as u32 / width,
                );
                for channel in 0..4 {
                    assert!(
                        actual[channel].abs_diff(expected[channel]) <= 1,
                        "{} scale={scale} pixel={index}: {actual:?} != {expected:?}",
                        scene["name"]
                    );
                }
            }
            assert_eq!(app.read_pixel(surface, 0, 0), [0; 4]);
        }
    }
    hold.release();
}

fn prepare(scene: &Value, scale: f32) -> PreparedPaint {
    let request = scene["request"].to_string();
    match scene["kind"].as_str().unwrap() {
        "opaqueSurface" => {
            prepare_surface(&resolve_opaque_surface_source(&request).unwrap(), scale, 4).unwrap()
        }
        "focusRing" => {
            prepare_focus(&resolve_focus_ir_source(&request).unwrap(), scale, 4).unwrap()
        }
        kind => panic!("unsupported authored scene kind {kind}"),
    }
}
