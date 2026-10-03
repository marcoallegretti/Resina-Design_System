#![cfg(feature = "testing")]

use guido::prelude::*;
use resina_guido::{PreparedPaint, prepare_focus, prepare_surface, prepare_surface_paint};
use resina_resolver::{
    resolve_focus_ir_source, resolve_opaque_surface_source, resolve_surface_paint_source,
};
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc};

#[test]
fn authored_paint_is_ready_on_first_frame_and_after_source_and_scale_changes() {
    let path = std::env::var("RESINA_SCENES")
        .expect("RESINA_SCENES must name the generated public material scene bundle");
    let bundle: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut scenes = bundle["scenarios"].as_array().unwrap().clone();
    assert_eq!(scenes.len(), 16);
    for scene in bundle["scenarios"].as_array().unwrap() {
        if scene["kind"] != "opaqueSurface" {
            continue;
        }
        let focus_name = scene["name"].as_str().unwrap().replace("-rest", "-focus");
        let surrounding = bundle["scenarios"]
            .as_array()
            .unwrap()
            .iter()
            .find(|candidate| candidate["name"] == focus_name)
            .unwrap()["request"]["surroundingColor"]
            .clone();
        for state in ["focused", "rest"] {
            let mut body = scene["request"].clone();
            body["surface"]["states"]["states"] = json!([state]);
            scenes.push(json!({
                "name": format!("{}-complete-{state}", scene["name"].as_str().unwrap()),
                "kind": "surfacePaint",
                "request": {"schemaVersion": "0.1.0", "body": body, "surroundingColor": surrounding}
            }));
        }
    }
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
        let mut previous_ring_pixel = None;
        for scene in &scenes {
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
            if scene["kind"] == "surfacePaint" {
                let ir = resolve_surface_paint_source(&scene["request"].to_string()).unwrap();
                let center = (height / 2 * width + width / 2) as usize;
                assert_eq!(pixels[center * 4 + 3], 255);
                if let Some(focus) = ir.focus() {
                    let index = indices[0] as u32;
                    let point = resina_model::PhysicalVector {
                        x: f64::from(origin.0)
                            + (f64::from(index % width) + 0.5) / f64::from(scale),
                        y: f64::from(origin.1)
                            + (f64::from(index / width) + 0.5) / f64::from(scale),
                    };
                    assert!(ir.body().sample_paint(point).unwrap().is_none());
                    assert!(focus.sample_paint(point).unwrap().is_some());
                    previous_ring_pixel =
                        Some((offset_x + index % width, offset_y + index / width));
                    let body = ir.body().geometry().silhouette().bounds().unwrap();
                    let inner = focus.geometry().inner();
                    let bounds = inner.contour().bounds().unwrap();
                    let gap_y = (body.y + bounds.y + inner.offset().y) / 2.0;
                    let gap_x = body.x + body.width / 2.0;
                    let x = ((gap_x - f64::from(origin.0)) * f64::from(scale)).floor() as u32;
                    let y = ((gap_y - f64::from(origin.1)) * f64::from(scale)).floor() as u32;
                    let index = (y * width + x) as usize;
                    assert_eq!(&pixels[index * 4..index * 4 + 4], &[0; 4]);
                    indices.push(index);
                } else {
                    let (x, y) = previous_ring_pixel
                        .take()
                        .expect("rest follows focused paint");
                    assert_eq!(
                        app.read_pixel(surface, x, y),
                        [0; 4],
                        "previous ring must be removed"
                    );
                }
            }
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
        "surfacePaint" => {
            prepare_surface_paint(&resolve_surface_paint_source(&request).unwrap(), scale, 4)
                .unwrap()
        }
        kind => panic!("unsupported authored scene kind {kind}"),
    }
}
