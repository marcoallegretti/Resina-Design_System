//! Fixed-size native face-paint inspection fixture.
use guido::{prelude::*, widget_prelude::*};
use resina_guido::{PreparedPaint, prepare_relief_paint_at};
use resina_model::PhysicalVector;
use resina_resolver::{resolve_relief_paint_source, resolve_srgb_fallback};
use serde_json::{Value, json};
use std::error::Error;

struct Face(PreparedPaint);
impl Widget for Face {
    fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        constraints.constrain(Size::new(160.0, 96.0))
    }
    fn paint(&self, ctx: &mut PaintContext) {
        let origin = self.0.origin();
        let size = self.0.logical_size();
        ctx.draw_image(
            self.0.image_source(),
            Rect::new(origin.0, origin.1, size.width, size.height),
            ContentFit::Fill,
            None,
        );
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: relief_probe <request.json> <device-scale> <prepared.png>".into());
    }
    let source = std::fs::read_to_string(&args[0])?;
    let request: Value = serde_json::from_str(&source)?;
    let paint = resolve_relief_paint_source(&source)?.paint;
    let size = paint.surface().outline().size();
    if size.width > 120.0 || size.height > 60.0 {
        return Err("native inspection fixture requires a face at most 120 by 60 px".into());
    }
    let scale: f32 = args[1].parse()?;
    let prepared =
        prepare_relief_paint_at(&paint, PhysicalVector { x: 12.25, y: 12.25 }, scale, 4)?;
    let ImageSource::Rgba {
        width,
        height,
        pixels,
    } = prepared.image_source()
    else {
        unreachable!()
    };
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
    let canvas = resolve_srgb_fallback(&request["edge"]["adjacentColor"])?;
    if canvas.alpha() != 1.0 {
        return Err("canvas must be opaque".into());
    }
    let [r, g, b] = canvas.components().map(|value| value as f32);
    let origin = prepared.origin();
    println!(
        "RESINA_RELIEF_PREPARED {}",
        json!({"scale":scale,"origin":{"x":origin.0,"y":origin.1},
        "width":width,"height":height,"canvas":canvas,"contentBounds":paint.content_bounds()})
    );
    let reason = App::new().run(move |app| {
        app.add_surface(
            SurfaceConfig::new()
                .width(160)
                .height(96)
                .anchor(Anchor::TOP | Anchor::LEFT)
                .background_color(Color::rgb(r, g, b)),
            move || Face(prepared),
        );
    });
    match reason {
        guido::ExitReason::Quit => Ok(()),
        reason => Err(format!("native relief inspection exited: {reason:?}").into()),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("native relief inspection: {error}");
        std::process::exit(1);
    }
}
