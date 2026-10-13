use crate::board::Board;
use guido::{prelude::*, renderer::DrawCommand, widget_prelude::*};
use std::error::Error;

struct NativeBoard {
    commands: Vec<DrawCommand>,
    size: Size,
}

impl Widget for NativeBoard {
    fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        constraints.constrain(self.size)
    }
    fn paint(&self, ctx: &mut PaintContext) {
        for command in &self.commands {
            match command {
                DrawCommand::Image {
                    source,
                    rect,
                    content_fit,
                    tint,
                    ..
                } => ctx.draw_image(source.clone(), *rect, *content_fit, *tint),
                DrawCommand::RoundedRect {
                    rect,
                    color,
                    radius,
                    curvature,
                    border: None,
                    shadow: None,
                    gradient: None,
                } => ctx.draw_rounded_rect_with_curvature(*rect, *color, *radius, *curvature),
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

pub(crate) fn dimensions(board: &Board) -> Result<(u32, u32), Box<dyn Error>> {
    let width = board.width as f32 / board.scale;
    let height = board.height as f32 / board.scale;
    if width.fract() != 0.0 || height.fract() != 0.0 {
        return Err("native board requires integral logical surface dimensions".into());
    }
    for command in &board.commands {
        match command {
            DrawCommand::Image { decoded: None, .. }
            | DrawCommand::Text { .. }
            | DrawCommand::RoundedRect {
                border: None,
                shadow: None,
                gradient: None,
                ..
            } => (),
            _ => return Err("native board contains an unsupported draw command".into()),
        }
    }
    Ok((width as u32, height as u32))
}

pub(crate) fn present(board: Board) -> Result<(), Box<dyn Error>> {
    let (width, height) = dimensions(&board)?;
    let config = SurfaceConfig::new()
        .width(width)
        .height(height)
        .anchor(Anchor::TOP | Anchor::LEFT)
        .background_color(board.clear);
    let widget = NativeBoard {
        commands: board.commands,
        size: Size::new(width as f32, height as f32),
    };
    match App::new().run(move |app| {
        app.add_surface(config, move || widget);
    }) {
        guido::ExitReason::Quit => Ok(()),
        reason => Err(format!("native board exited: {reason:?}").into()),
    }
}
