//! Fixed-size native relief and complete-label inspection fixture.
use guido::{prelude::*, renderer::DrawCommand, widget_prelude::*};
use resina_environment::{LayoutDirection, SafeArea};
use resina_guido::{
    ReliefContentPrepareError, measure_command_label, prepare_material_relief_content_at,
    prepare_outline_focus_at, prepare_relief_content_at,
};
use resina_model::{PhysicalBounds, PhysicalVector, TypographyRole};
use resina_resolver::{
    CommandLabelInput, CommandLabelIr, MaterialReliefPaint, OpaquePigmentResult, ReliefPaint,
    SurfaceRelief, compile_theme_source, resolve_command_label,
    resolve_material_relief_paint_source, resolve_outline_focus_source,
    resolve_relief_paint_source, resolve_srgb_fallback,
};
use serde_json::{Value, json};
use std::error::Error;

struct Content(Vec<DrawCommand>);

enum Paint {
    Static(ReliefPaint),
    Material(MaterialReliefPaint),
}

impl Paint {
    fn surface(&self) -> &SurfaceRelief {
        match self {
            Self::Static(paint) => paint.surface(),
            Self::Material(paint) => paint.surface(),
        }
    }

    fn pigment(&self) -> &OpaquePigmentResult {
        match self {
            Self::Static(paint) => paint.pigment(),
            Self::Material(paint) => paint.pigment(),
        }
    }

    fn content_bounds(&self) -> PhysicalBounds {
        match self {
            Self::Static(paint) => paint.content_bounds(),
            Self::Material(paint) => paint.content_bounds(),
        }
    }

    fn prepare(
        &self,
        label: &CommandLabelIr,
        family: guido::widgets::font::FontFamily,
        origin: PhysicalVector,
        scale: f32,
    ) -> Result<[DrawCommand; 2], ReliefContentPrepareError> {
        match self {
            Self::Static(paint) => {
                prepare_relief_content_at(paint, label, family, origin, scale, 4)
            }
            Self::Material(paint) => {
                prepare_material_relief_content_at(paint, label, family, origin, scale, 4)
            }
        }
    }
}
impl Widget for Content {
    fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        constraints.constrain(Size::new(360.0, 200.0))
    }
    fn paint(&self, ctx: &mut PaintContext) {
        for command in &self.0 {
            match command {
                DrawCommand::Image {
                    source,
                    rect,
                    content_fit,
                    tint,
                    ..
                } => ctx.draw_image(source.clone(), *rect, *content_fit, *tint),
                DrawCommand::Text {
                    text,
                    rect,
                    color,
                    font_size,
                    font_family,
                    font_weight,
                    line_height,
                    letter_spacing,
                    align,
                    fit,
                } => ctx.draw_text_decorated(
                    text,
                    *rect,
                    *color,
                    *font_size,
                    *font_family,
                    *font_weight,
                    *line_height,
                    *letter_spacing,
                    *align,
                    None,
                    None,
                    *fit,
                ),
                _ => unreachable!(),
            }
        }
    }
}
fn config(canvas: Color) -> SurfaceConfig {
    SurfaceConfig::new()
        .width(360)
        .height(200)
        .anchor(Anchor::TOP | Anchor::LEFT)
        .background_color(canvas)
}
fn run() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let material_light = args.first().is_some_and(|arg| arg == "--material-light");
    if material_light {
        args.remove(0);
    }
    if args.len() != 5 && args.len() != 6 {
        return Err("usage: relief_content_probe [--material-light] <paint.json> <font.ttf> <device-scale> <text-scale> <expected.png> [focus.json]".into());
    }
    let font = std::fs::read(&args[1])?;
    let mut database = fontdb::Database::new();
    database.load_system_fonts();
    if database.faces().next().is_some() {
        return Err(
            "inspection requires isolated font discovery; use check_guido_relief_native.py".into(),
        );
    }
    database.load_font_data(font.clone());
    if !database.faces().any(|face| {
        face.families.iter().any(|(name, _)| name == "DejaVu Sans")
            && face.weight == fontdb::Weight::NORMAL
            && face.style == fontdb::Style::Normal
            && face.stretch == fontdb::Stretch::Normal
    }) {
        return Err("inspection font contains no regular normal-width DejaVu Sans face".into());
    }
    guido::load_font(font.clone());
    let scale: f32 = args[2].parse()?;
    let text_scale: f64 = args[3].parse()?;
    if !scale.is_finite()
        || !(1.0..=3.0).contains(&scale)
        || !text_scale.is_finite()
        || !(1.0..=2.0).contains(&text_scale)
    {
        return Err("inspection scales must be finite: device [1,3], text [1,2]".into());
    }
    let source = std::fs::read_to_string(&args[0])?;
    let request: Value = serde_json::from_str(&source)?;
    let paint = if material_light {
        Paint::Material(resolve_material_relief_paint_source(&source)?.paint)
    } else {
        Paint::Static(resolve_relief_paint_source(&source)?.paint)
    };
    let canvas = resolve_srgb_fallback(&request["edge"]["adjacentColor"])?;
    if canvas.alpha() != 1.0 {
        return Err("inspection canvas must be opaque".into());
    }
    let focus = if let Some(path) = args.get(5) {
        let source = std::fs::read_to_string(path)?;
        let request: Value = serde_json::from_str(&source)?;
        let result = resolve_outline_focus_source(&source)?;
        if result.focus().indicator().binding().material_family()
            != paint.pigment().material_family()
        {
            return Err("focus material family differs from the actual relief face".into());
        }
        if resolve_srgb_fallback(&request["focus"]["surroundingColor"])? != canvas {
            return Err("focus surrounding color differs from the actual canvas".into());
        }
        if result.focus().geometry().outline() != paint.surface().outline()
            || result.focus().geometry().side_offset() != (PhysicalVector { x: 0.0, y: 0.0 })
        {
            return Err("focus must match the actual unprojected relief footprint".into());
        }
        Some(result)
    } else {
        None
    };
    let [r, g, b] = canvas.components().map(|value| value as f32);
    let canvas = Color::rgb(r, g, b);
    let size = paint.surface().outline().size();
    if size.width != 320.0 || size.height != 160.0 {
        return Err("inspection face must be 320 by 160 px".into());
    }
    let theme_request: Value = serde_json::from_str(include_str!(
        "../../../conformance/ir/command-motion-request.json"
    ))?;
    let theme = &theme_request["surface"]["body"]["theme"];
    let mut environment = theme["environment"].clone();
    environment["textScale"] = json!(text_scale);
    let mut theme_source: Value = serde_json::from_str(
        theme["themeSource"]
            .as_str()
            .ok_or("theme source missing")?,
    )?;
    theme_source["tokens"]["type"]["weight"]["$value"] = json!(400);
    let resolved = compile_theme_source(&theme_source.to_string())?
        .resolve(&serde_json::from_value(environment)?)
        .map_err(|errors| format!("inspection theme: {errors:?}"))?;
    let direction: LayoutDirection =
        serde_json::from_value(request["outline"]["layoutDirection"].clone())?;
    let family = guido::widgets::font::FontFamily::name("DejaVu Sans");
    let label = resolve_command_label(
        CommandLabelInput {
            text: "j Reconnect now",
            typography: &resolved.typography()[&TypographyRole::Label],
            minimum_size: size,
            maximum_size: size,
            padding: SafeArea {
                start: 32.0,
                end: 40.0,
                top: 0.0,
                bottom: 0.0,
            },
            direction,
        },
        |input| measure_command_label(family, input),
    )?;
    let origin = PhysicalVector { x: 12.25, y: 7.125 };
    let mut commands = paint.prepare(&label, family, origin, scale)?.to_vec();
    if let Some(focus) = &focus {
        let prepared = prepare_outline_focus_at(focus.focus(), origin, scale)?;
        let (x, y) = prepared.origin();
        let size = prepared.logical_size();
        if x < 0.0 || y < 0.0 || x + size.width > 360.0 || y + size.height > 200.0 {
            return Err("inspection canvas clips complete outer focus bounds".into());
        }
        commands.insert(
            0,
            DrawCommand::Image {
                source: prepared.image_source(),
                decoded: None,
                rect: Rect::new(x, y, size.width, size.height),
                content_fit: ContentFit::Fill,
                tint: None,
            },
        );
    }
    let width = (360.0 * scale).round() as u32;
    let height = (200.0 * scale).round() as u32;
    {
        let mut app = guido::testing::Headless::new().ok_or("GPU capture is required")?;
        let expected_commands = commands.clone();
        let surface = app.surface(config(canvas), move || Content(expected_commands));
        app.configure(surface, 360, 200, scale);
        app.step();
        let pixels = app.read_frame(surface);
        if pixels.len() != (width * height * 4) as usize {
            return Err("unexpected GPU capture dimensions".into());
        }
        let file = std::fs::File::options()
            .write(true)
            .create_new(true)
            .open(&args[4])?;
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&pixels)?;
        writer.finish()?;
    }
    guido::load_font(font);
    println!(
        "RESINA_RELIEF_CONTENT_PREPARED {}",
        json!({"scale":scale,"textScale":text_scale,"width":width,"height":height,"labelBounds":label.label_bounds(),"contentBounds":paint.content_bounds(),"focusComposed":focus.is_some(),"materialLighting":material_light})
    );
    match App::new().run(move |app| {
        app.add_surface(config(canvas), move || Content(commands));
    }) {
        guido::ExitReason::Quit => Ok(()),
        reason => Err(format!("native content inspection exited: {reason:?}").into()),
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("native relief content inspection: {error}");
        std::process::exit(1);
    }
}
