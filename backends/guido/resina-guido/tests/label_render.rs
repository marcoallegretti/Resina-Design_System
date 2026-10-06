#![cfg(feature = "testing")]

#[path = "common/cases.rs"]
mod label_cases;
#[path = "common/label.rs"]
mod label_style;

use guido::{
    renderer::{
        DrawCommand, FlattenScratch, GpuContext, LineFit, RenderNode, RenderTarget, Renderer,
        flatten_root_into,
    },
    widgets::{Color, TextAlign, TextOverflow, font::FontFamily},
};
use resina_environment::{LayoutDirection, SafeArea};
use resina_guido::{
    LabelMeasureError, LabelPrepareError, measure_command_label, prepare_command_label,
};
use resina_model::SurfaceSize;
use resina_resolver::{CommandLabelInput, LabelMeasureInput, resolve_command_label};
use std::rc::Rc;
#[path = "common/readback.rs"]
mod readback;
use readback::{capture, read_frame};

const UNWRAPPED: LineFit = LineFit {
    width: None,
    max_lines: None,
    overflow: TextOverflow::Clip,
    wrap: false,
};

fn render_one(
    target: &mut RenderTarget,
    renderer: &mut Renderer,
    command: DrawCommand,
    device_scale: f32,
) -> (Vec<u8>, u32, u32) {
    let mut root = RenderNode::new(1);
    root.commands.push(Rc::new(command));
    let (mut commands, mut layers, mut scratch) =
        (Vec::new(), Vec::new(), FlattenScratch::default());
    flatten_root_into(&root, &mut commands, &mut layers, &mut scratch);
    let width = (220.0 * device_scale).round() as u32;
    let height = (300.0 * device_scale).round() as u32;
    target.resize(width, height);
    renderer.set_screen_size(width as f32, height as f32);
    renderer.set_scale_factor(device_scale);
    assert!(renderer.render(target, &commands, &layers, Color::WHITE));
    (read_frame(target), width, height)
}

/// Ink bounds of black text on white: left, top, right, bottom.
fn ink_box(pixels: &[u8], width: u32) -> Option<[u32; 4]> {
    let mut bounds: Option<[u32; 4]> = None;
    for (index, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
        if pixel[0] < 240 {
            let (x, y) = (index as u32 % width, index as u32 / width);
            let b = bounds.get_or_insert([x, y, x, y]);
            *b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        }
    }
    bounds
}

/// A drawing in a wider box cannot break a line the label box would break again.
fn assert_matches_unconstrained(
    target: &mut RenderTarget,
    renderer: &mut Renderer,
    command: &DrawCommand,
    device_scale: f32,
    drawn: &[u8],
    name: &str,
) {
    let mut reference = command.clone();
    let DrawCommand::Text { rect, fit, .. } = &mut reference else {
        panic!("text command required")
    };
    rect.x -= 100.0;
    rect.width += 200.0;
    *fit = Some(UNWRAPPED);
    let (pixels, width, _) = render_one(target, renderer, reference, device_scale);
    let (expected, actual) = (
        ink_box(&pixels, width).unwrap(),
        ink_box(drawn, width).unwrap(),
    );
    assert!(
        expected.iter().zip(actual).all(|(e, a)| e.abs_diff(a) <= 1),
        "{name} at device scale {device_scale}: ink {actual:?}, unconstrained {expected:?}"
    );
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
    let captures = std::env::var_os("RESINA_LABEL_CAPTURE_DIR").map(std::path::PathBuf::from);
    if let Some(path) = &captures {
        std::fs::create_dir_all(path).unwrap();
    }
    for text_scale in [1.0, 1.5, 2.0] {
        let typography = label_style::style(text_scale, 0.15, 400.0, 1.4);
        for case in label_cases::cases() {
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
            let line_height = typography.font_size() * typography.line_height();
            let lines = (ir.label_bounds().height / line_height).round() as usize;
            for device_scale in [1.0_f32, 1.25, 2.0, 3.0] {
                let command =
                    prepare_command_label(&ir, family, Color::BLACK, device_scale).unwrap();
                let DrawCommand::Text {
                    text: drawn,
                    rect,
                    align,
                    fit,
                    letter_spacing,
                    ..
                } = &command
                else {
                    panic!("text command required")
                };
                assert_eq!(drawn, text);
                assert_eq!(*letter_spacing, (0.15 * text_scale) as f32);
                assert_eq!(*align, TextAlign::Center);
                let unwrapped = lines == text.split('\n').count();
                assert_eq!(*fit, unwrapped.then_some(UNWRAPPED));
                assert!(f64::from(rect.width) <= ir.label_bounds().width);
                let (pixels, width, height) =
                    render_one(&mut target, &mut renderer, command.clone(), device_scale);
                if unwrapped {
                    assert_matches_unconstrained(
                        &mut target,
                        &mut renderer,
                        &command,
                        device_scale,
                        &pixels,
                        name,
                    );
                }
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
                let below = ((ir.label_bounds().y
                    + ir.label_bounds().height
                    + typography.font_size() * 0.5)
                    * scale)
                    .ceil() as u32;
                for y in below..height {
                    for x in 0..width {
                        assert!(
                            pixels[((y * width + x) * 4) as usize] >= 240,
                            "{name} text {text_scale} device {device_scale} drew an unresolved line"
                        );
                    }
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
                prepare_command_label(&ir, family, Color::rgba(f32::NAN, 0.0, 0.0, 1.0), 1.0)
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
        prepare_command_label(&ir, family, Color::BLACK, 1.0).unwrap_err(),
        LabelPrepareError::MeasurementMismatch
    );
    for color in [
        Color::rgba(-0.1, 0.0, 0.0, 1.0),
        Color::rgba(0.0, 1.1, 0.0, 1.0),
        Color::rgba(0.0, 0.0, f32::INFINITY, 1.0),
        Color::rgba(0.0, 0.0, 0.0, 2.0),
    ] {
        assert_eq!(
            prepare_command_label(&ir, family, color, 1.0).unwrap_err(),
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
        prepare_command_label(&ir, family, Color::BLACK, 1.0).unwrap_err(),
        LabelPrepareError::Geometry
    );
    let spaced = label_style::style(1.0, 0.15, 400.0, 1.4);
    let word = measure_command_label(
        family,
        LabelMeasureInput {
            text: "Save",
            typography: &spaced,
            maximum_width: None,
        },
    )
    .unwrap();
    let exact = resolve_command_label(
        CommandLabelInput {
            text: "Save Save",
            typography: &spaced,
            minimum_size: SurfaceSize {
                width: word.width,
                height: 44.0,
            },
            maximum_size: SurfaceSize {
                width: word.width,
                height: 300.0,
            },
            padding: SafeArea {
                start: 0.0,
                end: 0.0,
                top: 8.0,
                bottom: 8.0,
            },
            direction: LayoutDirection::Ltr,
        },
        |input| measure_command_label(family, input),
    )
    .unwrap();
    assert_eq!(exact.label_bounds().height, 2.0 * 20.0 * 1.4);
    let DrawCommand::Text { fit, .. } =
        prepare_command_label(&exact, family, Color::BLACK, 2.0).unwrap()
    else {
        panic!("text command required")
    };
    assert!(fit.is_none());
    // Each line exactly fills the box; shaping at these device sizes wraps it again.
    for device_scale in [1.5, 3.0] {
        assert_eq!(
            prepare_command_label(&exact, family, Color::BLACK, device_scale).unwrap_err(),
            LabelPrepareError::ScaledLineMismatch
        );
    }
    for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            prepare_command_label(&exact, family, Color::BLACK, scale).unwrap_err(),
            LabelPrepareError::InvalidScale
        );
    }
    let edit_fit = measure_command_label(
        family,
        LabelMeasureInput {
            text: "Edit Fit",
            typography: &spaced,
            maximum_width: None,
        },
    )
    .unwrap();
    // Both rebreak at 1.5 while keeping the complete height and widest line.
    for text in ["Edit Fit a\nFit Edit", "Fit Edit Edit Fit a"] {
        let rebreaking = resolve_command_label(
            CommandLabelInput {
                text,
                typography: &spaced,
                minimum_size: SurfaceSize {
                    width: edit_fit.width,
                    height: 44.0,
                },
                maximum_size: SurfaceSize {
                    width: edit_fit.width,
                    height: 300.0,
                },
                padding: SafeArea {
                    start: 0.0,
                    end: 0.0,
                    top: 8.0,
                    bottom: 8.0,
                },
                direction: LayoutDirection::Ltr,
            },
            |input| measure_command_label(family, input),
        )
        .unwrap();
        assert_eq!(rebreaking.label_bounds().height, 3.0 * 20.0 * 1.4);
        for device_scale in [1.5, 3.0] {
            assert_eq!(
                prepare_command_label(&rebreaking, family, Color::BLACK, device_scale).unwrap_err(),
                LabelPrepareError::ScaledLineMismatch,
                "{text:?} at device scale {device_scale}"
            );
        }
    }
    for scale in [f32::MAX, 1.0e-4] {
        assert_eq!(
            prepare_command_label(&exact, family, Color::BLACK, scale).unwrap_err(),
            LabelPrepareError::ScaledMetrics
        );
    }
}

#[test]
fn native_drawing_applies_the_measured_letter_spacing() {
    guido::load_font(
        std::fs::read(std::env::var("RESINA_LABEL_FONT").expect("RESINA_LABEL_FONT is required"))
            .unwrap(),
    );
    let family = FontFamily::name("DejaVu Sans");
    let gpu = GpuContext::try_new().expect("native label GPU rendering is required");
    let mut target = RenderTarget::offscreen(&gpu, 220, 80);
    let mut renderer = Renderer::new(gpu.device.clone(), gpu.queue.clone(), target.format());
    let ink_span =
        |tracking: f64, device_scale: f32, target: &mut RenderTarget, renderer: &mut Renderer| {
            let typography = label_style::style(1.0, tracking, 400.0, 1.4);
            let ir = resolve_command_label(
                CommandLabelInput {
                    text: "Save",
                    typography: &typography,
                    minimum_size: SurfaceSize {
                        width: 220.0,
                        height: 44.0,
                    },
                    maximum_size: SurfaceSize {
                        width: 220.0,
                        height: 80.0,
                    },
                    padding: SafeArea {
                        start: 0.0,
                        end: 0.0,
                        top: 8.0,
                        bottom: 8.0,
                    },
                    direction: LayoutDirection::Ltr,
                },
                |input| measure_command_label(family, input),
            )
            .unwrap();
            let mut root = RenderNode::new(1);
            root.commands.push(Rc::new(
                prepare_command_label(&ir, family, Color::BLACK, device_scale).unwrap(),
            ));
            let (mut commands, mut layers, mut scratch) =
                (Vec::new(), Vec::new(), FlattenScratch::default());
            flatten_root_into(&root, &mut commands, &mut layers, &mut scratch);
            let width = (220.0 * device_scale).round() as u32;
            let height = (80.0 * device_scale).round() as u32;
            target.resize(width, height);
            renderer.set_screen_size(width as f32, height as f32);
            renderer.set_scale_factor(device_scale);
            assert!(renderer.render(target, &commands, &layers, Color::WHITE));
            let pixels = read_frame(target);
            let columns: Vec<u32> = (0..width)
                .filter(|x| (0..height).any(|y| pixels[((y * width + x) * 4) as usize] < 240))
                .collect();
            f64::from(columns.last().unwrap() - columns.first().unwrap())
        };
    for device_scale in [1.0_f32, 2.0] {
        let unspaced = ink_span(0.0, device_scale, &mut target, &mut renderer);
        for tracking in [-1.0, 2.0] {
            // Spacing follows every glyph; only the three interior gaps move ink.
            let expected = unspaced + 3.0 * tracking * f64::from(device_scale);
            let actual = ink_span(tracking, device_scale, &mut target, &mut renderer);
            assert!(
                (actual - expected).abs() <= 1.0,
                "spacing {tracking} at device scale {device_scale}: ink span {actual}, expected {expected}"
            );
        }
    }
}

#[test]
fn hard_breaks_and_empty_paragraphs_draw_their_measured_lines() {
    guido::load_font(
        std::fs::read(std::env::var("RESINA_LABEL_FONT").expect("RESINA_LABEL_FONT is required"))
            .unwrap(),
    );
    let family = FontFamily::name("DejaVu Sans");
    let gpu = GpuContext::try_new().expect("native label GPU rendering is required");
    let mut target = RenderTarget::offscreen(&gpu, 220, 300);
    let mut renderer = Renderer::new(gpu.device.clone(), gpu.queue.clone(), target.format());
    let typography = label_style::style(1.0, 0.15, 400.0, 1.4);
    let line_height = typography.font_size() * typography.line_height();
    for (text, direction) in [
        ("Save\nNow", LayoutDirection::Ltr),
        ("Save\n\nNow", LayoutDirection::Ltr),
        ("\nSave", LayoutDirection::Ltr),
        ("\u{5e9}\u{5de}\u{5d5}\u{5e8}\nSave", LayoutDirection::Rtl),
    ] {
        let paragraphs: Vec<_> = text.split('\n').collect();
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
        assert_eq!(
            ir.label_bounds().height,
            paragraphs.len() as f64 * line_height
        );
        for device_scale in [1.0_f32, 1.5, 3.0] {
            let command = prepare_command_label(&ir, family, Color::BLACK, device_scale).unwrap();
            let DrawCommand::Text { fit, .. } = &command else {
                panic!("text command required")
            };
            assert_eq!(*fit, Some(UNWRAPPED), "{text:?}");
            let (pixels, width, height) =
                render_one(&mut target, &mut renderer, command.clone(), device_scale);
            assert_matches_unconstrained(
                &mut target,
                &mut renderer,
                &command,
                device_scale,
                &pixels,
                text,
            );
            let scale = f64::from(device_scale);
            let inked = |top: f64, bottom: f64| {
                let (top, bottom) = ((top * scale).ceil() as u32, (bottom * scale).floor() as u32);
                (top..bottom.min(height))
                    .flat_map(|y| (0..width).map(move |x| ((y * width + x) * 4) as usize))
                    .filter(|at| pixels[*at] < 240)
                    .count()
            };
            for (line, paragraph) in paragraphs.iter().enumerate() {
                let top = ir.label_bounds().y + line as f64 * line_height;
                let ink = inked(top + 1.0, top + line_height - 1.0);
                assert_eq!(
                    ink > 8,
                    !paragraph.is_empty(),
                    "{text:?} line {line} at device scale {device_scale}"
                );
            }
            let bottom = ir.label_bounds().y + ir.label_bounds().height;
            assert_eq!(inked(bottom + typography.font_size() * 0.5, 300.0), 0);
        }
    }
}
