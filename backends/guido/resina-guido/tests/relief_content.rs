#![cfg(feature = "testing")]

#[path = "common/conformance_font.rs"]
mod conformance_font;
#[path = "common/label.rs"]
mod label_style;
#[path = "common/readback.rs"]
mod readback;

use guido::{
    renderer::{
        DrawCommand, FlattenScratch, GpuContext, LineFit, RenderNode, RenderTarget, Renderer,
        flatten_root_into,
    },
    widgets::Color,
};
use resina_environment::{LayoutDirection, SafeArea};
use resina_guido::{ReliefContentPrepareError, measure_command_label, prepare_relief_content_at};
use resina_model::PhysicalVector;
use resina_resolver::{
    CommandContentError, CommandLabelInput, LabelMeasureInput, resolve_command_label,
    resolve_relief_paint_source,
};
use serde_json::{Value, json};
use std::rc::Rc;

#[test]
fn placed_relief_content_preserves_complete_native_text_and_rejects_edge_overlap() {
    let family = conformance_font::load();
    let vectors: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/ir/relief-paint-vectors.json"
    ))
    .unwrap();
    let mut request = vectors[0]["request"].clone();
    request["outline"]["size"] = json!({"width": 240, "height": 100});
    let paint = resolve_relief_paint_source(&request.to_string())
        .unwrap()
        .paint;
    let size = paint.surface().outline().size();
    let typography = label_style::style(1.0, 0.0, 400.0, 1.4);
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        for safe in [false, true] {
            let label = resolve_command_label(
                CommandLabelInput {
                    text: "Reconnect",
                    typography: &typography,
                    minimum_size: size,
                    maximum_size: size,
                    padding: SafeArea {
                        start: if safe { 16.0 } else { 0.0 },
                        end: 24.0,
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
                let result = prepare_relief_content_at(&paint, &label, family, origin, scale, 4);
                if !safe {
                    assert!(matches!(
                        result,
                        Err(ReliefContentPrepareError::Content(
                            CommandContentError::OutsideContent
                        ))
                    ));
                    continue;
                }
                let commands = result.unwrap();
                assert!(matches!(commands[0], DrawCommand::Image { .. }));
                let DrawCommand::Text {
                    text, rect, color, ..
                } = &commands[1]
                else {
                    panic!("complete native text required")
                };
                assert_eq!(text.as_ref(), "Reconnect");
                assert!(
                    (f64::from(rect.x) - origin.x - label.label_bounds().x).abs() <= 1.0 / 1024.0
                );
                assert!(
                    (f64::from(rect.y) - origin.y - label.label_bounds().y).abs() <= 1.0 / 1024.0
                );
                let [r, g, b] = paint.foreground().components().map(|value| value as f32);
                assert_eq!(*color, guido::widgets::Color::rgba(r, g, b, 1.0));
            }
        }
    }
}

#[test]
fn actual_glyph_ink_remains_on_the_opaque_plateau_at_fractional_device_and_text_scales() {
    let family = conformance_font::load();
    let vectors: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/ir/relief-paint-vectors.json"
    ))
    .unwrap();
    let gpu = GpuContext::try_new().expect("native text GPU rendering is required");
    let mut target = RenderTarget::offscreen(&gpu, 360, 200);
    let mut renderer = Renderer::new(gpu.device.clone(), gpu.queue.clone(), target.format());
    let origin = PhysicalVector { x: 12.25, y: 7.125 };
    for curve in ["circular", "quartic"] {
        for kind in ["flat", "convex", "recessed"] {
            let mut request = vectors[7]["request"].clone();
            request["outline"]["size"] = json!({"width":320,"height":160});
            request["outline"]["curvature"] = json!(curve);
            request["outline"]["radii"] = json!({
                "topStart": {"x":20,"y":12}, "topEnd": {"x":30,"y":15},
                "bottomEnd": {"x":24,"y":18}, "bottomStart": {"x":12,"y":20}
            });
            request["edgeWidth"] = json!(0.25);
            request["points"] = json!([]);
            request["profile"] = json!({"kind":kind,"bodyThickness":4,
                "amplitude":if kind == "flat" {0.0} else {0.25},
                "shoulderWidth":if kind == "flat" {0.0} else {1.0}});
            for text_scale in [1.0, 2.0] {
                let typography = label_style::style(text_scale, 0.25, 400.0, 1.4);
                for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
                    request["outline"]["layoutDirection"] = json!(direction);
                    let paint = resolve_relief_paint_source(&request.to_string())
                        .unwrap()
                        .paint;
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
                    for scale in [1.0_f32, 1.25, 1.5, 2.0, 3.0] {
                        let [material, text] =
                            prepare_relief_content_at(&paint, &label, family, origin, scale, 4)
                                .unwrap();
                        let truncated_text = if curve == "circular"
                            && kind == "flat"
                            && text_scale == 2.0
                            && direction == LayoutDirection::Ltr
                            && scale == 1.0
                        {
                            let unwrapped = measure_command_label(
                                family,
                                LabelMeasureInput {
                                    text: label.text(),
                                    typography: label.typography(),
                                    maximum_width: None,
                                },
                            )
                            .unwrap();
                            assert!(label.label_bounds().height > unwrapped.height);
                            let mut truncated = text.clone();
                            let DrawCommand::Text { rect, .. } = &mut truncated else {
                                panic!("native text required")
                            };
                            rect.height = 1.0;
                            Some(truncated)
                        } else {
                            None
                        };
                        let mut expanded_text = text.clone();
                        let DrawCommand::Text {
                            text: complete,
                            rect,
                            fit,
                            ..
                        } = &mut expanded_text
                        else {
                            panic!("complete native text required")
                        };
                        assert_eq!(complete.as_ref(), "j Reconnect now");
                        if fit.is_none() {
                            *fit = Some(LineFit::wrapping(Some(rect.width)));
                        }
                        rect.x -= 32.0;
                        rect.width += 64.0;
                        rect.height = label.label_bounds().height as f32 + 64.0;
                        let mut root = RenderNode::new(1);
                        root.commands.push(Rc::new(material));
                        let width = (360.0 * scale).round() as u32;
                        let height = (200.0 * scale).round() as u32;
                        target.resize(width, height);
                        renderer.set_screen_size(width as f32, height as f32);
                        renderer.set_scale_factor(scale);
                        let mut commands = Vec::new();
                        let mut layers = Vec::new();
                        flatten_root_into(
                            &root,
                            &mut commands,
                            &mut layers,
                            &mut FlattenScratch::default(),
                        );
                        assert!(renderer.render(&mut target, &commands, &layers, Color::WHITE));
                        let background = readback::read_frame(&target);
                        root.commands.push(Rc::new(text));
                        commands.clear();
                        layers.clear();
                        flatten_root_into(
                            &root,
                            &mut commands,
                            &mut layers,
                            &mut FlattenScratch::default(),
                        );
                        assert!(renderer.render(&mut target, &commands, &layers, Color::WHITE));
                        let composed = readback::read_frame(&target);
                        root.commands.pop();
                        root.commands.push(Rc::new(expanded_text));
                        commands.clear();
                        layers.clear();
                        flatten_root_into(
                            &root,
                            &mut commands,
                            &mut layers,
                            &mut FlattenScratch::default(),
                        );
                        assert!(renderer.render(&mut target, &commands, &layers, Color::WHITE));
                        assert_eq!(
                            composed,
                            readback::read_frame(&target),
                            "complete label must match an expanded native text rectangle"
                        );
                        if let Some(truncated) = truncated_text {
                            root.commands.pop();
                            root.commands.push(Rc::new(truncated));
                            commands.clear();
                            layers.clear();
                            flatten_root_into(
                                &root,
                                &mut commands,
                                &mut layers,
                                &mut FlattenScratch::default(),
                            );
                            assert!(renderer.render(&mut target, &commands, &layers, Color::WHITE));
                            let truncated_frame = readback::read_frame(&target);
                            assert_ne!(
                                truncated_frame, composed,
                                "missing wrapped line must be detected"
                            );
                            assert_ne!(
                                truncated_frame, background,
                                "truncated label must still have ink"
                            );
                            println!(
                                "deliberate wrapped-line truncation detected with remaining native ink"
                            );
                        }
                        let mut ink_pixels = 0;
                        for (index, (before, after)) in background
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .zip(composed.as_chunks::<4>().0)
                            .enumerate()
                        {
                            if before == after {
                                continue;
                            }
                            ink_pixels += 1;
                            assert_eq!(before[3], 255);
                            assert_eq!(after[3], 255);
                            for (channel, body) in
                                before[..3].iter().zip(paint.pigment().body().components())
                            {
                                assert!(
                                    channel.abs_diff((body * 255.0).round() as u8) <= 1,
                                    "actual glyph background must equal the uniform body pigment"
                                );
                            }
                            let x = index as u32 % width;
                            let y = index as u32 / width;
                            for dx in [0.0, 1.0] {
                                for dy in [0.0, 1.0] {
                                    let point = PhysicalVector {
                                        x: (f64::from(x) + dx) / f64::from(scale) - origin.x,
                                        y: (f64::from(y) + dy) / f64::from(scale) - origin.y,
                                    };
                                    assert_eq!(
                                        paint.sample_paint(point).unwrap().as_ref(),
                                        Some(paint.pigment().body()),
                                        "ink background changed for {kind}/{curve}/{direction:?}, text {text_scale}, device {scale}, pixel {x},{y}"
                                    );
                                }
                            }
                        }
                        assert!(ink_pixels > 0, "native glyph pixels required");
                        println!(
                            "{kind}/{curve}, {direction:?}, text {text_scale}, device {scale}: {ink_pixels} guarded native glyph pixels"
                        );
                        if let Some(directory) =
                            std::env::var_os("RESINA_RELIEF_CONTENT_CAPTURE_DIR")
                        {
                            std::fs::create_dir_all(&directory).unwrap();
                            readback::capture(&std::path::Path::new(&directory).join(format!("{curve}-{kind}-{direction:?}-text-{text_scale}-device-{scale}.ppm")),width,height,&composed);
                        }
                    }
                }
            }
        }
    }
}
