#![cfg(feature = "testing")]

#[path = "common/readback.rs"]
mod readback;
use guido::{
    prelude::{ContentFit, ImageSource},
    renderer::{
        DrawCommand, FlattenScratch, GpuContext, RenderNode, RenderTarget, Renderer,
        flatten_root_into,
    },
    widgets::{Color, Rect},
};
use resina_guido::prepare_surface_paint;
use resina_resolver::{resolve_toggle_part_motion_source, resolve_toggle_part_paint_source};
use serde_json::{Value, json};
use std::rc::Rc;

#[test]
fn checked_part_paint_reaches_native_pixels_with_track_owned_focus() {
    let gpu = GpuContext::try_new().expect("GUIdo GPU rendering is required");
    let mut target = RenderTarget::offscreen(&gpu, 100, 64);
    let mut renderer = Renderer::new(gpu.device.clone(), gpu.queue.clone(), target.format());
    let captures = std::env::var_os("RESINA_TOGGLE_PART_CAPTURE_DIR").map(std::path::PathBuf::from);
    if let Some(path) = &captures {
        std::fs::create_dir_all(path).unwrap();
    }
    let mut count = 0;
    for family in ["cast", "frost", "elastomer"] {
        for part in ["track", "thumb"] {
            for checked in [false, true] {
                for focused in [false, true] {
                    let mut request: Value = serde_json::from_str(include_str!(
                        "../../../../conformance/ir/toggle-part-paint-request.json"
                    ))
                    .unwrap();
                    request["surface"]["body"]["appearance"]["shapeAssignments"] =
                        serde_json::from_str::<Value>(include_str!(
                            "../../../../definitions/tier0-shapes.json"
                        ))
                        .unwrap();
                    request["surface"]["body"]["appearance"]["pigmentProfiles"] =
                        serde_json::from_str::<Value>(include_str!(
                            "../../../../definitions/tier0-pigment.json"
                        ))
                        .unwrap();
                    request["part"] = json!(part);
                    let mut states = vec!["rest"];
                    if focused {
                        states.push("focused");
                    }
                    if checked {
                        states.push("checked");
                    }
                    request["surface"]["body"]["surface"]["states"]["states"] = json!(states);
                    request["surface"]["body"]["size"] = if part == "track" {
                        json!({"width":52,"height":28})
                    } else {
                        json!({"width":20,"height":20})
                    };
                    request["surface"]["body"]["surface"]["form"]["shape"] = json!("rounded");
                    let mut theme: Value = serde_json::from_str(
                        request["surface"]["body"]["theme"]["themeSource"]
                            .as_str()
                            .unwrap(),
                    )
                    .unwrap();
                    theme["materialAssignments"]["control"]["interactive"] = json!(family);
                    theme["opaqueColorAssignments"]["roles"]["outline.strong"] =
                        json!("palette.base");
                    request["surface"]["body"]["adjacentColor"] =
                        json!({"colorSpace":"srgb","components":[0,0,0],"alpha":1});
                    request["surface"]["body"]["theme"]["themeSource"] = json!(theme.to_string());
                    if family == "frost" {
                        request["surface"]["body"]["postTreatmentBackdrop"] =
                            json!({"colorSpace":"srgb","components":[1,1,1],"alpha":1});
                    }
                    let ir = resolve_toggle_part_paint_source(&request.to_string()).unwrap();
                    assert_eq!(ir.paint().focus().is_some(), part == "track" && focused);
                    for time in [None, Some(0.0), Some(0.25), Some(100.0)] {
                        let paint = if let Some(time) = time {
                            let mut moving = request.clone();
                            moving["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]
                                ["reducedMotion"] = json!(false);
                            moving["surface"]["body"]["surface"]["states"]["states"][0] =
                                json!("pressed");
                            let template: Value = serde_json::from_str(include_str!(
                                "../../../../conformance/ir/toggle-part-motion-request.json"
                            ))
                            .unwrap();
                            moving["channels"] = template["channels"].clone();
                            moving["time"] = json!(time);
                            let motion =
                                resolve_toggle_part_motion_source(&moving.to_string()).unwrap();
                            assert_eq!(
                                motion.part_paint().paint().focus().is_some(),
                                part == "track" && focused
                            );
                            assert_eq!(motion.part_paint().checked(), checked);
                            assert_eq!(
                                motion.policy(),
                                if family == "cast" {
                                    resina_resolver::CommandMotionPolicy::CastImmediate
                                } else {
                                    resina_resolver::CommandMotionPolicy::Spring
                                }
                            );
                            if family != "cast" && time == 0.25 {
                                assert_ne!(motion.part_paint().response(), motion.target());
                                assert_ne!(motion.part_paint().response().depth_scale(), 1.0);
                            }
                            motion.part_paint().paint().clone()
                        } else {
                            ir.paint().clone()
                        };
                        for scale in [1.0_f32, 1.25, 2.0] {
                            let prepared = prepare_surface_paint(&paint, scale, 4).unwrap();
                            let ImageSource::Rgba {
                                width: iw,
                                height: _,
                                pixels: expected,
                            } = prepared.image_source()
                            else {
                                unreachable!()
                            };
                            let origin = prepared.origin();
                            let size = prepared.logical_size();
                            let mut root = RenderNode::new(1);
                            root.commands.push(Rc::new(DrawCommand::Image {
                                source: prepared.image_source(),
                                decoded: None,
                                rect: Rect::new(
                                    16.0 + origin.0,
                                    16.0 + origin.1,
                                    size.width,
                                    size.height,
                                ),
                                content_fit: ContentFit::Fill,
                                tint: None,
                            }));
                            let mut commands = Vec::new();
                            let mut layers = Vec::new();
                            let mut scratch = FlattenScratch::default();
                            flatten_root_into(&root, &mut commands, &mut layers, &mut scratch);
                            let width = (100.0 * scale) as u32;
                            let height = (64.0 * scale) as u32;
                            target.resize(width, height);
                            renderer.set_screen_size(width as f32, height as f32);
                            renderer.set_scale_factor(scale);
                            assert!(renderer.render(&mut target, &commands, &layers, Color::BLACK));
                            let actual = readback::read_frame(&target);
                            let ox = ((16.0 + origin.0) * scale).round() as u32;
                            let oy = ((16.0 + origin.1) * scale).round() as u32;
                            let mut opaque = 0;
                            for (index, pixel) in expected
                                .as_chunks::<4>()
                                .0
                                .iter()
                                .enumerate()
                                .filter(|(_, p)| p[3] == 255)
                            {
                                let destination =
                                    (((oy + index as u32 / iw) * width + ox + index as u32 % iw)
                                        * 4) as usize;
                                for channel in 0..4 {
                                    assert!(
                                        actual[destination + channel].abs_diff(pixel[channel]) <= 1,
                                        "{family} {part} checked={checked} focused={focused} scale={scale}"
                                    );
                                }
                                opaque += 1;
                            }
                            assert!(opaque > 0);
                            if let Some(path) = &captures {
                                readback::capture(
                                    &path.join(format!(
                                        "{family}-{part}-{checked}-{focused}-{scale}-{time:?}.ppm"
                                    )),
                                    width,
                                    height,
                                    &actual,
                                );
                            }
                            count += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(count, 288);
}
