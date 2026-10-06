#![cfg(feature = "testing")]

#[path = "common/label.rs"]
mod label_style;
#[path = "common/readback.rs"]
mod readback;

use guido::{
    renderer::{
        DrawCommand, FlattenScratch, GpuContext, RenderNode, RenderTarget, Renderer,
        flatten_root_into,
    },
    widgets::{Color, font::FontFamily},
};
use resina_environment::{LayoutDirection, SafeArea};
use resina_guido::{measure_command_label, prepare_command_label_at};
use resina_model::{PhysicalVector, SurfaceSize};
use resina_resolver::{CommandLabelInput, resolve_command_label};
use std::rc::Rc;

#[test]
fn native_label_advance_does_not_crop_glyph_overhang() {
    guido::load_font(
        std::fs::read(std::env::var("RESINA_LABEL_FONT").expect("RESINA_LABEL_FONT is required"))
            .unwrap(),
    );
    let family = FontFamily::name("DejaVu Sans");
    let typography = label_style::style(3.0, 0.0, 400.0, 1.4);
    assert_eq!(typography.font_size(), 60.0);
    let label = resolve_command_label(
        CommandLabelInput {
            text: "j",
            typography: &typography,
            minimum_size: SurfaceSize {
                width: 1.0,
                height: 1.0,
            },
            maximum_size: SurfaceSize {
                width: 200.0,
                height: 200.0,
            },
            padding: SafeArea {
                start: 0.0,
                end: 0.0,
                top: 0.0,
                bottom: 0.0,
            },
            direction: LayoutDirection::Ltr,
        },
        |input| measure_command_label(family, input),
    )
    .unwrap();
    assert!(label.label_bounds().width < 60.0);
    let bounded_origin = PhysicalVector { x: 80.0, y: 40.0 };
    let reference_origin = PhysicalVector { x: 220.0, y: 40.0 };
    let gpu = GpuContext::try_new().expect("native glyph GPU rendering is required");
    let mut target = RenderTarget::offscreen(&gpu, 400, 240);
    let mut renderer = Renderer::new(gpu.device.clone(), gpu.queue.clone(), target.format());
    for scale in [1.0_f32, 1.25, 2.0, 3.0] {
        let bounded =
            prepare_command_label_at(&label, family, Color::BLACK, bounded_origin, scale).unwrap();
        let mut reference =
            prepare_command_label_at(&label, family, Color::BLACK, reference_origin, scale)
                .unwrap();
        let DrawCommand::Text { rect, .. } = &mut reference else {
            panic!("native text command required")
        };
        rect.x -= 20.0;
        rect.width += 40.0;
        let mut root = RenderNode::new(1);
        root.commands.extend([Rc::new(bounded), Rc::new(reference)]);
        let mut commands = Vec::new();
        let mut layers = Vec::new();
        flatten_root_into(
            &root,
            &mut commands,
            &mut layers,
            &mut FlattenScratch::default(),
        );
        let width = (400.0 * scale) as u32;
        let height = (240.0 * scale) as u32;
        target.resize(width, height);
        renderer.set_screen_size(width as f32, height as f32);
        renderer.set_scale_factor(scale);
        assert!(renderer.render(&mut target, &commands, &layers, Color::WHITE));
        let pixels = readback::read_frame(&target);
        assert_eq!(pixels.len(), (width * height * 4) as usize);
        let pixel = |x: u32, y: u32| {
            let offset = ((y * width + x) * 4) as usize;
            &pixels[offset..offset + 4]
        };
        let bounded_left = (bounded_origin.x * f64::from(scale)) as u32;
        let reference_offset = ((reference_origin.x - bounded_origin.x) * f64::from(scale)) as u32;
        let comparison_start = ((bounded_origin.x - 20.0) * f64::from(scale)) as u32;
        let comparison_end = ((bounded_origin.x + 60.0) * f64::from(scale)) as u32;
        let mut overhang = 0;
        for y in 0..height {
            for x in comparison_start..comparison_end {
                assert_eq!(
                    pixel(x, y),
                    pixel(x + reference_offset, y),
                    "glyph ink differs at scale {scale}, ({x}, {y})"
                );
                assert_eq!(pixel(x, y)[3], 255);
                if x < bounded_left && pixel(x, y)[..3] != [255, 255, 255] {
                    overhang += 1;
                }
            }
        }
        assert!(
            overhang > 0,
            "fixture must demonstrate uncropped ink at scale {scale}"
        );
        println!("device scale {scale}: {overhang} visible pixels outside the advance box");
        if let Some(directory) = std::env::var_os("RESINA_LABEL_CAPTURE_DIR") {
            std::fs::create_dir_all(&directory).unwrap();
            let path = std::path::Path::new(&directory).join(format!("ink-device-{scale}.ppm"));
            readback::capture(&path, width, height, &pixels);
        }
    }
}
