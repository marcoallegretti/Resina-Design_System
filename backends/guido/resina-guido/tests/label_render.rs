#![cfg(feature = "testing")]

#[path = "common/label.rs"]
mod label_style;

use guido::{
    renderer::{
        DrawCommand, FlattenScratch, GpuContext, RenderNode, RenderTarget, Renderer,
        flatten_root_into,
    },
    widgets::{Color, TextAlign, font::FontFamily},
};
use resina_environment::{LayoutDirection, SafeArea};
use resina_guido::{
    LabelMeasureError, LabelPrepareError, measure_command_label, prepare_command_label,
};
use resina_model::SurfaceSize;
use resina_resolver::{CommandLabelInput, resolve_command_label};
use std::{path::Path, rc::Rc};

fn read_frame(target: &RenderTarget) -> Vec<u8> {
    let RenderTarget::Offscreen(target) = target else {
        panic!("offscreen target required")
    };
    let width = target.texture.width();
    let height = target.texture.height();
    let row_bytes = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = target.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Resina label conformance readback"),
        size: u64::from(row_bytes) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = target.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    target.queue.submit([encoder.finish()]);
    let (sent, received) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            sent.send(result).unwrap();
        });
    target
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("GPU readback must complete");
    received.recv().unwrap().expect("GPU readback must map");
    let mapped = buffer.slice(..).get_mapped_range().unwrap();
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in mapped.chunks_exact(row_bytes as usize) {
        pixels.extend_from_slice(&row[..(width * 4) as usize]);
    }
    pixels
}

fn capture(path: &Path, width: u32, height: u32, pixels: &[u8]) {
    let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
    for pixel in pixels.as_chunks::<4>().0 {
        ppm.extend_from_slice(&pixel[..3]);
    }
    std::fs::write(path, ppm).unwrap();
}

#[test]
fn complete_centered_labels_reach_every_native_line_at_all_scales() {
    guido::load_font(
        std::fs::read(std::env::var("RESINA_LABEL_FONT").expect("RESINA_LABEL_FONT is required"))
            .unwrap(),
    );
    let family = FontFamily::name("DejaVu Sans");
    let gpu = GpuContext::try_new().expect("native label GPU rendering is required");
    let mut target = RenderTarget::offscreen(&gpu, 220, 300);
    let mut renderer = Renderer::new(gpu.device.clone(), gpu.queue.clone(), target.format());
    let mut commands = Vec::new();
    let mut layers = Vec::new();
    let mut scratch = FlattenScratch::default();
    let captures = std::env::var_os("RESINA_LABEL_CAPTURE_DIR").map(std::path::PathBuf::from);
    if let Some(path) = &captures {
        std::fs::create_dir_all(path).unwrap();
    }
    for text_scale in [1.0, 1.5, 2.0] {
        let typography = label_style::style(text_scale, 0.0, 400.0, 1.4);
        for case in label_style::cases() {
            let name = case.name.as_str();
            let text = case.text.as_str();
            let direction = case.direction;
            let ir = resolve_command_label(
                CommandLabelInput {
                    text,
                    typography: &typography,
                    minimum_size: SurfaceSize {
                        width: 64.0,
                        height: 44.0,
                    },
                    maximum_size: SurfaceSize {
                        width: 220.0,
                        height: 300.0,
                    },
                    padding: SafeArea {
                        start: 16.0,
                        end: 12.0,
                        top: 8.0,
                        bottom: 8.0,
                    },
                    direction,
                },
                |input| measure_command_label(family, input),
            )
            .unwrap();
            let command = prepare_command_label(&ir, family, Color::BLACK).unwrap();
            let DrawCommand::Text {
                text: drawn,
                rect,
                align,
                fit,
                ..
            } = &command
            else {
                panic!("text command required")
            };
            assert_eq!(drawn, text);
            assert_eq!(*align, TextAlign::Center);
            assert!(fit.is_none());
            assert!(f64::from(rect.width) <= ir.label_bounds().width);
            let line_height = typography.font_size() * typography.line_height();
            let lines = (ir.label_bounds().height / line_height).round() as usize;
            let mut root = RenderNode::new(1);
            root.commands.push(Rc::new(command));
            flatten_root_into(&root, &mut commands, &mut layers, &mut scratch);
            for device_scale in [1.0_f32, 1.25, 2.0, 3.0] {
                let width = (220.0 * device_scale).round() as u32;
                let height = (300.0 * device_scale).round() as u32;
                target.resize(width, height);
                renderer.set_screen_size(width as f32, height as f32);
                renderer.set_scale_factor(device_scale);
                assert!(renderer.render(&mut target, &commands, &layers, Color::WHITE));
                let pixels = read_frame(&target);
                let scale = f64::from(device_scale);
                for line in 0..lines {
                    let top =
                        ((ir.label_bounds().y + line as f64 * line_height) * scale).floor() as u32;
                    let bottom = ((ir.label_bounds().y + (line + 1) as f64 * line_height) * scale)
                        .ceil() as u32;
                    let mut left = width;
                    let mut right = 0;
                    let mut ink = 0;
                    for y in top..bottom.min(height) {
                        for x in 0..width {
                            let at = ((y * width + x) * 4) as usize;
                            assert_eq!(pixels[at + 3], 255);
                            if pixels[at] < 240 {
                                left = left.min(x);
                                right = right.max(x);
                                ink += 1;
                            }
                        }
                    }
                    assert!(
                        ink > 8,
                        "{name} text {text_scale} device {device_scale} missing line {line}"
                    );
                    let center = (ir.label_bounds().x + ir.label_bounds().width * 0.5) * scale;
                    assert!(
                        ((f64::from(left) + f64::from(right)) * 0.5 - center).abs()
                            <= typography.font_size() * scale * 0.25 + 2.0,
                        "{name} line {line} is not centered at device scale {device_scale}"
                    );
                }
                if let Some(path) = &captures {
                    capture(
                        &path.join(format!(
                            "{name}-text-{text_scale}-device-{device_scale}.ppm"
                        )),
                        width,
                        height,
                        &pixels,
                    );
                }
            }
            assert_eq!(
                prepare_command_label(&ir, family, Color::rgba(f32::NAN, 0.0, 0.0, 1.0))
                    .unwrap_err(),
                LabelPrepareError::Color
            );
        }
    }
    let typography = label_style::style(1.0, 0.0, 400.0, 1.4);
    let ir = resolve_command_label(
        CommandLabelInput {
            text: "Save",
            typography: &typography,
            minimum_size: SurfaceSize {
                width: 64.0,
                height: 44.0,
            },
            maximum_size: SurfaceSize {
                width: 220.0,
                height: 300.0,
            },
            padding: SafeArea {
                start: 16.0,
                end: 12.0,
                top: 8.0,
                bottom: 8.0,
            },
            direction: LayoutDirection::Ltr,
        },
        |_| {
            Ok::<_, LabelMeasureError>(SurfaceSize {
                width: 20.0,
                height: 18.0,
            })
        },
    )
    .unwrap();
    assert_eq!(
        prepare_command_label(&ir, family, Color::BLACK).unwrap_err(),
        LabelPrepareError::MeasurementMismatch
    );
    for color in [
        Color::rgba(-0.1, 0.0, 0.0, 1.0),
        Color::rgba(0.0, 1.1, 0.0, 1.0),
        Color::rgba(0.0, 0.0, f32::INFINITY, 1.0),
        Color::rgba(0.0, 0.0, 0.0, 2.0),
    ] {
        assert_eq!(
            prepare_command_label(&ir, family, color).unwrap_err(),
            LabelPrepareError::Color
        );
    }
    let ir = resolve_command_label(
        CommandLabelInput {
            text: "Save",
            typography: &typography,
            minimum_size: SurfaceSize {
                width: 16_777_317.0,
                height: 44.0,
            },
            maximum_size: SurfaceSize {
                width: 16_777_317.0,
                height: 300.0,
            },
            padding: SafeArea {
                start: 16_777_217.0,
                end: 0.0,
                top: 8.0,
                bottom: 8.0,
            },
            direction: LayoutDirection::Ltr,
        },
        |input| measure_command_label(family, input),
    )
    .unwrap();
    assert_eq!(
        prepare_command_label(&ir, family, Color::BLACK).unwrap_err(),
        LabelPrepareError::Geometry
    );
}
