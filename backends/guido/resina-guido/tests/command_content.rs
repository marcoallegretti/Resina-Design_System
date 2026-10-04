#![cfg(feature = "testing")]

#[path = "common/cases.rs"]
mod label_cases;
#[path = "common/readback.rs"]
mod readback;

use guido::{
    renderer::{
        DrawCommand, FlattenScratch, GpuContext, RenderNode, RenderTarget, Renderer,
        flatten_root_into,
    },
    transform::Transform,
    widgets::{Color, ContentFit, font::FontFamily},
};
use resina_environment::{LayoutDirection, SafeArea};
use resina_guido::{
    CommandContentPrepareError, LabelPrepareError, PrepareError, measure_command_label,
    prepare_command_content,
};
use resina_model::{ContourSegment, PhysicalVector, SurfaceSize, TypographyRole};
use resina_resolver::{
    CommandContentError, CommandLabelInput, CommandLabelIr, CommandMotionPolicy, CommandPaintIr,
    PlacedContour, opaque_contrast_ratio, resolve_command_label, resolve_command_motion_source,
    resolve_command_paint_source, resolve_srgb_fallback, resolve_theme_request_source,
};
use serde_json::{Value, json};
use std::{collections::HashSet, rc::Rc};

fn command_request(scene: &Value, phase: &str, scale: f64, direction: LayoutDirection) -> Value {
    let mut surface = scene["request"].clone();
    let body = &mut surface["body"];
    let mut theme: Value =
        serde_json::from_str(body["theme"]["themeSource"].as_str().unwrap()).unwrap();
    theme["materialAssignments"]["control"]["interactive"] =
        scene["expectedMaterialFamily"].clone();
    theme["typographyAssignments"]["roles"]["label"]["letterSpacing"] =
        json!("type.tracking.normal");
    body["theme"]["themeSource"] = json!(theme.to_string());
    body["theme"]["environment"]["textScale"] = json!(scale);
    body["theme"]["environment"]["layoutDirection"] = serde_json::to_value(direction).unwrap();
    body["surface"]["materialRole"] = json!("control.interactive");
    body["surface"]["states"]["states"] = json!([phase, "focused"]);
    let appearance: Value =
        serde_json::from_str(if scene["name"].as_str().unwrap().starts_with("dark-") {
            include_str!("../../../../definitions/command-appearance-dark.json")
        } else {
            include_str!("../../../../definitions/command-appearance-light.json")
        })
        .unwrap();
    json!({"schemaVersion":"0.1.0", "surface":surface, "commandAppearance":appearance})
}
fn resolve_label(
    request: &mut Value,
    text: &str,
    family: FontFamily,
    inset: f64,
) -> CommandLabelIr {
    let theme = &request["surface"]["body"]["theme"];
    let resolved = resolve_theme_request_source(&theme.to_string()).unwrap();
    let direction =
        serde_json::from_value(theme["environment"]["layoutDirection"].clone()).unwrap();
    let label = resolve_command_label(
        CommandLabelInput {
            text,
            typography: &resolved.typography()[&TypographyRole::Label],
            minimum_size: SurfaceSize {
                width: 96.0,
                height: 48.0,
            },
            maximum_size: SurfaceSize {
                width: 260.0,
                height: 360.0,
            },
            padding: SafeArea {
                start: inset,
                end: inset,
                top: 12.0,
                bottom: 12.0,
            },
            direction,
        },
        |input| measure_command_label(family, input),
    )
    .unwrap();
    request["surface"]["body"]["size"] = serde_json::to_value(label.size()).unwrap();
    label
}

fn background(request: &Value) -> Color {
    let color = resolve_srgb_fallback(&request["surface"]["surroundingColor"]).unwrap();
    let [r, g, b] = color.components().map(|value| value as f32);
    assert_eq!(color.alpha(), 1.0);
    Color::rgb(r, g, b)
}

struct NativeFrames {
    family: FontFamily,
    target: RenderTarget,
    renderer: Renderer,
}
impl NativeFrames {
    fn new(family: FontFamily) -> Self {
        let gpu = GpuContext::try_new().expect("native command content GPU rendering is required");
        let target = RenderTarget::offscreen(&gpu, 300, 400);
        let renderer = Renderer::new(gpu.device.clone(), gpu.queue.clone(), target.format());
        Self {
            family,
            target,
            renderer,
        }
    }
    fn render(
        &mut self,
        commands: &[DrawCommand],
        width: u32,
        height: u32,
        scale: f32,
        background: Color,
    ) -> Vec<u8> {
        let mut root = RenderNode::new(1);
        root.local_transform = Transform::translate(16.0, 16.0);
        root.commands.extend(commands.iter().cloned().map(Rc::new));
        let mut flattened = Vec::new();
        let mut layers = Vec::new();
        let mut scratch = FlattenScratch::default();
        flatten_root_into(&root, &mut flattened, &mut layers, &mut scratch);
        self.target.resize(width, height);
        self.renderer.set_screen_size(width as f32, height as f32);
        self.renderer.set_scale_factor(scale);
        assert!(
            self.renderer
                .render(&mut self.target, &flattened, &layers, background)
        );
        readback::read_frame(&self.target)
    }
}
fn in_content(contour: &PlacedContour, point: PhysicalVector) -> bool {
    let point = PhysicalVector {
        x: point.x - contour.offset().x,
        y: point.y - contour.offset().y,
    };
    let bounds = contour.contour().bounds().unwrap();
    if point.x < bounds.x
        || point.y < bounds.y
        || point.x > bounds.x + bounds.width
        || point.y > bounds.y + bounds.height
    {
        return false;
    }
    for segment in contour.contour().segments() {
        if let ContourSegment::Arc {
            center,
            radii,
            start,
            end,
        } = segment
        {
            assert_eq!(radii.x, radii.y);
            assert_eq!(start.x * end.x + start.y * end.y, 0.0);
            let x = point.x - center.x;
            let y = point.y - center.y;
            if x * (start.x + end.x) > 0.0 && y * (start.y + end.y) > 0.0 && x.hypot(y) > radii.x {
                return false;
            }
        }
    }
    true
}
fn pixel_color(pixel: &[u8]) -> resina_resolver::SrgbFallback {
    resolve_srgb_fallback(&json!({"colorSpace":"srgb", "components":pixel[..3].iter().map(|v| f64::from(*v)/255.0).collect::<Vec<_>>(), "alpha":1})).unwrap()
}
fn check_frame(
    frames: &mut NativeFrames,
    paint: &CommandPaintIr,
    label: &CommandLabelIr,
    scale: f32,
    name: &str,
    capture: Option<&std::path::Path>,
    background: Color,
) {
    let commands = prepare_command_content(paint, label, frames.family, scale, 4).unwrap();
    let DrawCommand::Image {
        source:
            guido::prelude::ImageSource::Rgba {
                width: image_width,
                height: image_height,
                pixels,
            },
        decoded,
        rect,
        content_fit,
        tint,
    } = &commands[0]
    else {
        panic!("prepared material image must be first");
    };
    assert!(decoded.is_none() && tint.is_none());
    assert_eq!(*content_fit, ContentFit::Fill);
    let mut bounds = if let Some(focus) = paint.paint().focus() {
        let contour = focus.geometry().outer();
        let mut bounds = contour.contour().bounds().unwrap();
        bounds.x += contour.offset().x;
        bounds.y += contour.offset().y;
        bounds
    } else {
        paint
            .paint()
            .body()
            .geometry()
            .silhouette()
            .bounds()
            .unwrap()
    };
    bounds.x = (bounds.x * f64::from(scale)).floor() / f64::from(scale);
    bounds.y = (bounds.y * f64::from(scale)).floor() / f64::from(scale);
    assert!((f64::from(rect.x) - bounds.x).abs() <= 1.0 / 1024.0);
    assert!((f64::from(rect.y) - bounds.y).abs() <= 1.0 / 1024.0);
    assert!(
        (f64::from(rect.width) * f64::from(scale) - f64::from(*image_width)).abs() <= 1.0 / 1024.0
    );
    assert!(
        (f64::from(rect.height) * f64::from(scale) - f64::from(*image_height)).abs()
            <= 1.0 / 1024.0
    );
    let image_origin = ((16.0 + rect.x) * scale, (16.0 + rect.y) * scale);
    let DrawCommand::Text {
        text, color, fit, ..
    } = &commands[1]
    else {
        panic!("complete text must follow material");
    };
    assert_eq!(text, label.text());
    assert!(fit.is_none());
    assert_eq!(
        [color.r, color.g, color.b],
        paint
            .paint()
            .body()
            .foreground()
            .components()
            .map(|value| value as f32)
    );
    let width = ((label.size().width + 32.0) * f64::from(scale)).ceil() as u32;
    let height = ((label.size().height + 32.0) * f64::from(scale)).ceil() as u32;
    let combined = frames.render(&commands, width, height, scale, background);
    let body = frames.render(&commands[..1], width, height, scale, background);
    let mask = frames.render(&commands[1..], width, height, scale, Color::TRANSPARENT);
    for (index, source) in pixels.as_chunks::<4>().0.iter().enumerate() {
        if source[3] != 255 {
            continue;
        }
        let x = image_origin.0.round() as u32 + index as u32 % *image_width;
        let y = image_origin.1.round() as u32 + index as u32 / *image_width;
        let at = ((y * width + x) * 4) as usize;
        for channel in 0..4 {
            assert!(
                body[at + channel].abs_diff(source[channel]) <= 1,
                "{name}: prepared material was misplaced or changed"
            );
        }
    }
    let body_ir = paint.paint().body();
    let expected = body_ir
        .pigment()
        .body()
        .components()
        .map(|v| (v * 255.0).round() as u8);
    let line_height = label.typography().font_size() * label.typography().line_height();
    let lines = (label.label_bounds().height / line_height).round() as usize;
    let mut ink = vec![0; lines];
    let mut solids = 0;
    let mut checked_colors = HashSet::new();
    for (index, pixel) in mask.as_chunks::<4>().0.iter().enumerate() {
        let at = index * 4;
        let baseline = &body[at..at + 4];
        let actual = &combined[at..at + 4];
        if pixel[3] == 0 {
            for channel in 0..4 {
                assert!(
                    actual[channel].abs_diff(baseline[channel]) <= 1,
                    "{name}: material changed outside glyph coverage"
                );
            }
            continue;
        }
        let x = index as u32 % width;
        let y = index as u32 / width;
        for dx in [0.0, 1.0] {
            for dy in [0.0, 1.0] {
                let point = PhysicalVector {
                    x: (f64::from(x) + dx) / f64::from(scale) - 16.0,
                    y: (f64::from(y) + dy) / f64::from(scale) - 16.0,
                };
                assert!(
                    in_content(body_ir.geometry().content(), point),
                    "{name}: glyph footprint crossed content contour at {x},{y}"
                );
            }
        }
        assert_eq!(baseline[3], 255, "{name}: glyph background is translucent");
        for channel in 0..3 {
            assert!(
                baseline[channel].abs_diff(expected[channel]) <= 1,
                "{name}: glyph background is not uniform body pigment"
            );
        }
        assert_eq!(actual[3], 255);
        let local_y = (f64::from(y) + 0.5) / f64::from(scale) - 16.0;
        let line = ((local_y - label.label_bounds().y) / line_height)
            .floor()
            .clamp(0.0, (lines - 1) as f64) as usize;
        ink[line] += 1;
        if pixel[3] >= 32 {
            assert_ne!(actual, baseline, "{name}: covered glyph was not drawn");
        }
        if pixel[3] == 255 {
            solids += 1;
            let pair = (
                [actual[0], actual[1], actual[2]],
                [baseline[0], baseline[1], baseline[2]],
            );
            if checked_colors.insert(pair) {
                assert!(
                    opaque_contrast_ratio(&pixel_color(actual), &pixel_color(baseline)).unwrap()
                        >= 4.5,
                    "{name}: solid native glyph contrast is insufficient"
                );
            }
        }
    }
    assert!(solids > 8, "{name}: no solid native glyph coverage");
    assert!(
        ink.iter().all(|count| *count > 8),
        "{name}: a complete shaped line is missing"
    );
    if let Some(directory) = capture {
        std::fs::create_dir_all(directory).unwrap();
        readback::capture(
            &directory.join(format!("{name}-device-{scale}.ppm")),
            width,
            height,
            &combined,
        );
    }
}

#[test]
fn native_command_content_keeps_actual_ink_on_guarded_material() {
    guido::load_font(
        std::fs::read(std::env::var("RESINA_LABEL_FONT").expect("RESINA_LABEL_FONT is required"))
            .unwrap(),
    );
    let family = FontFamily::name("DejaVu Sans");
    let scenes: Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::env::var("RESINA_SCENES").expect("RESINA_SCENES is required"),
        )
        .unwrap(),
    )
    .unwrap();
    let scene = |name: &str| {
        scenes["scenarios"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"] == name)
            .unwrap()
    };
    let captures = std::env::var_os("RESINA_COMMAND_CAPTURE_DIR").map(std::path::PathBuf::from);
    let mut frames = NativeFrames::new(family);
    let mut count = 0;
    for text_scale in [1.0, 1.5, 2.0] {
        for case in label_cases::cases() {
            let mut request = command_request(
                scene("light-elastomer-paint-focused"),
                "rest",
                text_scale,
                case.direction,
            );
            let label = resolve_label(&mut request, &case.text, family, 24.0);
            let paint = resolve_command_paint_source(&request.to_string()).unwrap();
            for scale in [1.0, 1.25, 2.0, 3.0] {
                check_frame(
                    &mut frames,
                    &paint,
                    &label,
                    scale,
                    &format!("{}-text-{text_scale}", case.name),
                    captures.as_deref(),
                    background(&request),
                );
                count += 1;
            }
        }
    }
    for scheme in ["light", "dark"] {
        for material in ["cast", "frost", "elastomer"] {
            for phase in ["rest", "hover", "pressed", "disabled"] {
                let mut request = command_request(
                    scene(&format!("{scheme}-{material}-paint-focused")),
                    phase,
                    1.0,
                    LayoutDirection::Ltr,
                );
                let label = resolve_label(&mut request, "Save", family, 24.0);
                let paint = resolve_command_paint_source(&request.to_string()).unwrap();
                for scale in [1.0, 1.25, 2.0, 3.0] {
                    check_frame(
                        &mut frames,
                        &paint,
                        &label,
                        scale,
                        &format!("{scheme}-{material}-{phase}"),
                        captures.as_deref(),
                        background(&request),
                    );
                    count += 1;
                }
            }
        }
    }
    let mut rest = command_request(
        scene("light-elastomer-paint-focused"),
        "rest",
        1.0,
        LayoutDirection::Ltr,
    );
    rest["surface"]["body"]["surface"]["states"]["states"] = json!(["rest"]);
    let label = resolve_label(&mut rest, "Save", family, 24.0);
    let paint = resolve_command_paint_source(&rest.to_string()).unwrap();
    assert!(paint.paint().focus().is_none());
    for scale in [1.0, 1.25, 2.0, 3.0] {
        check_frame(
            &mut frames,
            &paint,
            &label,
            scale,
            "unfocused-rest",
            captures.as_deref(),
            background(&rest),
        );
        count += 1;
    }
    let mut request = command_request(
        scene("light-elastomer-paint-focused"),
        "pressed",
        1.0,
        LayoutDirection::Ltr,
    );
    request["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]["reducedMotion"] =
        json!(false);
    request["channels"] = serde_json::from_str::<Value>(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap()["channels"]
        .clone();
    let label = resolve_label(&mut request, "Save", family, 24.0);
    for time in [0.0, 0.016, 0.1, 1.0] {
        request["time"] = json!(time);
        let motion = resolve_command_motion_source(&request.to_string()).unwrap();
        assert_eq!(motion.policy(), CommandMotionPolicy::Spring);
        for scale in [1.0, 1.25, 2.0, 3.0] {
            check_frame(
                &mut frames,
                motion.command(),
                &label,
                scale,
                &format!("motion-{time}"),
                captures.as_deref(),
                background(&request),
            );
            count += 1;
        }
    }
    assert_eq!(count, 188);
    let mut request = command_request(
        scene("light-elastomer-paint-focused"),
        "rest",
        1.0,
        LayoutDirection::Ltr,
    );
    let label = resolve_label(&mut request, "Save", family, 24.0);
    let paint = resolve_command_paint_source(&request.to_string()).unwrap();
    assert!(matches!(
        prepare_command_content(&paint, &label, family, 0.0, 4),
        Err(CommandContentPrepareError::Paint(
            PrepareError::InvalidScale
        ))
    ));
    assert!(matches!(
        prepare_command_content(&paint, &label, family, 1.0, 0),
        Err(CommandContentPrepareError::Paint(PrepareError::Raster(_)))
    ));
    request["surface"]["body"]["size"]["width"] = json!(label.size().width + 1.0);
    let mismatch = resolve_command_paint_source(&request.to_string()).unwrap();
    let error = prepare_command_content(&mismatch, &label, family, 1.0, 4).unwrap_err();
    assert!(matches!(
        error,
        CommandContentPrepareError::Content(CommandContentError::SizeMismatch)
    ));
    assert!(std::error::Error::source(&error).is_some());
    let mut request = command_request(
        scene("light-elastomer-paint-focused"),
        "rest",
        1.0,
        LayoutDirection::Ltr,
    );
    let unsafe_label = resolve_label(&mut request, "Save", family, 0.0);
    let paint = resolve_command_paint_source(&request.to_string()).unwrap();
    assert!(matches!(
        prepare_command_content(&paint, &unsafe_label, family, 1.0, 4),
        Err(CommandContentPrepareError::Content(
            CommandContentError::OutsideContent
        ))
    ));
    let theme = &mut request["surface"]["body"]["theme"];
    let mut source: Value = serde_json::from_str(theme["themeSource"].as_str().unwrap()).unwrap();
    source["typographyAssignments"]["roles"]["label"]["letterSpacing"] =
        json!("type.tracking.wide");
    theme["themeSource"] = json!(source.to_string());
    let unsupported = resolve_theme_request_source(&theme.to_string())
        .unwrap()
        .typography()[&TypographyRole::Label]
        .clone();
    let label = resolve_command_label(
        CommandLabelInput {
            text: "Save",
            typography: &unsupported,
            minimum_size: unsafe_label.size(),
            maximum_size: unsafe_label.size(),
            padding: SafeArea {
                start: 24.0,
                end: 24.0,
                top: 12.0,
                bottom: 12.0,
            },
            direction: LayoutDirection::Ltr,
        },
        |_| {
            Ok::<_, std::convert::Infallible>(SurfaceSize {
                width: 20.0,
                height: 18.0,
            })
        },
    )
    .unwrap();
    assert!(matches!(
        prepare_command_content(&paint, &label, family, 1.0, 4),
        Err(CommandContentPrepareError::Label(
            LabelPrepareError::Measurement(_)
        ))
    ));
}
