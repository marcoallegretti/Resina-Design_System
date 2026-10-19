#![cfg(feature = "testing")]

use guido::prelude::*;
use resina_guido::prepare_lit_control_paint;
use resina_resolver::resolve_command_paint_source;
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc};

#[test]
fn command_endpoints_use_portable_paint_on_the_first_native_frame() {
    let bundle: Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::env::var("RESINA_SCENES").expect("RESINA_SCENES is required"),
        )
        .unwrap(),
    )
    .unwrap();
    let template: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-paint-request.json"
    ))
    .unwrap();
    let initial_command = resolve_command_paint_source(&template.to_string()).unwrap();
    let initial = prepare_lit_control_paint(initial_command.paint(), 1.0, 4).unwrap();
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
        for phase in ["rest", "hover", "pressed", "disabled"] {
            surface_request["body"]["surface"]["states"]["states"] = if focused {
                json!([phase, "focused"])
            } else {
                json!([phase])
            };
            let request = json!({"schemaVersion":"0.2.0", "surface":surface_request, "commandAppearance":profile,
                "light":template["light"], "relief":template["relief"]});
            let command = resolve_command_paint_source(&request.to_string()).unwrap();
            let ratio = command.paint().body().face().content_contrast_ratio();
            if phase == "disabled" {
                let bound = if surface_request["body"]["theme"]["environment"]["accessibilityPreferences"]
                    ["highContrast"]
                    == json!(true)
                {
                    resina_resolver::HIGH_CONTRAST_DISABLED_LABEL_CONTRAST
                } else {
                    resina_resolver::DISABLED_LABEL_CONTRAST
                };
                assert!(
                    (bound..bound + 1e-9).contains(&ratio),
                    "disabled label contrast {ratio} is not dimmed to {bound}:1"
                );
            } else {
                assert!(ratio >= 4.5);
            }
            for scale in [1.0, 1.25, 2.0, 3.0] {
                app.configure(surface, 204, 96, scale);
                let paint = prepare_lit_control_paint(command.paint(), scale, 4).unwrap();
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
    assert_eq!(checked, 192);
    hold.release();
}

#[test]
fn admitted_command_light_reaches_the_native_frame() {
    let mut request: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-paint-request.json"
    ))
    .unwrap();
    let environment = &mut request["surface"]["body"]["theme"]["environment"];
    environment["accessibilityPreferences"]["highContrast"] = json!(false);
    environment["accessibilityPreferences"]["reducedMotion"] = json!(false);
    environment["rendererCapabilities"]["dynamicLighting"] = json!(true);
    environment["qualityPolicy"] = json!("balanced");
    request["surface"]["body"]["size"] = json!({"width": 64, "height": 40});
    request["surface"]["body"]["surface"]["states"]["states"] = json!(["hover"]);
    let lit = resolve_command_paint_source(&request.to_string()).unwrap();
    let light = &serde_json::to_value(&lit).unwrap()["paint"]["body"]["face"]["light"];
    assert!(light["ambient"].as_f64().unwrap() > 0.0, "{light}");
    assert!(
        light["interaction"]["strength"].as_f64().unwrap() > 0.0,
        "{light}"
    );
    request["light"] = json!({"ambient": null, "interaction": null});
    let unlit = resolve_command_paint_source(&request.to_string()).unwrap();
    let scale = 2.0;
    let paint = prepare_lit_control_paint(lit.paint(), scale, 4).unwrap();
    let reference = prepare_lit_control_paint(unlit.paint(), scale, 4).unwrap();
    assert_eq!(paint.origin(), reference.origin());
    let (
        ImageSource::Rgba {
            width,
            pixels: lit_pixels,
            ..
        },
        ImageSource::Rgba {
            pixels: unlit_pixels,
            ..
        },
    ) = (paint.image_source(), reference.image_source())
    else {
        unreachable!()
    };
    let (index, difference) = lit_pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(unlit_pixels.as_chunks::<4>().0)
        .map(|(lit, unlit)| {
            if lit[3] == 255 && unlit[3] == 255 {
                (0..3).map(|c| lit[c].abs_diff(unlit[c])).max().unwrap()
            } else {
                0
            }
        })
        .enumerate()
        .max_by_key(|(_, difference)| *difference)
        .unwrap();
    assert!(
        difference > 2,
        "admitted light leaves the prepared body unchanged"
    );
    let mut app = guido::testing::Headless::new().expect("GUIdo GPU rendering is required");
    let origin = paint.origin();
    let size = paint.logical_size();
    let source = paint.image_source();
    let surface = app.surface(
        SurfaceConfig::new()
            .height(96)
            .background_color(Color::rgba(0.0, 0.0, 0.0, 0.0)),
        move || {
            container().width(204.0).height(96.0).child(
                container()
                    .width(size.width)
                    .height(size.height)
                    .translate((12.0 + origin.0, 12.0 + origin.1))
                    .child(image(source.clone()).content_fit(ContentFit::Fill)),
            )
        },
    );
    app.configure(surface, 204, 96, scale);
    app.step();
    app.wait_for_image_decodes();
    app.step();
    let actual = app.read_pixel(
        surface,
        ((12.0 + origin.0) * scale).round() as u32 + index as u32 % width,
        ((12.0 + origin.1) * scale).round() as u32 + index as u32 / width,
    );
    for channel in 0..4 {
        assert!(
            actual[channel].abs_diff(lit_pixels[index * 4 + channel]) <= 1,
            "{actual:?} {:?}",
            &lit_pixels[index * 4..index * 4 + 4]
        );
    }
    assert!((0..3).any(|c| actual[c].abs_diff(unlit_pixels[index * 4 + c]) > 1));
}
