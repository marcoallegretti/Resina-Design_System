#![cfg(feature = "testing")]

#[path = "common/readback.rs"]
mod readback;

use guido::{
    renderer::{
        DrawCommand, FlattenScratch, GpuContext, LineFit, RenderNode, RenderTarget, Renderer,
        flatten_root_into,
    },
    transform::Transform,
    widgets::{Color, ContentFit, TextOverflow, font::FontFamily},
};
use resina_environment::{EnvironmentSnapshot, LayoutDirection, SafeArea};
use resina_guido::{
    LabelPrepareError, PrepareError, ToggleContentPrepareError, measure_command_label,
    prepare_toggle_content, prepare_toggle_travel_content,
};
use resina_model::{
    ActivationState, ColorRole, PhysicalBounds, PhysicalVector, PressHold, SpringDynamics,
    SpringState, SurfaceSize, TypographyRole,
};
use resina_raster::{Viewport, render_surface_paint};
use resina_resolver::{
    CommandLabelInput, CommandLabelIr, HitRegionInput, HitRegionIr, SrgbFallback,
    ToggleLayoutInput, ToggleLayoutIr, TogglePartMotionIr, TogglePartPaintError, TogglePartPaintIr,
    ToggleSnapshot, ToggleSnapshotInput, opaque_contrast_ratio, resolve_command_label,
    resolve_hit_region, resolve_srgb_fallback, resolve_theme_request_source, resolve_toggle_layout,
    resolve_toggle_part_motion_source, resolve_toggle_part_paint_source, resolve_toggle_snapshot,
    resolve_toggle_travel,
};
use serde_json::{Value, json};
use std::{path::Path, rc::Rc};

const UNWRAPPED: LineFit = LineFit {
    width: None,
    max_lines: None,
    overflow: TextOverflow::Clip,
    wrap: false,
};
fn size(width: f64, height: f64) -> SurfaceSize {
    SurfaceSize { width, height }
}
fn bounds(x: f64, y: f64, width: f64, height: f64) -> PhysicalBounds {
    PhysicalBounds {
        x,
        y,
        width,
        height,
    }
}
fn available() -> PhysicalBounds {
    bounds(-512.0, -256.0, 1024.0, 512.0)
}
fn padding() -> SafeArea {
    SafeArea {
        start: 8.0,
        end: 8.0,
        top: 8.0,
        bottom: 8.0,
    }
}

struct Fixture {
    environment: EnvironmentSnapshot,
    track_request: Value,
    thumb_request: Value,
    label: CommandLabelIr,
    foreground: SrgbFallback,
    background: SrgbFallback,
    origin: PhysicalVector,
    layout: ToggleLayoutIr,
    track: TogglePartPaintIr,
    thumb: TogglePartPaintIr,
    target: HitRegionIr,
    activation: ActivationState,
    hovered: bool,
    checked: bool,
}
impl Fixture {
    fn new(
        scene: &Value,
        phase: &str,
        direction: LayoutDirection,
        checked: bool,
        family: FontFamily,
        text_scale: f64,
        text: &str,
    ) -> Self {
        let mut surface = scene["request"].clone();
        let mut source: Value =
            serde_json::from_str(surface["body"]["theme"]["themeSource"].as_str().unwrap())
                .unwrap();
        source["materialAssignments"]["control"]["interactive"] =
            scene["expectedMaterialFamily"].clone();
        surface["body"]["theme"]["themeSource"] = json!(source.to_string());
        surface["body"]["theme"]["environment"]["layoutDirection"] =
            serde_json::to_value(direction).unwrap();
        surface["body"]["theme"]["environment"]["locale"] =
            json!(if direction == LayoutDirection::Rtl {
                "ar"
            } else {
                "en"
            });
        surface["body"]["theme"]["environment"]["textScale"] = json!(text_scale);
        surface["body"]["theme"]["environment"]["accessibilityPreferences"]["reducedMotion"] =
            json!(false);
        let focused = phase != "disabled" && phase != "unfocused";
        let mut states = vec![if phase == "unfocused" { "rest" } else { phase }];
        if focused {
            states.push("focused");
        }
        if checked {
            states.push("checked");
        }
        surface["body"]["surface"]["materialRole"] = json!("control.interactive");
        surface["body"]["surface"]["colorRole"] = json!("surface.high");
        surface["body"]["surface"]["form"]["shape"] = json!("rounded");
        surface["body"]["surface"]["states"]["states"] = json!(states);
        surface["body"]["size"] = json!({"width":64.5,"height":40.5});
        surface["body"]["foregroundRole"] = json!("content.primary");
        let appearance: Value =
            serde_json::from_str(if scene["name"].as_str().unwrap().starts_with("dark-") {
                include_str!("../../../../definitions/command-appearance-dark.json")
            } else {
                include_str!("../../../../definitions/command-appearance-light.json")
            })
            .unwrap();
        let mut request = json!({"schemaVersion":"0.1.0","part":"track","surface":surface,
            "checkedColorRole":"surface.high","interactionAppearance":appearance});
        let track_request = request.clone();
        let track = resolve_toggle_part_paint_source(&request.to_string()).unwrap();
        assert_eq!(track.paint().focus().is_some(), focused);
        request["part"] = json!("thumb");
        request["surface"]["body"]["size"] = json!({"width":16.0,"height":16.0});
        request["surface"]["body"]["surface"]["colorRole"] = json!("content.primary");
        request["checkedColorRole"] = json!("content.primary");
        request["surface"]["body"]["foregroundRole"] = json!("content.inverse");
        request["surface"]["body"]["adjacentColor"] =
            serde_json::to_value(track.paint().body().pigment().body()).unwrap();
        request["surface"]["body"]["postTreatmentBackdrop"] =
            request["surface"]["body"]["adjacentColor"].clone();
        let thumb_request = request.clone();
        let thumb = resolve_toggle_part_paint_source(&request.to_string()).unwrap();
        assert!(thumb.paint().focus().is_none());
        let theme = &request["surface"]["body"]["theme"];
        let environment: EnvironmentSnapshot =
            serde_json::from_value(theme["environment"].clone()).unwrap();
        let resolved = resolve_theme_request_source(&theme.to_string()).unwrap();
        let label = resolve_command_label(
            CommandLabelInput {
                text,
                typography: &resolved.typography()[&TypographyRole::Label],
                minimum_size: size(180.0, 40.5),
                maximum_size: size(180.0, 360.0),
                padding: padding(),
                direction,
            },
            |input| measure_command_label(family, input),
        )
        .unwrap();
        let origin = PhysicalVector {
            x: if direction == LayoutDirection::Ltr {
                80.5
            } else {
                -196.0
            },
            y: (40.5 - label.size().height) / 2.0,
        };
        let layout = resolve_toggle_layout(ToggleLayoutInput {
            track_size: size(64.5, 40.5),
            thumb_size: size(16.0, 16.0),
            insets: &SafeArea {
                start: 12.25,
                end: 12.25,
                top: 12.25,
                bottom: 12.25,
            },
            layout_direction: direction,
            checked,
        })
        .unwrap();
        let x = if direction == LayoutDirection::Ltr {
            -20.0
        } else {
            origin.x - 20.0
        };
        let y = origin.y.min(-20.0);
        let right = if direction == LayoutDirection::Ltr {
            origin.x + label.size().width + 20.0
        } else {
            84.5
        };
        let bottom = (origin.y + label.size().height + 20.0).max(60.5);
        let target = resolve_hit_region(HitRegionInput {
            environment: &environment,
            visual_bounds: bounds(x, y, right - x, bottom - y),
            available_bounds: available(),
            component_minimum: size(48.0, 48.0),
            occupied_regions: &[],
        })
        .unwrap();
        let activation = ActivationState::try_new(
            phase != "disabled",
            focused,
            if phase == "pressed" {
                Some(PressHold::Pointer {
                    id: "primary".into(),
                    inside: true,
                })
            } else {
                None
            },
        )
        .unwrap();
        Self {
            environment,
            track_request,
            thumb_request,
            label,
            foreground: resolved.color_fallbacks()[&ColorRole::ContentSecondary].clone(),
            background: resolve_srgb_fallback(&request["surface"]["surroundingColor"]).unwrap(),
            origin,
            layout,
            track,
            thumb,
            target,
            activation,
            hovered: phase == "hover",
            checked,
        }
    }
    fn sample_parts(
        &mut self,
        channels: &[Value; 2],
        time: f64,
        reduced_motion: bool,
    ) -> Result<[TogglePartMotionIr; 2], TogglePartPaintError> {
        let mut track_request = self.track_request.clone();
        track_request["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]["reducedMotion"] =
            json!(reduced_motion);
        track_request["channels"] = channels[0].clone();
        track_request["time"] = json!(time);
        let track = resolve_toggle_part_motion_source(&track_request.to_string())?;
        let mut thumb_request = self.thumb_request.clone();
        thumb_request["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]["reducedMotion"] =
            json!(reduced_motion);
        thumb_request["channels"] = channels[1].clone();
        thumb_request["time"] = json!(time);
        thumb_request["surface"]["body"]["adjacentColor"] =
            serde_json::to_value(track.part_paint().paint().body().pigment().body()).unwrap();
        thumb_request["surface"]["body"]["postTreatmentBackdrop"] =
            thumb_request["surface"]["body"]["adjacentColor"].clone();
        let thumb = resolve_toggle_part_motion_source(&thumb_request.to_string())?;
        self.track = track.part_paint().clone();
        self.thumb = thumb.part_paint().clone();
        self.environment = serde_json::from_value(
            track_request["surface"]["body"]["theme"]["environment"].clone(),
        )
        .unwrap();
        Ok([track, thumb])
    }
    fn snapshot(&self) -> ToggleSnapshot<'_> {
        resolve_toggle_snapshot(self.snapshot_input()).unwrap()
    }
    fn snapshot_input(&self) -> ToggleSnapshotInput<'_, '_> {
        ToggleSnapshotInput {
            label: &self.label,
            label_origin: self.origin,
            label_foreground: &self.foreground,
            label_background: &self.background,
            minimum_label_contrast: 4.5,
            layout: &self.layout,
            track: &self.track,
            thumb: &self.thumb,
            hit_region: self.target,
            activation: &self.activation,
            hovered: self.hovered,
            checked: self.checked,
            description: None,
            focusable: true,
            minimum_thumb_contrast: 3.0,
            environment: &self.environment,
            available_bounds: available(),
            component_minimum: size(48.0, 48.0),
            occupied_regions: &[],
        }
    }
}
struct Frames {
    target: RenderTarget,
    renderer: Renderer,
}
impl Frames {
    fn new() -> Self {
        let gpu = GpuContext::try_new().expect("native Toggle GPU rendering is required");
        let target = RenderTarget::offscreen(&gpu, 520, 440);
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
        let (width, height) = ((520.0 * scale) as u32, (440.0 * scale) as u32);
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
fn check_frame(
    frames: &mut Frames,
    f: &Fixture,
    family: FontFamily,
    scale: f32,
    name: &str,
    capture: Option<&Path>,
) {
    let snapshot = f.snapshot();
    let commands = prepare_toggle_content(&snapshot, family, scale, 4).unwrap();
    check_commands(
        frames,
        f,
        scale,
        name,
        capture,
        &commands,
        f.layout.thumb_bounds(),
    );
}
fn check_commands(
    frames: &mut Frames,
    f: &Fixture,
    scale: f32,
    name: &str,
    capture: Option<&Path>,
    commands: &[DrawCommand; 3],
    thumb: PhysicalBounds,
) {
    let output_width = (520.0 * scale) as u32;
    for (index, paint, offset) in [
        (0, f.track.paint(), PhysicalVector { x: 0.0, y: 0.0 }),
        (
            1,
            f.thumb.paint(),
            PhysicalVector {
                x: thumb.x,
                y: thumb.y,
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
        let actual = frames.render(&commands[index..index + 1], scale, color(&f.background));
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
    assert!((f64::from(rect.x) - f.origin.x - label.x).abs() <= 1.0 / 1024.0);
    assert!((f64::from(rect.y) - f.origin.y - label.y).abs() <= 1.0 / 1024.0);
    assert_ne!(
        f.foreground.components(),
        f.track.paint().body().foreground().components()
    );
    let expected = color(&f.foreground);
    assert_eq!(
        [foreground.r, foreground.g, foreground.b, foreground.a],
        [expected.r, expected.g, expected.b, expected.a]
    );
    let background = color(&f.background);
    let backdrop_bytes = f.background.components().map(|v| (v * 255.0).round() as u8);
    let combined = frames.render(commands, scale, background);
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
            x >= f.origin.x
                && x + 1.0 / f64::from(scale) <= f.origin.x + f.label.size().width
                && y >= f.origin.y
                && y + 1.0 / f64::from(scale) <= f.origin.y + f.label.size().height,
            "{name}: native glyph escaped reserved label slot"
        );
        let line = ((y - f.origin.y - label.y) / line_height)
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
            (440.0 * scale) as u32,
            &combined,
        );
    }
}
#[test]
fn checked_toggle_content_preserves_placed_paint_and_complete_native_labels() {
    guido::load_font(
        std::fs::read(std::env::var("RESINA_LABEL_FONT").expect("RESINA_LABEL_FONT required"))
            .unwrap(),
    );
    let family = FontFamily::name("DejaVu Sans");
    let scenes: Value = serde_json::from_str(
        &std::fs::read_to_string(std::env::var("RESINA_SCENES").expect("RESINA_SCENES required"))
            .unwrap(),
    )
    .unwrap();
    let capture = std::env::var_os("RESINA_TOGGLE_CAPTURE_DIR").map(std::path::PathBuf::from);
    let mut frames = Frames::new();
    let mut count = 0;
    for scheme in ["light", "dark"] {
        for material in ["cast", "frost", "elastomer"] {
            let name = format!("{scheme}-{material}-paint-focused");
            let scene = scenes["scenarios"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["name"] == name)
                .unwrap();
            for phase in ["rest", "hover", "pressed", "disabled", "unfocused"] {
                for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
                    for checked in [false, true] {
                        let text = if direction == LayoutDirection::Ltr {
                            "Automatic updates"
                        } else {
                            "التحديثات التلقائية"
                        };
                        let f = Fixture::new(scene, phase, direction, checked, family, 1.0, text);
                        for scale in [1.0, 1.25, 2.0] {
                            check_frame(
                                &mut frames,
                                &f,
                                family,
                                scale,
                                &format!("{name}-{phase}-{direction:?}-{checked}"),
                                capture.as_deref(),
                            );
                            count += 1;
                        }
                    }
                }
            }
            for (direction, text) in [
                (
                    LayoutDirection::Ltr,
                    "Install updates automatically when connected to power",
                ),
                (
                    LayoutDirection::Rtl,
                    "تثبيت التحديثات تلقائيًا عند الاتصال بمصدر الطاقة",
                ),
            ] {
                let f = Fixture::new(scene, "rest", direction, true, family, 2.0, text);
                for scale in [1.0, 1.25, 2.0] {
                    check_frame(
                        &mut frames,
                        &f,
                        family,
                        scale,
                        &format!("{name}-scaled-wrapped-{direction:?}"),
                        capture.as_deref(),
                    );
                    count += 1;
                }
            }
            if material == "elastomer" {
                let dynamics = SpringDynamics::try_new(1.0, 100.0, 20.0, 0.0001, 0.0001).unwrap();
                for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
                    for checked in [false, true] {
                        let text = if direction == LayoutDirection::Ltr {
                            "Automatic updates"
                        } else {
                            "التحديثات التلقائية"
                        };
                        let f = Fixture::new(scene, "rest", direction, checked, family, 1.0, text);
                        let snapshot = f.snapshot();
                        let initial =
                            SpringState::try_new(if checked { 0.0 } else { 1.0 }, 0.0).unwrap();
                        for time in [0.0, 0.1, 0.25, 100.0] {
                            let travel =
                                resolve_toggle_travel(&snapshot, &dynamics, initial, time).unwrap();
                            assert_eq!(
                                travel.policy(),
                                resina_resolver::ToggleTravelPolicy::Spring
                            );
                            if time == 0.0 {
                                assert_eq!(
                                    travel.thumb_bounds(),
                                    if checked {
                                        f.layout.off_thumb_bounds()
                                    } else {
                                        f.layout.on_thumb_bounds()
                                    }
                                );
                            } else if time == 100.0 {
                                assert_eq!(travel.thumb_bounds(), f.layout.thumb_bounds());
                            } else {
                                assert_ne!(travel.thumb_bounds(), f.layout.off_thumb_bounds());
                                assert_ne!(travel.thumb_bounds(), f.layout.on_thumb_bounds());
                            }
                            let commands =
                                prepare_toggle_travel_content(&travel, family, 1.25, 4).unwrap();
                            check_commands(
                                &mut frames,
                                &f,
                                1.25,
                                &format!("{name}-travel-{direction:?}-{checked}-{time}"),
                                capture.as_deref(),
                                &commands,
                                travel.thumb_bounds(),
                            );
                        }
                    }
                }
            }
            let mut f = Fixture::new(
                scene,
                "rest",
                LayoutDirection::Rtl,
                true,
                family,
                1.0,
                "التحديثات التلقائية",
            );
            let snapshot = f.snapshot();
            assert!(matches!(
                prepare_toggle_content(&snapshot, family, 0.0, 4),
                Err(ToggleContentPrepareError::Track(PrepareError::InvalidScale))
            ));
            assert!(matches!(
                prepare_toggle_content(&snapshot, family, 1.0, 0),
                Err(ToggleContentPrepareError::Track(PrepareError::Raster(_)))
            ));
            f.label = resolve_command_label(
                CommandLabelInput {
                    text: f.label.text(),
                    typography: f.label.typography(),
                    minimum_size: size(180.0, 40.5),
                    maximum_size: size(180.0, 360.0),
                    padding: padding(),
                    direction: LayoutDirection::Rtl,
                },
                |_| Ok::<_, std::convert::Infallible>(size(20.0, 18.0)),
            )
            .unwrap();
            let Err(error) = prepare_toggle_content(&f.snapshot(), family, 1.0, 4) else {
                panic!("approximate label measurement must fail native publication");
            };
            assert!(matches!(
                error,
                ToggleContentPrepareError::Label(LabelPrepareError::MeasurementMismatch)
            ));
            assert!(std::error::Error::source(&error).is_some());
        }
    }
    assert_eq!(count, 396);
}

fn motion_channels() -> [Value; 2] {
    let template: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/toggle-part-motion-request.json"
    ))
    .unwrap();
    let mut channels = [template["channels"].clone(), template["channels"].clone()];
    for (part, stiffness, damping) in [(0, 100, 20), (1, 64, 16)] {
        for channel in ["bodyMix", "depthScale"] {
            channels[part][channel]["dynamics"]["stiffness"] = json!(stiffness);
            channels[part][channel]["dynamics"]["damping"] = json!(damping);
        }
    }
    channels
}
fn continue_channels(parts: &[TogglePartMotionIr; 2]) -> [Value; 2] {
    let mut channels = motion_channels();
    for (index, part) in parts.iter().enumerate() {
        channels[index]["bodyMix"]["initial"] =
            serde_json::to_value(part.body_mix().state()).unwrap();
        channels[index]["depthScale"]["initial"] =
            serde_json::to_value(part.depth_scale().state()).unwrap();
    }
    channels
}
#[test]
fn sampled_toggle_frames_keep_current_adjacency_semantics_and_raw_retarget_state() {
    guido::load_font(
        std::fs::read(std::env::var("RESINA_LABEL_FONT").expect("RESINA_LABEL_FONT required"))
            .unwrap(),
    );
    let family = FontFamily::name("DejaVu Sans");
    let scenes: Value = serde_json::from_str(
        &std::fs::read_to_string(std::env::var("RESINA_SCENES").expect("RESINA_SCENES required"))
            .unwrap(),
    )
    .unwrap();
    let capture = std::env::var_os("RESINA_TOGGLE_CAPTURE_DIR").map(std::path::PathBuf::from);
    let dynamics = SpringDynamics::try_new(1.0, 100.0, 20.0, 0.0001, 0.0001).unwrap();
    let mut frames = Frames::new();
    let mut count = 0;
    for scheme in ["light", "dark"] {
        for material in ["cast", "frost", "elastomer"] {
            let name = format!("{scheme}-{material}-paint-focused");
            let scene = scenes["scenarios"]
                .as_array()
                .unwrap()
                .iter()
                .find(|scene| scene["name"] == name)
                .unwrap();
            for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
                for checked in [false, true] {
                    let text = if direction == LayoutDirection::Ltr {
                        "Automatic updates"
                    } else {
                        "التحديثات التلقائية"
                    };
                    let mut f =
                        Fixture::new(scene, "pressed", direction, checked, family, 1.0, text);
                    let reserved = f.target.bounds();
                    let initial =
                        SpringState::try_new(if checked { 0.0 } else { 1.0 }, 0.0).unwrap();
                    let channels = motion_channels();
                    for time in [0.0, 0.1, 100.0] {
                        let parts = f.sample_parts(&channels, time, false).unwrap();
                        if material != "cast" && time == 0.1 {
                            let mut stale_request = f.thumb_request.clone();
                            stale_request["channels"] = channels[1].clone();
                            stale_request["time"] = json!(time);
                            let stale =
                                resolve_toggle_part_motion_source(&stale_request.to_string())
                                    .unwrap();
                            f.thumb = stale.part_paint().clone();
                            assert!(matches!(
                                resolve_toggle_snapshot(f.snapshot_input()),
                                Err(
                                    resina_resolver::ToggleSnapshotError::ThumbAdjacencyMismatch { .. }
                                )
                            ));
                            f.thumb = parts[1].part_paint().clone();
                        }
                        let snapshot = f.snapshot();
                        let travel =
                            resolve_toggle_travel(&snapshot, &dynamics, initial, time).unwrap();
                        assert_eq!(
                            travel.policy(),
                            if material == "cast" {
                                resina_resolver::ToggleTravelPolicy::CastImmediate
                            } else {
                                resina_resolver::ToggleTravelPolicy::Spring
                            }
                        );
                        if material == "cast" || time == 100.0 {
                            assert_eq!(travel.thumb_bounds(), f.layout.thumb_bounds());
                            assert!(travel.trajectory().settled());
                            for part in &parts {
                                assert!(part.body_mix().settled());
                                assert!(part.depth_scale().settled());
                                assert_eq!(part.part_paint().response(), part.target());
                            }
                        } else if time == 0.0 {
                            assert_eq!(
                                travel.thumb_bounds(),
                                if checked {
                                    f.layout.off_thumb_bounds()
                                } else {
                                    f.layout.on_thumb_bounds()
                                }
                            );
                        }
                        assert_eq!(snapshot.hit_region().bounds(), reserved);
                        assert_eq!(snapshot.accessibility().name(), text);
                        assert_eq!(snapshot.accessibility().state().checked(), checked);
                        assert!(snapshot.accessibility().state().enabled());
                        assert!(snapshot.accessibility().state().focused());
                        assert!(snapshot.accessibility().actions()[0].available());
                        for part in &parts {
                            assert_eq!(
                                part.policy(),
                                if material == "cast" {
                                    resina_resolver::CommandMotionPolicy::CastImmediate
                                } else {
                                    resina_resolver::CommandMotionPolicy::Spring
                                }
                            );
                            if material != "cast" && time == 0.1 {
                                assert_ne!(part.part_paint().response(), part.target());
                                assert_ne!(part.part_paint().response().depth_scale(), 1.0);
                                assert_ne!(travel.thumb_bounds(), f.layout.off_thumb_bounds());
                                assert_ne!(travel.thumb_bounds(), f.layout.on_thumb_bounds());
                            }
                        }
                        let commands =
                            prepare_toggle_travel_content(&travel, family, 1.25, 4).unwrap();
                        check_commands(
                            &mut frames,
                            &f,
                            1.25,
                            &format!("{name}-joint-pressed-{direction:?}-{checked}-{time}"),
                            capture.as_deref(),
                            &commands,
                            travel.thumb_bounds(),
                        );
                        count += 1;
                    }
                    let parts = f.sample_parts(&channels, 0.1, false).unwrap();
                    let snapshot = f.snapshot();
                    let before = resolve_toggle_travel(&snapshot, &dynamics, initial, 0.1).unwrap();
                    let retained = before.trajectory().state();
                    let retained_bounds = before.thumb_bounds();
                    let continued = continue_channels(&parts);
                    let mut rest =
                        Fixture::new(scene, "rest", direction, !checked, family, 1.0, text);
                    assert_eq!(rest.target.bounds(), reserved);
                    for time in [0.0, 0.1] {
                        let retargeted = rest.sample_parts(&continued, time, false).unwrap();
                        for part in &retargeted {
                            assert_eq!(
                                part.target(),
                                resina_model::CommandResponse::try_new(0.0, 1.0).unwrap()
                            );
                        }
                        let snapshot = rest.snapshot();
                        let travel =
                            resolve_toggle_travel(&snapshot, &dynamics, retained, time).unwrap();
                        assert_eq!(snapshot.accessibility().state().checked(), !checked);
                        assert_eq!(snapshot.hit_region().bounds(), reserved);
                        if material != "cast" && time == 0.0 {
                            assert_eq!(travel.trajectory().state(), retained);
                            assert_eq!(travel.thumb_bounds(), retained_bounds);
                            for (old, new) in parts.iter().zip(&retargeted) {
                                assert_eq!(old.body_mix().state(), new.body_mix().state());
                                assert_eq!(old.depth_scale().state(), new.depth_scale().state());
                                assert_eq!(
                                    old.part_paint().paint().body().geometry(),
                                    new.part_paint().paint().body().geometry()
                                );
                            }
                        }
                        let commands =
                            prepare_toggle_travel_content(&travel, family, 1.25, 4).unwrap();
                        check_commands(
                            &mut frames,
                            &rest,
                            1.25,
                            &format!("{name}-joint-retarget-{direction:?}-{checked}-{time}"),
                            capture.as_deref(),
                            &commands,
                            travel.thumb_bounds(),
                        );
                        count += 1;
                    }
                    let mut disabled =
                        Fixture::new(scene, "disabled", direction, checked, family, 1.0, text);
                    disabled.sample_parts(&continued, 0.1, false).unwrap();
                    let snapshot = disabled.snapshot();
                    let travel =
                        resolve_toggle_travel(&snapshot, &dynamics, retained, 0.1).unwrap();
                    assert_eq!(snapshot.hit_region().bounds(), reserved);
                    assert!(!snapshot.accessibility().state().enabled());
                    assert!(!snapshot.accessibility().state().focused());
                    assert!(!snapshot.accessibility().actions()[0].available());
                    assert!(snapshot.track().paint().focus().is_none());
                    let commands = prepare_toggle_travel_content(&travel, family, 1.25, 4).unwrap();
                    check_commands(
                        &mut frames,
                        &disabled,
                        1.25,
                        &format!("{name}-joint-disabled-{direction:?}-{checked}"),
                        capture.as_deref(),
                        &commands,
                        travel.thumb_bounds(),
                    );
                    count += 1;
                    let immediate = f.sample_parts(&channels, 0.0, true).unwrap();
                    for part in &immediate {
                        assert_eq!(
                            part.policy(),
                            resina_resolver::CommandMotionPolicy::ReducedMotion
                        );
                        assert!(part.body_mix().settled());
                        assert!(part.depth_scale().settled());
                        assert_eq!(part.body_mix().state().velocity(), 0.0);
                        assert_eq!(part.depth_scale().state().velocity(), 0.0);
                        assert_eq!(part.part_paint().response(), part.target());
                    }
                    let snapshot = f.snapshot();
                    let travel = resolve_toggle_travel(&snapshot, &dynamics, initial, 0.0).unwrap();
                    assert_eq!(
                        travel.policy(),
                        resina_resolver::ToggleTravelPolicy::ReducedMotion
                    );
                    assert_eq!(travel.thumb_bounds(), f.layout.thumb_bounds());
                    assert_eq!(snapshot.hit_region().bounds(), reserved);
                    let commands = prepare_toggle_travel_content(&travel, family, 1.25, 4).unwrap();
                    check_commands(
                        &mut frames,
                        &f,
                        1.25,
                        &format!("{name}-joint-reduced-{direction:?}-{checked}"),
                        capture.as_deref(),
                        &commands,
                        travel.thumb_bounds(),
                    );
                    count += 1;
                }
                let text = if direction == LayoutDirection::Ltr {
                    "Install updates automatically when connected to power"
                } else {
                    "تثبيت التحديثات تلقائيًا عند الاتصال بمصدر الطاقة"
                };
                let mut f = Fixture::new(scene, "pressed", direction, true, family, 2.0, text);
                f.sample_parts(&motion_channels(), 0.1, false).unwrap();
                let snapshot = f.snapshot();
                let travel = resolve_toggle_travel(
                    &snapshot,
                    &dynamics,
                    SpringState::try_new(0.0, 0.0).unwrap(),
                    0.1,
                )
                .unwrap();
                let commands = prepare_toggle_travel_content(&travel, family, 1.25, 4).unwrap();
                check_commands(
                    &mut frames,
                    &f,
                    1.25,
                    &format!("{name}-joint-scaled-{direction:?}"),
                    capture.as_deref(),
                    &commands,
                    travel.thumb_bounds(),
                );
                count += 1;
            }
        }
    }
    assert_eq!(count, 180);
}
