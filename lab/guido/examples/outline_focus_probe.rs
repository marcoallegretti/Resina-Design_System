//! Fixed-size native continuous-outline focus inspection fixture.
use guido::{prelude::*, widget_prelude::*};
use resina_guido::{PreparedPaint, prepare_outline_focus_at};
use resina_model::PhysicalVector;
use resina_resolver::{resolve_outline_focus_source, resolve_srgb_fallback};
use serde_json::{Value, json};
use std::error::Error;

struct Focus(PreparedPaint);
impl Widget for Focus {
    fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        constraints.constrain(Size::new(160.0, 96.0))
    }
    fn paint(&self, ctx: &mut PaintContext) {
        let (x, y) = self.0.origin();
        let size = self.0.logical_size();
        ctx.draw_image(
            self.0.image_source(),
            Rect::new(x, y, size.width, size.height),
            ContentFit::Fill,
            None,
        );
    }
}
fn config(canvas: Color) -> SurfaceConfig {
    SurfaceConfig::new()
        .width(160)
        .height(96)
        .anchor(Anchor::TOP | Anchor::LEFT)
        .background_color(canvas)
}
fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: outline_focus_probe <focus.json> <device-scale> <expected.png>".into());
    }
    let source = std::fs::read_to_string(&args[0])?;
    let request: Value = serde_json::from_str(&source)?;
    let result = resolve_outline_focus_source(&source)?;
    let scale: f32 = args[1].parse()?;
    if !scale.is_finite()
        || !(1.0..=3.0).contains(&scale)
        || (160.0 * scale).fract() != 0.0
        || (96.0 * scale).fract() != 0.0
    {
        return Err(
            "inspection scale must be in [1,3] and map the 160x96 canvas to integral pixels".into(),
        );
    }
    let canvas = resolve_srgb_fallback(&request["focus"]["surroundingColor"])?;
    if canvas.alpha() != 1.0 {
        return Err("inspection canvas must be opaque".into());
    }
    let [r, g, b] = canvas.components().map(|value| value as f32);
    let canvas = Color::rgb(r, g, b);
    let prepared =
        prepare_outline_focus_at(result.focus(), PhysicalVector { x: 12.25, y: 12.25 }, scale)?;
    let (x, y) = prepared.origin();
    let size = prepared.logical_size();
    if x < 0.0 || y < 0.0 || x + size.width > 160.0 || y + size.height > 96.0 {
        return Err("inspection canvas clips complete outer focus bounds".into());
    }
    let width = (160.0 * scale).round() as u32;
    let height = (96.0 * scale).round() as u32;
    {
        let mut app = guido::testing::Headless::new().ok_or("focus GPU reference is required")?;
        let reference = prepared.clone();
        let surface = app.surface(config(canvas), move || Focus(reference));
        app.configure(surface, 160, 96, scale);
        app.step();
        let pixels = app.read_frame(surface);
        if pixels.len() != (width * height * 4) as usize {
            return Err("unexpected GPU capture dimensions".into());
        }
        let file = std::fs::File::options()
            .write(true)
            .create_new(true)
            .open(&args[2])?;
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&pixels)?;
        writer.finish()?;
    }
    println!(
        "RESINA_OUTLINE_FOCUS_PREPARED {}",
        json!({"scale":scale,"width":width,"height":height})
    );
    match App::new().run(move |app| {
        app.add_surface(config(canvas), move || Focus(prepared));
    }) {
        guido::ExitReason::Quit => Ok(()),
        reason => Err(format!("native focus inspection exited: {reason:?}").into()),
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("native focus inspection: {error}");
        std::process::exit(1);
    }
}
