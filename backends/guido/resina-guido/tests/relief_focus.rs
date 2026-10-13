#![cfg(feature = "testing")]

#[path = "common/label.rs"]
mod label_style;
#[path = "common/readback.rs"]
mod readback;

use guido::{
    prelude::ImageSource,
    renderer::{
        DrawCommand, FlattenScratch, GpuContext, RenderNode, RenderTarget, Renderer,
        flatten_root_into,
    },
    widgets::{Color, ContentFit, Rect, font::FontFamily},
};
use resina_environment::{LayoutDirection, SafeArea};
use resina_guido::{measure_command_label, prepare_outline_focus_at, prepare_relief_content_at};
use resina_model::PhysicalVector;
use resina_resolver::{
    CommandLabelInput, resolve_command_label, resolve_outline_focus_source,
    resolve_relief_paint_source,
};
use serde_json::{Value, json};
use std::rc::Rc;

fn frame(
    renderer: &mut Renderer,
    target: &mut RenderTarget,
    root: &RenderNode,
    canvas: Color,
) -> Vec<u8> {
    let mut commands = Vec::new();
    let mut layers = Vec::new();
    flatten_root_into(
        root,
        &mut commands,
        &mut layers,
        &mut FlattenScratch::default(),
    );
    assert!(renderer.render(target, &commands, &layers, canvas));
    readback::read_frame(target)
}

#[test]
fn composed_relief_retains_the_complete_label_and_opaque_focus_ring() {
    guido::load_font(std::fs::read(std::env::var("RESINA_LABEL_FONT").unwrap()).unwrap());
    let family = FontFamily::name("DejaVu Sans");
    let vectors: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/relief-paint-vectors.json"
    ))
    .unwrap();
    let bindings: Value = serde_json::from_str(include_str!(
        "../../../../conformance/surfaces/binding-vectors.json"
    ))
    .unwrap();
    let mut resolution: Value = serde_json::from_str(include_str!(
        "../../../../conformance/headless/valid-request.json"
    ))
    .unwrap();
    let token = resolution["opaqueColorAssignments"]["roles"]["focus"]
        .as_str()
        .unwrap()
        .split('.')
        .nth(1)
        .unwrap()
        .to_owned();
    let mut binding = bindings[1]["document"].clone();
    binding["treatmentStack"]["treatments"] = json!(["none"]);
    let gpu = GpuContext::try_new().expect("focus composition GPU evidence is required");
    let mut target = RenderTarget::offscreen(&gpu, 360, 200);
    let mut renderer = Renderer::new(gpu.device.clone(), gpu.queue.clone(), target.format());
    let origin = PhysicalVector { x: 12.25, y: 7.125 };
    let typography = label_style::style(2.0, 0.25, 400.0, 1.4);
    for curve in ["circular", "quartic"] {
        for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
            for dark in [false, true] {
                let mut request = vectors[7]["request"].clone();
                request["outline"]["size"] = json!({"width":320,"height":160});
                request["outline"]["curvature"] = json!(curve);
                request["outline"]["layoutDirection"] = json!(direction);
                request["outline"]["radii"] = json!({
                    "topStart":{"x":20,"y":12}, "topEnd":{"x":30,"y":15},
                    "bottomEnd":{"x":24,"y":18}, "bottomStart":{"x":12,"y":20}
                });
                request["profile"]["amplitude"] = json!(0.25);
                request["profile"]["shoulderWidth"] = json!(1);
                request["edgeWidth"] = json!(0.25);
                request["points"] = json!([]);
                let canvas_components = if dark { [0, 0, 0] } else { [1, 1, 1] };
                let ring_components = if dark { [1, 1, 1] } else { [0, 0, 0] };
                request["edge"]["adjacentColor"]["components"] = json!(canvas_components);
                request["edge"]["outline"]["components"] = json!(ring_components);
                request["edge"]["outlineStrong"]["components"] = json!(ring_components);
                resolution["tokens"]["palette"][&token]["$value"] =
                    json!({"colorSpace":"srgb","components":ring_components,"alpha":1});
                let focus_request = json!({"schemaVersion":"0.1.0", "focus":{
                    "schemaVersion":"0.1.0", "scenario":{"schemaVersion":"0.4.0","resolution":resolution,"surface":binding},
                    "surroundingColor":request["edge"]["adjacentColor"]},
                    "outline":request["outline"],"sideOffset":{"x":0,"y":0},"directions":[]});
                let paint = resolve_relief_paint_source(&request.to_string())
                    .unwrap()
                    .paint;
                let focus = resolve_outline_focus_source(&focus_request.to_string()).unwrap();
                assert_eq!(
                    focus.focus().geometry().outline(),
                    paint.surface().outline()
                );
                assert_eq!(
                    focus.focus().geometry().side_offset(),
                    PhysicalVector { x: 0.0, y: 0.0 }
                );
                assert_eq!(
                    serde_json::to_value(focus.focus().indicator()).unwrap()["binding"]["states"]["states"],
                    bindings[1]["expected"]["states"]["states"]
                );
                let label = resolve_command_label(
                    CommandLabelInput {
                        text: "j Reconnect now",
                        typography: &typography,
                        minimum_size: paint.surface().outline().size(),
                        maximum_size: paint.surface().outline().size(),
                        padding: SafeArea {
                            start: 32.0,
                            end: 40.0,
                            top: 0.0,
                            bottom: 0.0,
                        },
                        direction,
                    },
                    |input| measure_command_label(family, input),
                )
                .unwrap();
                let canvas = if dark { Color::BLACK } else { Color::WHITE };
                for scale in [1.25_f32, 3.0] {
                    let width = (360.0 * scale).round() as u32;
                    let height = (200.0 * scale).round() as u32;
                    target.resize(width, height);
                    renderer.set_screen_size(width as f32, height as f32);
                    renderer.set_scale_factor(scale);
                    let mut root = RenderNode::new(1);
                    root.commands.extend(
                        prepare_relief_content_at(&paint, &label, family, origin, scale, 4)
                            .unwrap()
                            .into_iter()
                            .map(Rc::new),
                    );
                    let baseline = frame(&mut renderer, &mut target, &root, canvas);
                    let prepared = prepare_outline_focus_at(focus.focus(), origin, scale).unwrap();
                    let ImageSource::Rgba {
                        width: ring_width,
                        height: ring_height,
                        pixels,
                    } = prepared.image_source()
                    else {
                        panic!("prepared focus pixels required")
                    };
                    let (x, y) = prepared.origin();
                    let size = prepared.logical_size();
                    root.commands.insert(
                        0,
                        Rc::new(DrawCommand::Image {
                            source: prepared.image_source(),
                            decoded: None,
                            rect: Rect::new(x, y, size.width, size.height),
                            content_fit: ContentFit::Fill,
                            tint: None,
                        }),
                    );
                    let composed = frame(&mut renderer, &mut target, &root, canvas);
                    let offset_x = (x * scale).round() as u32;
                    let offset_y = (y * scale).round() as u32;
                    let mut solid = 0;
                    let mut unaffected = 0;
                    for ry in 0..ring_height {
                        for rx in 0..ring_width {
                            let ring_index = ((ry * ring_width + rx) * 4) as usize;
                            let frame_index =
                                (((offset_y + ry) * width + offset_x + rx) * 4) as usize;
                            let alpha = pixels[ring_index + 3];
                            if alpha == 255 {
                                solid += 1;
                                for channel in 0..4 {
                                    assert!(
                                        composed[frame_index + channel]
                                            .abs_diff(pixels[ring_index + channel])
                                            <= 1,
                                        "opaque focus erased at {rx},{ry}"
                                    );
                                }
                                assert_ne!(
                                    &baseline[frame_index..frame_index + 3],
                                    &pixels[ring_index..ring_index + 3]
                                );
                            } else if alpha == 0 {
                                unaffected += 1;
                                for channel in 0..4 {
                                    assert!(
                                        composed[frame_index + channel]
                                            .abs_diff(baseline[frame_index + channel])
                                            <= 1,
                                        "content changed at {rx},{ry}"
                                    );
                                }
                            }
                        }
                    }
                    assert!(solid > 0 && unaffected > 0);
                    println!(
                        "{curve}/{direction:?}, dark {dark}, scale {scale}: {solid} retained solid focus pixels, {unaffected} unchanged content pixels"
                    );
                    if let Ok(directory) = std::env::var("RESINA_RELIEF_FOCUS_CAPTURE_DIR") {
                        std::fs::create_dir_all(&directory).unwrap();
                        readback::capture(
                            &std::path::Path::new(&directory).join(format!(
                                "{curve}-{direction:?}-dark-{dark}-scale-{scale}.ppm"
                            )),
                            width,
                            height,
                            &composed,
                        );
                    }
                }
            }
        }
    }
}
