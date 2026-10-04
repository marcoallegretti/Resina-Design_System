#![cfg(feature = "testing")]

use guido::prelude::*;
use resina_guido::prepare_surface_paint;
use resina_resolver::resolve_command_motion_source;
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc};

#[test]
fn sampled_command_motion_uses_complete_paint_on_the_first_native_frame() {
    let bundle: Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::env::var("RESINA_SCENES").expect("RESINA_SCENES is required"),
        )
        .unwrap(),
    )
    .unwrap();
    let motion_template: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap();
    let initial_command = resolve_command_motion_source(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap();
    let initial = prepare_surface_paint(initial_command.command().paint(), 1.0, 4).unwrap();
    let mut app = guido::testing::Headless::new().expect("GUIdo GPU rendering is required");
    let slot = Rc::new(Cell::new(None));
    let captured = slot.clone();
    let surface = app.surface(
        SurfaceConfig::new()
            .height(96)
            .background_color(Color::rgba(0.0, 0.0, 0.0, 0.0)),
        move || {
            let signal = create_signal(initial.clone());
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
    app.step();
    let hold = app.hold_image_decodes();
    let mut checked = 0;
    for scene in bundle["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|scene| {
            scene["kind"] == "surfacePaint" && !scene["name"].as_str().unwrap().contains("gel")
        })
    {
        let family = scene["name"].as_str().unwrap().split('-').nth(1).unwrap();
        let dark = scene["name"].as_str().unwrap().starts_with("dark-");
        let profile: Value = serde_json::from_str(if dark {
            include_str!("../../../../definitions/command-appearance-dark.json")
        } else {
            include_str!("../../../../definitions/command-appearance-light.json")
        })
        .unwrap();
        let mut surface_request = scene["request"].clone();
        let mut theme: Value = serde_json::from_str(
            surface_request["body"]["theme"]["themeSource"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        theme["materialAssignments"]["control"]["interactive"] = json!(family);
        surface_request["body"]["theme"]["themeSource"] = json!(theme.to_string());
        surface_request["body"]["surface"]["materialRole"] = json!("control.interactive");
        let focused = surface_request["body"]["surface"]["states"]["states"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "focused");
        for (phase, time) in ["rest", "hover", "pressed", "disabled"]
            .into_iter()
            .flat_map(|phase| {
                [0.0, 0.25, 100.0]
                    .into_iter()
                    .map(move |time| (phase, time))
            })
        {
            surface_request["body"]["surface"]["states"]["states"] = if focused {
                json!([phase, "focused"])
            } else {
                json!([phase])
            };
            surface_request["body"]["theme"]["environment"]["accessibilityPreferences"]["reducedMotion"] =
                json!(false);
            let request = json!({"schemaVersion":"0.1.0", "surface":surface_request, "commandAppearance":profile, "channels":motion_template["channels"], "time":time});
            let command = resolve_command_motion_source(&request.to_string()).unwrap();
            assert!(command.command().paint().body().content_contrast_ratio() >= 4.5);
            for scale in [1.0, 1.25, 2.0, 3.0] {
                app.configure(surface, 204, 96, scale);
                let paint = prepare_surface_paint(command.command().paint(), scale, 4).unwrap();
                let ImageSource::Rgba {
                    width,
                    height,
                    pixels,
                } = paint.image_source()
                else {
                    unreachable!()
                };
                let origin = paint.origin();
                let ox = ((12.0 + origin.0) * scale).round() as u32;
                let oy = ((12.0 + origin.1) * scale).round() as u32;
                slot.get().unwrap().update_always(|p| *p = paint);
                app.step();
                for index in [
                    (height / 2 * width + width / 2) as usize,
                    pixels
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .position(|p| p[3] == 255)
                        .unwrap(),
                    pixels
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .position(|p| p[3] > 0 && p[3] < 255)
                        .unwrap(),
                ] {
                    let pixel = &pixels[index * 4..index * 4 + 4];
                    let actual = app.read_pixel(
                        surface,
                        ox + index as u32 % width,
                        oy + index as u32 / width,
                    );
                    for channel in 0..4 {
                        let expected = if channel == 3 {
                            pixel[3]
                        } else {
                            ((u16::from(pixel[channel]) * u16::from(pixel[3]) + 127) / 255) as u8
                        };
                        assert!(
                            actual[channel].abs_diff(expected) <= 1,
                            "{} {phase} scale={scale}",
                            scene["name"]
                        );
                    }
                }
                assert_eq!(app.read_pixel(surface, 0, 0), [0; 4]);
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 576);
    hold.release();
}
