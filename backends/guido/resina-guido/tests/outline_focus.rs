use guido::prelude::ImageSource;
use resina_guido::{PrepareError, prepare_outline_focus_at};
use resina_model::PhysicalVector;
use resina_resolver::resolve_outline_focus_source;
use serde_json::{Value, json};

fn request() -> Value {
    let bindings: Value = serde_json::from_str(include_str!(
        "../../../../conformance/surfaces/binding-vectors.json"
    ))
    .unwrap();
    let mut surface = bindings[0]["document"].clone();
    surface["treatmentStack"]["treatments"] = json!(["none"]);
    let outlines: Value = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/surface-outline-vectors.json"
    ))
    .unwrap();
    json!({"schemaVersion":"0.1.0", "focus":{
        "schemaVersion":"0.1.0", "scenario":{
            "schemaVersion":"0.4.0", "resolution":serde_json::from_str::<Value>(include_str!(
                "../../../../conformance/headless/valid-request.json"
            )).unwrap(), "surface":surface
        }, "surroundingColor":{"colorSpace":"srgb","components":[1,1,1],"alpha":1}
    }, "outline":outlines[0]["request"], "sideOffset":{"x":6,"y":-4}, "directions":[]})
}

#[test]
fn analytic_focus_reserves_complete_outer_bounds_before_device_grid_placement() {
    let request = request();
    let ir = resolve_outline_focus_source(&request.to_string()).unwrap();
    let bounds = ir.focus().geometry().outer_bounds();
    let origin = PhysicalVector { x: 12.25, y: 7.125 };
    for scale in [1.0_f32, 1.25, 1.5, 2.0, 3.0] {
        let prepared = prepare_outline_focus_at(ir.focus(), origin, scale).unwrap();
        let (x, y) = prepared.origin();
        let size = prepared.logical_size();
        assert!(f64::from(x) <= bounds.x + origin.x);
        assert!(f64::from(y) <= bounds.y + origin.y);
        assert!(f64::from(x + size.width) >= bounds.x + origin.x + bounds.width);
        assert!(f64::from(y + size.height) >= bounds.y + origin.y + bounds.height);
        let ImageSource::Rgba {
            width,
            height,
            pixels,
        } = prepared.image_source()
        else {
            panic!("focus must retain prepared RGBA pixels");
        };
        assert!(width > 0 && height > 0);
        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] > 0 && pixel[3] < 255)
        );
        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] == 255)
        );
    }
    for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(matches!(
            prepare_outline_focus_at(ir.focus(), origin, scale),
            Err(PrepareError::InvalidScale)
        ));
    }
}

#[cfg(feature = "testing")]
#[test]
fn analytic_focus_pixels_survive_first_frame_curve_sweep_and_scale_changes() {
    use guido::prelude::*;
    use std::{cell::Cell, rc::Rc};
    let mut app = guido::testing::Headless::new().expect("focus GPU rendering is required");
    let hold = app.hold_image_decodes();
    let slot = Rc::new(Cell::new(None));
    let captured = slot.clone();
    let origin = PhysicalVector { x: 12.25, y: 12.25 };
    let mut source = request();
    source["outline"]["radii"] = json!({
        "topStart":{"x":20,"y":12}, "topEnd":{"x":30,"y":15},
        "bottomEnd":{"x":24,"y":18}, "bottomStart":{"x":12,"y":20}
    });
    let ir = resolve_outline_focus_source(&source.to_string()).unwrap();
    let first = prepare_outline_focus_at(ir.focus(), origin, 1.0).unwrap();
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
    for scale in [1.0_f32, 1.25, 1.5, 2.0, 3.0] {
        app.configure(surface, 160, 96, scale);
        for curve in ["circular", "quartic"] {
            for direction in ["ltr", "rtl"] {
                for (dx, dy) in [(6, -4), (-6, 4)] {
                    source["outline"]["curvature"] = json!(curve);
                    source["outline"]["layoutDirection"] = json!(direction);
                    source["sideOffset"] = json!({"x":dx,"y":dy});
                    let ir = resolve_outline_focus_source(&source.to_string()).unwrap();
                    let prepared = prepare_outline_focus_at(ir.focus(), origin, scale).unwrap();
                    let ImageSource::Rgba {
                        width,
                        height,
                        pixels,
                    } = prepared.image_source()
                    else {
                        panic!("prepared pixels required")
                    };
                    let (x, y) = prepared.origin();
                    let offset_x = (x * scale).round() as u32;
                    let offset_y = (y * scale).round() as u32;
                    slot.get().unwrap().update_always(|paint| *paint = prepared);
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
                                })
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
                            + index as u32 % width)
                            as usize
                            * 4;
                        for channel in 0..4 {
                            assert!(
                                frame[frame_index + channel].abs_diff(expected[channel]) <= 1,
                                "{curve}/{direction}, sweep {dx},{dy}, scale {scale}, pixel {index}"
                            );
                        }
                    }
                    assert_eq!(app.read_pixel(surface, 0, 0), [0; 4]);
                    println!(
                        "{curve}/{direction}, sweep {dx},{dy}, scale {scale}: {} prepared focus pixels",
                        width * height
                    );
                }
            }
        }
    }
    hold.release();
}
