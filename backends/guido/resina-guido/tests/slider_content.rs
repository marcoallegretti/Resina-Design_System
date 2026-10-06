#![cfg(feature = "testing")]
#[path = "../../../../reference/rust/resina-resolver/tests/common/slider_snapshot.rs"]
mod fixture;
#[path = "common/readback.rs"]
mod readback;
use fixture::{Fixture, FixtureConfig, size};
use guido::{
    renderer::{
        DrawCommand, FlattenScratch, GpuContext, LineFit, RenderNode, RenderTarget, Renderer,
        flatten_root_into,
    },
    transform::Transform,
    widgets::{Color, ContentFit, TextOverflow, font::FontFamily},
};
use resina_environment::LayoutDirection;
use resina_guido::{
    LabelMeasureError, LabelPrepareError, PrepareError, SliderContentPrepareError,
    measure_command_label, prepare_slider_content,
};
use resina_model::PhysicalVector;
use resina_raster::{Viewport, render_surface_paint};
use resina_resolver::{
    SliderOrientation, SrgbFallback, opaque_contrast_ratio, resolve_slider_snapshot,
    resolve_srgb_fallback,
};
use serde_json::json;
use std::{convert::Infallible, path::Path, rc::Rc};

const UNWRAPPED: LineFit = LineFit {
    width: None,
    max_lines: None,
    overflow: TextOverflow::Clip,
    wrap: false,
};
fn load_font() -> FontFamily {
    guido::load_font(
        std::fs::read(std::env::var("RESINA_LABEL_FONT").expect("RESINA_LABEL_FONT required"))
            .unwrap(),
    );
    FontFamily::name("DejaVu Sans")
}
fn config() -> FixtureConfig<'static> {
    FixtureConfig {
        family: "cast",
        direction: LayoutDirection::Ltr,
        orientation: SliderOrientation::Horizontal,
        enabled: true,
        focused: true,
        read_only: false,
        preview: false,
        tracking: 0.15,
        text_scale: 1.0,
        text: "Volume",
        label_maximum_size: size(180.0, 240.0),
    }
}
#[test]
fn checked_slider_content_preserves_placed_parts_focus_and_native_labels() {
    let font = load_font();
    let capture = std::env::var_os("RESINA_SLIDER_CAPTURE_DIR").map(std::path::PathBuf::from);
    let mut frames = Frames::new();
    let mut count = 0;
    for material in ["cast", "frost", "elastomer"] {
        for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
            for orientation in [SliderOrientation::Horizontal, SliderOrientation::Vertical] {
                for (name, enabled, focused, read_only, preview) in [
                    ("rest", true, false, false, false),
                    ("focused", true, true, false, false),
                    ("disabled", false, false, false, false),
                    ("readonly", true, true, true, false),
                    ("dragging", true, true, false, true),
                ] {
                    let mut options = config();
                    options.family = material;
                    options.direction = direction;
                    options.orientation = orientation;
                    options.enabled = enabled;
                    options.focused = focused;
                    options.read_only = read_only;
                    options.preview = preview;
                    options.text = if direction == LayoutDirection::Ltr {
                        "Volume"
                    } else {
                        "مستوى الصوت"
                    };
                    let f = Fixture::resolve(options, |input| measure_command_label(font, input));
                    let snapshot = resolve_slider_snapshot(f.input()).unwrap();
                    assert!(snapshot.track().paint().focus().is_none());
                    assert_eq!(snapshot.thumb().paint().focus().is_some(), focused);
                    assert_eq!(
                        snapshot.accessibility().value(),
                        f.presentation.visible().value()
                    );
                    assert_eq!(snapshot.accessibility().state().read_only(), read_only);
                    if preview {
                        assert_ne!(f.presentation.visible(), f.presentation.committed());
                    }
                    for scale in [1.0, 1.25, 2.0] {
                        let commands = prepare_slider_content(&snapshot, font, scale, 4).unwrap();
                        check_commands(
                            &mut frames,
                            &f,
                            scale,
                            &format!("{material}-{direction:?}-{orientation:?}-{name}"),
                            capture.as_deref(),
                            &commands,
                        );
                        count += 1;
                    }
                }
                let mut options = config();
                options.family = material;
                options.direction = direction;
                options.orientation = orientation;
                options.text_scale = 2.0;
                options.text = if direction == LayoutDirection::Ltr {
                    "Output volume level"
                } else {
                    "مستوى الصوت الخارج"
                };
                let f = Fixture::resolve(options, |input| measure_command_label(font, input));
                assert!(
                    f.label.label_bounds().height
                        > f.label.typography().font_size() * f.label.typography().line_height()
                );
                let snapshot = resolve_slider_snapshot(f.input()).unwrap();
                let commands = prepare_slider_content(&snapshot, font, 1.25, 4).unwrap();
                check_commands(
                    &mut frames,
                    &f,
                    1.25,
                    &format!("{material}-{direction:?}-{orientation:?}-scaled-wrapped"),
                    capture.as_deref(),
                    &commands,
                );
                count += 1;
            }
        }
    }
    assert_eq!(count, 192);
}
#[test]
fn unsupported_labels_and_invalid_raster_inputs_fail_before_publication() {
    let font = load_font();
    let f = Fixture::resolve(config(), |input| measure_command_label(font, input));
    let snapshot = resolve_slider_snapshot(f.input()).unwrap();
    assert!(matches!(
        prepare_slider_content(&snapshot, font, 0.0, 4),
        Err(SliderContentPrepareError::Track(PrepareError::InvalidScale))
    ));
    assert!(matches!(
        prepare_slider_content(&snapshot, font, 1.0, 0),
        Err(SliderContentPrepareError::Track(PrepareError::Raster(_)))
    ));
    let mut options = config();
    options.tracking = 1.0e10 + 0.5;
    let f = Fixture::resolve(options, |input| {
        Ok::<_, Infallible>(size(
            80.0,
            input.typography.font_size() * input.typography.line_height(),
        ))
    });
    let snapshot = resolve_slider_snapshot(f.input()).unwrap();
    let error = prepare_slider_content(&snapshot, font, 1.0, 4).unwrap_err();
    assert!(std::error::Error::source(&error).is_some());
    assert!(error.to_string().contains("letter spacing"));
    assert!(matches!(
        error,
        SliderContentPrepareError::Label(LabelPrepareError::Measurement(
            LabelMeasureError::Precision("letter spacing")
        ))
    ));
    let mut options = config();
    options.text = "Output volume level";
    let f = Fixture::resolve(options, |input| {
        Ok::<_, Infallible>(size(
            80.0,
            input.typography.font_size() * input.typography.line_height(),
        ))
    });
    let snapshot = resolve_slider_snapshot(f.input()).unwrap();
    assert!(matches!(
        prepare_slider_content(&snapshot, font, 1.0, 4),
        Err(SliderContentPrepareError::Label(
            LabelPrepareError::MeasurementMismatch
        ))
    ));
}
struct Frames {
    target: RenderTarget,
    renderer: Renderer,
}
impl Frames {
    fn new() -> Self {
        let gpu = GpuContext::try_new().expect("native Slider GPU rendering is required");
        let target = RenderTarget::offscreen(&gpu, 640, 560);
        let renderer = Renderer::new(gpu.device.clone(), gpu.queue.clone(), target.format());
        Self { target, renderer }
    }
    fn render(&mut self, commands: &[DrawCommand], scale: f32, clear: Color) -> Vec<u8> {
        let mut root = RenderNode::new(1);
        root.local_transform = Transform::translate(220.0, 200.0);
        root.commands.extend(commands.iter().cloned().map(Rc::new));
        let (mut flattened, mut layers, mut scratch) =
            (Vec::new(), Vec::new(), FlattenScratch::default());
        flatten_root_into(&root, &mut flattened, &mut layers, &mut scratch);
        let (width, height) = ((640.0 * scale) as u32, (560.0 * scale) as u32);
        self.target.resize(width, height);
        self.renderer.set_screen_size(width as f32, height as f32);
        self.renderer.set_scale_factor(scale);
        assert!(
            self.renderer
                .render(&mut self.target, &flattened, &layers, clear)
        );
        readback::read_frame(&self.target)
    }
}
fn color(value: &SrgbFallback) -> Color {
    let [r, g, b] = value.components().map(|v| v as f32);
    Color::rgba(r, g, b, value.alpha() as f32)
}
fn pixel_color(pixel: &[u8]) -> SrgbFallback {
    resolve_srgb_fallback(&json!({"colorSpace":"srgb","components":pixel[..3].iter().map(|v|f64::from(*v)/255.0).collect::<Vec<_>>(),"alpha":1})).unwrap()
}
fn check_commands(
    frames: &mut Frames,
    f: &Fixture,
    scale: f32,
    name: &str,
    capture: Option<&Path>,
    commands: &[DrawCommand; 3],
) {
    let origin = f.input().label_origin;
    let output_width = (640.0 * scale) as u32;
    for (index, paint, offset) in [
        (
            0,
            f.track.paint(),
            PhysicalVector {
                x: f.layout.track_bounds().x,
                y: f.layout.track_bounds().y,
            },
        ),
        (
            1,
            f.thumb.paint(),
            PhysicalVector {
                x: f.layout.thumb_bounds().x,
                y: f.layout.thumb_bounds().y,
            },
        ),
    ] {
        let DrawCommand::Image {
            source:
                guido::prelude::ImageSource::Rgba {
                    width,
                    height,
                    pixels,
                },
            rect,
            content_fit,
            tint,
            ..
        } = &commands[index]
        else {
            panic!("complete part paint required")
        };
        assert_eq!(*content_fit, ContentFit::Fill);
        assert!(tint.is_none());
        let local_bounds = if let Some(focus) = paint.focus() {
            let outer = focus.geometry().outer();
            let mut bounds = outer.contour().bounds().unwrap();
            bounds.x += outer.offset().x;
            bounds.y += outer.offset().y;
            bounds
        } else {
            paint.body().geometry().silhouette().bounds().unwrap()
        };
        let placed = PhysicalVector {
            x: ((local_bounds.x + offset.x) * f64::from(scale)).floor() / f64::from(scale),
            y: ((local_bounds.y + offset.y) * f64::from(scale)).floor() / f64::from(scale),
        };
        assert!((f64::from(rect.x) - placed.x).abs() <= 1.0 / 1024.0);
        assert!((f64::from(rect.y) - placed.y).abs() <= 1.0 / 1024.0);
        let expected = render_surface_paint(
            paint,
            Viewport {
                origin: PhysicalVector {
                    x: placed.x - offset.x,
                    y: placed.y - offset.y,
                },
                width: *width,
                height: *height,
                pixels_per_unit: f64::from(scale),
            },
            4,
        )
        .unwrap();
        assert_eq!(
            &**pixels,
            expected.rgba(),
            "{name}: part must sample its placed device grid"
        );
        for value in [rect.x * scale, rect.y * scale] {
            assert!((value - value.round()).abs() < 0.001);
        }
        let actual = frames.render(&commands[index..index + 1], scale, color(&f.white));
        let ox = ((220.0 + rect.x) * scale).round() as u32;
        let oy = ((200.0 + rect.y) * scale).round() as u32;
        let mut opaque = 0;
        for (i, pixel) in pixels
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .filter(|(_, p)| p[3] == 255)
        {
            let destination =
                (((oy + i as u32 / *width) * output_width + ox + i as u32 % *width) * 4) as usize;
            for channel in 0..4 {
                assert!(
                    actual[destination + channel].abs_diff(pixel[channel]) <= 1,
                    "{name}: native part {index} mismatch"
                );
            }
            opaque += 1;
        }
        assert!(opaque > 8);
    }
    let DrawCommand::Text {
        text,
        rect,
        color: foreground,
        fit,
        ..
    } = &commands[2]
    else {
        panic!("complete measured label required")
    };
    assert_eq!(text, f.label.text());
    let lines = (f.label.label_bounds().height
        / (f.label.typography().font_size() * f.label.typography().line_height()))
    .round();
    let paragraphs = f.label.text().split('\n').count() as f64;
    assert_eq!(*fit, (lines == paragraphs).then_some(UNWRAPPED));
    let label = f.label.label_bounds();
    assert!((f64::from(rect.x) - origin.x - label.x).abs() <= 1.0 / 1024.0);
    assert!((f64::from(rect.y) - origin.y - label.y).abs() <= 1.0 / 1024.0);
    let expected = color(&f.black);
    assert_eq!(
        [foreground.r, foreground.g, foreground.b, foreground.a],
        [expected.r, expected.g, expected.b, expected.a]
    );
    let background = color(&f.white);
    let backdrop_bytes = f.white.components().map(|v| (v * 255.0).round() as u8);
    let combined = frames.render(commands, scale, background);
    let DrawCommand::Image {
        source: guido::prelude::ImageSource::Rgba { width, pixels, .. },
        rect,
        ..
    } = &commands[1]
    else {
        panic!("complete thumb paint required")
    };
    let ox = ((220.0 + rect.x) * scale).round() as u32;
    let oy = ((200.0 + rect.y) * scale).round() as u32;
    for (i, pixel) in pixels
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(_, p)| p[3] == 255)
    {
        let at = (((oy + i as u32 / *width) * output_width + ox + i as u32 % *width) * 4) as usize;
        for channel in 0..4 {
            assert!(
                combined[at + channel].abs_diff(pixel[channel]) <= 1,
                "{name}: complete thumb and navigation must remain above the track"
            );
        }
    }
    let parts = frames.render(&commands[..2], scale, background);
    let mask = frames.render(&commands[2..], scale, Color::TRANSPARENT);
    let line_height = f.label.typography().font_size() * f.label.typography().line_height();
    let mut ink = vec![0; (label.height / line_height).round() as usize];
    let mut solids = 0;
    for (i, pixel) in mask
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(_, p)| p[3] > 0)
    {
        let x = (i as u32 % output_width) as f64 / f64::from(scale) - 220.0;
        let y = (i as u32 / output_width) as f64 / f64::from(scale) - 200.0;
        assert!(
            x >= origin.x
                && x + 1.0 / f64::from(scale) <= origin.x + f.label.size().width
                && y >= origin.y
                && y + 1.0 / f64::from(scale) <= origin.y + f.label.size().height,
            "{name}: native glyph escaped reserved label slot"
        );
        let line = ((y - origin.y - label.y) / line_height)
            .floor()
            .clamp(0.0, (ink.len() - 1) as f64) as usize;
        ink[line] += 1;
        let at = i * 4;
        assert_eq!(parts[at + 3], 255);
        for channel in 0..3 {
            assert!(
                parts[at + channel].abs_diff(backdrop_bytes[channel]) <= 1,
                "{name}: glyph backdrop differs from the checked uniform color"
            );
        }
        if pixel[3] == 255 {
            solids += 1;
            assert_ne!(
                &combined[at..at + 3],
                &parts[at..at + 3],
                "{name}: native glyph disappeared"
            );
            assert!(
                opaque_contrast_ratio(
                    &pixel_color(&combined[at..at + 4]),
                    &pixel_color(&parts[at..at + 4])
                )
                .unwrap()
                    >= 4.5,
                "{name}: native glyph contrast failed"
            );
        }
    }
    assert!(solids > 8);
    assert!(
        ink.iter().all(|n| *n > 8),
        "{name}: complete shaped line missing"
    );
    if let Some(path) = capture {
        std::fs::create_dir_all(path).unwrap();
        readback::capture(
            &path.join(format!("{name}-{scale}.ppm")),
            output_width,
            (560.0 * scale) as u32,
            &combined,
        );
    }
}
