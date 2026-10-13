use crate::board::{self, Board};
use guido::{
    renderer::DrawCommand,
    widgets::{ContentFit, Rect, font::FontFamily},
};
use resina_environment::{LayoutDirection, SafeArea};
use resina_guido::{measure_command_label, prepare_outline_focus_at, prepare_relief_content_at};
use resina_model::{
    ColorRole, FontFamilyRole, PhysicalVector, SurfaceIntent, SurfaceSize, TypographyRole,
};
use resina_resolver::{
    CommandLabelInput, resolve_command_label, resolve_focus_indicator, resolve_outline_focus,
    resolve_relief_paint_source, resolve_shape_fallback_source,
};
use serde_json::{Value, json};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const WIDTH: f32 = 840.0;
const COLUMN: f32 = 384.0;
const MAX_PIXELS: u64 = 16_777_216;

pub(crate) fn prepare(
    source: &str,
    family: FontFamily,
    scale: f32,
    text_scale: f64,
    kind: &str,
    state: &str,
) -> Result<Board> {
    if !scale.is_finite() || !(0.5..=4.0).contains(&scale) {
        return Err("device scale must be finite and in [0.5, 4]".into());
    }
    let title = match kind {
        "flat" => "Cast / flat structure",
        "convex" => "Elastomer / raised control",
        "recessed" => "Cast / recessed well",
        _ => return Err("relief kind must be flat, convex or recessed".into()),
    };
    let focused = match state {
        "rest" => false,
        "focused" => true,
        _ => return Err("relief state must be rest or focused".into()),
    };
    let specimens = board::specimens(source, text_scale)?;
    let document: Value = serde_json::from_str(source)?;
    let recipes: Value = serde_json::from_str(include_str!("../relief-recipes.json"))?;
    let appearance: Value = serde_json::from_str(include_str!(
        "../../../definitions/tier0-surface-appearance.json"
    ))?;
    let tokens: Value = serde_json::from_str(include_str!("../../../tokens/foundation.json"))?;
    let assignments: Value =
        serde_json::from_str(include_str!("../../../definitions/tier0-shapes.json"))?;
    let mut size: SurfaceSize = serde_json::from_value(recipes["sizes"][kind].clone())?;
    size.height *= text_scale;
    let light = &specimens["light-cast-paint-rest"].theme;
    let heading_height = (40.0 * text_scale) as f32;
    let title_height = heading_height
        .max(
            measure_command_label(
                family,
                resina_resolver::LabelMeasureInput {
                    text: "Resina / Relief Board",
                    typography: &light.typography()[&TypographyRole::Heading],
                    maximum_width: Some(f64::from(WIDTH - 56.0)),
                },
            )?
            .height as f32,
        )
        .next_up();
    let mut caption_height = heading_height;
    for theme_name in ["light", "dark"] {
        let theme = &specimens[&format!("{theme_name}-cast-paint-rest")].theme;
        for shape in ["structural", "soft", "rounded", "capsule", "organic"] {
            let metrics = measure_command_label(
                family,
                resina_resolver::LabelMeasureInput {
                    text: &format!("{theme_name} / {shape}"),
                    typography: &theme.typography()[&TypographyRole::Caption],
                    maximum_width: Some(f64::from(COLUMN - 48.0)),
                },
            )?;
            caption_height = caption_height.max((metrics.height as f32).next_up());
        }
    }
    let row_height = caption_height + size.height as f32 + 40.0;
    let top = title_height + heading_height + 96.0;
    let height = ((top + 5.0 * row_height + 32.0) / 4.0).ceil() * 4.0;
    let width_px = (WIDTH * scale).ceil() as u32;
    let height_px = (height * scale).ceil() as u32;
    if u64::from(width_px) * u64::from(height_px) > MAX_PIXELS {
        return Err("relief board exceeds the 16 megapixel capture limit".into());
    }
    let mut commands = vec![board::rectangle(
        Rect::new(0.0, 0.0, width_px as f32 / scale, height_px as f32 / scale),
        board::color(&light.opaque_color_fallbacks()[&ColorRole::SurfaceBase]),
    )];
    commands.push(
        board::label(
            light,
            family,
            TypographyRole::Heading,
            "Resina / Relief Board",
            Rect::new(28.0, 24.0, WIDTH - 56.0, title_height),
            ColorRole::ContentPrimary,
            scale,
        )
        .map_err(|error| format!("relief board heading: {error}"))?,
    );
    commands.push(
        board::label(
            light,
            family,
            TypographyRole::Body,
            title,
            Rect::new(28.0, 32.0 + title_height, WIDTH - 56.0, heading_height),
            ColorRole::ContentSecondary,
            scale,
        )
        .map_err(|error| format!("relief board introduction: {error}"))?,
    );
    for (column, theme_name) in ["light", "dark"].into_iter().enumerate() {
        let theme = &specimens[&format!("{theme_name}-cast-paint-rest")].theme;
        let scene = document["scenarios"]
            .as_array()
            .ok_or("scene catalog needs scenarios")?
            .iter()
            .find(|scene| scene["name"] == format!("{theme_name}-cast-paint-rest"))
            .ok_or("scene catalog lacks its theme source")?;
        let direction: LayoutDirection = serde_json::from_value(
            scene["request"]["body"]["theme"]["environment"]["layoutDirection"].clone(),
        )?;
        let canvas = &theme.opaque_color_fallbacks()[&ColorRole::SurfaceBase];
        let x = 24.0 + column as f32 * (COLUMN + 24.0);
        commands.push(board::rectangle(
            Rect::new(x, top - 24.0, COLUMN, height - top),
            board::color(canvas),
        ));
        for (row, shape) in ["structural", "soft", "rounded", "capsule", "organic"]
            .into_iter()
            .enumerate()
        {
            let recipe = recipes["recipes"]
                .as_array()
                .ok_or("relief recipes need an array")?
                .iter()
                .find(|recipe| recipe["name"] == format!("{kind}-{shape}"))
                .ok_or("relief recipes lack a required shape/profile")?;
            if recipe["shape"] != shape || recipe["profile"]["kind"] != kind {
                return Err("relief recipe identifier differs from its shape/profile".into());
            }
            let shape_request = json!({
                "schemaVersion": "0.1.0", "tokens": tokens, "assignments": assignments,
                "shape": shape, "size": size,
            });
            let radii = serde_json::to_value(resolve_shape_fallback_source(
                &shape_request.to_string(),
            )?)?["radii"]
                .clone();
            let outline = json!({
                "schemaVersion": "0.1.0", "size": size, "radii": radii,
                "layoutDirection": direction, "curvature": recipe["curvature"],
                "minimumInset": recipes["minimumInset"],
            });
            let material_role = serde_json::from_value(recipe["materialRole"].clone())?;
            let color_role = serde_json::from_value(recipe["colorRole"].clone())?;
            let foreground_role = serde_json::from_value(recipe["foregroundRole"].clone())?;
            let material = theme
                .materials()
                .get(&material_role)
                .ok_or("recipe material role is absent")?;
            if serde_json::to_value(material)? != recipe["expectedMaterialFamily"] {
                return Err("recipe material family differs from the actual theme".into());
            }
            let body = theme
                .opaque_color_fallbacks()
                .get(&color_role)
                .ok_or("recipe body role is absent")?;
            let foreground = theme
                .opaque_color_fallbacks()
                .get(&foreground_role)
                .ok_or("recipe foreground role is absent")?;
            let request = json!({
                "schemaVersion": "0.1.0", "outline": outline, "profile": recipe["profile"],
                "pigment": {
                    "schemaVersion": "0.1.0", "materialFamily": material,
                    "body": body, "profiles": appearance["pigmentProfiles"],
                },
                "keyLight": appearance["keyLight"],
                "edge": {
                    "schemaVersion": "0.1.0",
                    "outline": theme.opaque_color_fallbacks()[&ColorRole::Outline],
                    "outlineStrong": theme.opaque_color_fallbacks()[&ColorRole::OutlineStrong],
                    "adjacentColor": canvas, "minimumContrast": 3,
                },
                "edgeWidth": recipes["edgeWidth"], "foreground": foreground,
                "minimumContentContrast": 4.5, "points": [],
            });
            let paint = resolve_relief_paint_source(&request.to_string())?.paint;
            let intent: SurfaceIntent = serde_json::from_value(json!({
                "schemaVersion": "0.2.0", "materialRole": recipe["materialRole"],
                "colorRole": recipe["colorRole"],
                "form": {"schemaVersion": "0.1.0", "shape": shape, "elevation": "base"},
                "states": {"schemaVersion": "0.1.0", "states": if focused {
                    json!(["selected", "focused", "hover"])
                } else {json!(["rest"])}},
                "treatmentStack": {"schemaVersion": "0.1.0", "treatments": ["none"]},
            }))
            .map_err(|error| format!("{theme_name}-{kind}-{shape} binding: {error}"))?;
            let focus = if focused {
                Some(resolve_outline_focus(
                    resolve_focus_indicator(&intent, theme, canvas)?,
                    paint.surface().outline().clone(),
                    PhysicalVector { x: 0.0, y: 0.0 },
                )?)
            } else {
                None
            };
            let bounds = paint.content_bounds();
            let text = recipe["label"].as_str().ok_or("recipe label is absent")?;
            if theme.typography()[&TypographyRole::Label].family_role() != FontFamilyRole::Sans {
                return Err("relief labels require an explicit Sans font mapping".into());
            }
            let label = resolve_command_label(
                CommandLabelInput {
                    text,
                    typography: &theme.typography()[&TypographyRole::Label],
                    minimum_size: size,
                    maximum_size: size,
                    padding: SafeArea {
                        start: if direction == LayoutDirection::Ltr {
                            bounds.x
                        } else {
                            size.width - bounds.x - bounds.width
                        },
                        end: if direction == LayoutDirection::Ltr {
                            size.width - bounds.x - bounds.width
                        } else {
                            bounds.x
                        },
                        top: bounds.y,
                        bottom: size.height - bounds.y - bounds.height,
                    },
                    direction,
                },
                |input| measure_command_label(family, input),
            )
            .map_err(|error| format!("{theme_name}-{kind}-{shape} label: {error}"))?;
            let y = top + row as f32 * row_height;
            commands.push(
                board::label(
                    theme,
                    family,
                    TypographyRole::Caption,
                    &format!("{theme_name} / {shape}"),
                    Rect::new(x + 24.0, y, COLUMN - 48.0, caption_height),
                    ColorRole::ContentSecondary,
                    scale,
                )
                .map_err(|error| format!("{theme_name}-{kind}-{shape} caption: {error}"))?,
            );
            let origin = PhysicalVector {
                x: f64::from(x) + (f64::from(COLUMN) - size.width) / 2.0 + 0.25,
                y: f64::from(y + caption_height) + 7.125,
            };
            if let Some(focus) = &focus {
                let ring = prepare_outline_focus_at(focus, origin, scale)?;
                let (rx, ry) = ring.origin();
                let rs = ring.logical_size();
                if rx < x
                    || ry < top - 24.0
                    || rx + rs.width > x + COLUMN
                    || ry + rs.height > height - 24.0
                {
                    return Err("relief board clips complete focus bounds".into());
                }
                commands.push(DrawCommand::Image {
                    source: ring.image_source(),
                    decoded: None,
                    rect: Rect::new(rx, ry, rs.width, rs.height),
                    content_fit: ContentFit::Fill,
                    tint: None,
                });
            }
            let content = prepare_relief_content_at(&paint, &label, family, origin, scale, 4)?;
            if let DrawCommand::Image { rect, .. } = &content[0]
                && (rect.x < x
                    || rect.y < top - 24.0
                    || rect.x + rect.width > x + COLUMN
                    || rect.y + rect.height > height - 24.0)
            {
                return Err("relief board clips complete face bounds".into());
            }
            commands.extend(content);
        }
    }
    Ok(Board {
        width: width_px,
        height: height_px,
        commands,
        scale,
        clear: board::color(&light.opaque_color_fallbacks()[&ColorRole::SurfaceBase]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_and_font() -> (String, FontFamily) {
        let source = std::fs::read_to_string(std::env::var("RESINA_SCENES").unwrap()).unwrap();
        let font = std::fs::read(std::env::var("RESINA_LABEL_FONT").unwrap()).unwrap();
        board::validate_font(&font, "DejaVu Sans").unwrap();
        guido::load_font(font);
        (source, FontFamily::name("DejaVu Sans"))
    }

    #[test]
    fn every_profile_preserves_complete_glyphs_and_prepared_opaque_pixels() {
        let (source, family) = source_and_font();
        for kind in ["flat", "convex", "recessed"] {
            let mut board = prepare(&source, family, 1.25, 1.0, kind, "focused").unwrap();
            let original = board::render(&board).unwrap();
            assert!(
                original
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|pixel| pixel[3] == 255)
            );
            let text_rectangles: Vec<_> = board
                .commands
                .iter()
                .filter_map(|command| match command {
                    DrawCommand::Text { rect, .. } => Some(*rect),
                    _ => None,
                })
                .collect();
            let mut compared = 0;
            for command in &board.commands {
                let DrawCommand::Image {
                    source:
                        guido::prelude::ImageSource::Rgba {
                            width,
                            height,
                            pixels,
                        },
                    rect,
                    ..
                } = command
                else {
                    continue;
                };
                let ox = (rect.x * board.scale).round() as u32;
                let oy = (rect.y * board.scale).round() as u32;
                for y in 0..*height {
                    for x in 0..*width {
                        let i = ((y * width + x) * 4) as usize;
                        if pixels[i + 3] != 255 {
                            continue;
                        }
                        let fx = ox + x;
                        let fy = oy + y;
                        if text_rectangles.iter().any(|text| {
                            (fx as f32) >= (text.x * board.scale).floor() - 1.0
                                && (fx as f32) <= ((text.x + text.width) * board.scale).ceil() + 1.0
                                && (fy as f32) >= (text.y * board.scale).floor() - 1.0
                                && (fy as f32)
                                    <= ((text.y + text.height) * board.scale).ceil() + 1.0
                        }) {
                            continue;
                        }
                        let j = ((fy * board.width + fx) * 4) as usize;
                        for channel in 0..4 {
                            assert!(
                                original[j + channel].abs_diff(pixels[i + channel]) <= 1,
                                "{kind}: prepared paint changed at {fx},{fy}"
                            );
                        }
                        compared += 1;
                    }
                }
            }
            assert!(compared > 100_000);
            let commands = board.commands.clone();
            for command in &mut board.commands {
                if let DrawCommand::Text {
                    rect, align, fit, ..
                } = command
                {
                    if fit.is_none() {
                        *fit = Some(guido::renderer::LineFit::wrapping(Some(rect.width)));
                    }
                    if *align == guido::widgets::TextAlign::Center {
                        rect.x -= 32.0;
                    }
                    rect.width += 64.0;
                    rect.height += 64.0;
                }
            }
            assert!(
                original == board::render(&board).unwrap(),
                "{kind}: complete text differs from an expanded native rectangle"
            );
            board.commands = commands;
            board.commands.retain(|command| !matches!(command, DrawCommand::Text {text,..} if text.as_ref()=="light / structural"));
            assert!(
                original != board::render(&board).unwrap(),
                "{kind}: missing caption escaped the complete glyph oracle"
            );
        }
    }

    #[test]
    fn large_text_captions_fit_before_complete_relief_and_focus() {
        let (source, family) = source_and_font();
        let board = prepare(&source, family, 1.25, 3.0, "convex", "focused").unwrap();
        assert_eq!(board.width, 1050);
        assert_eq!(
            board
                .commands
                .iter()
                .filter(|command| matches!(command, DrawCommand::Text { .. }))
                .count(),
            22
        );
        for command in &board.commands {
            if let DrawCommand::Image { rect, .. } = command {
                assert!(rect.x >= 0.0 && rect.y >= 0.0);
                assert!((rect.x + rect.width) * board.scale <= board.width as f32);
                assert!((rect.y + rect.height) * board.scale <= board.height as f32);
            }
        }
    }
}
