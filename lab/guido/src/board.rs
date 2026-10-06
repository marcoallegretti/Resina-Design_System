use guido::{
    renderer::{
        CornerRadii, DrawCommand, FlattenScratch, GpuContext, RenderNode, RenderTarget, Renderer,
        flatten_root_into,
    },
    widgets::{Color, ContentFit, Rect, font::FontFamily},
};
use resina_environment::{LayoutDirection, SafeArea};
use resina_guido::{measure_command_label, prepare_command_label_at, prepare_surface_paint_at};
use resina_model::{ColorRole, FontFamilyRole, PhysicalVector, SurfaceSize, TypographyRole};
use resina_resolver::{
    CommandLabelInput, HeadlessResolution, SrgbFallback, SurfacePaintIr, opaque_contrast_ratio,
    resolve_command_label, resolve_surface_paint_source, resolve_theme_request_source,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, error::Error, rc::Rc};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const WIDTH: f32 = 1008.0;
const COLUMN_WIDTH: f32 = 468.0;
const MAX_PIXELS: u64 = 16_777_216;

pub(crate) struct Board {
    pub(crate) width: u32,
    pub(crate) height: u32,
    commands: Vec<DrawCommand>,
    scale: f32,
    clear: Color,
}
struct Specimen {
    paint: SurfacePaintIr,
    theme: HeadlessResolution,
}

pub(crate) fn validate_font(data: &[u8], name: &str) -> Result<()> {
    let mut database = fontdb::Database::new();
    database.load_font_data(data.to_vec());
    if name.trim().is_empty()
        || !database
            .faces()
            .any(|face| face.families.iter().any(|(family, _)| family == name))
    {
        return Err(format!("font file contains no face in the requested family {name:?}").into());
    }
    Ok(())
}
fn color(value: &SrgbFallback) -> Color {
    let [r, g, b] = value.components().map(|v| v as f32);
    Color::rgba(r, g, b, value.alpha() as f32)
}
fn rectangle(rect: Rect, color: Color) -> DrawCommand {
    DrawCommand::RoundedRect {
        rect,
        color,
        radius: CornerRadii::uniform(0.0),
        curvature: 1.0,
        border: None,
        shadow: None,
        gradient: None,
    }
}
fn label(
    theme: &HeadlessResolution,
    family: FontFamily,
    role: TypographyRole,
    text: &str,
    rect: Rect,
    foreground: ColorRole,
    scale: f32,
) -> Result<DrawCommand> {
    let style = &theme.typography()[&role];
    if style.family_role() != FontFamilyRole::Sans {
        return Err("board text requires an explicit Sans font mapping".into());
    }
    let foreground = &theme.opaque_color_fallbacks()[&foreground];
    let background = &theme.opaque_color_fallbacks()[&ColorRole::SurfaceBase];
    if opaque_contrast_ratio(foreground, background)? < 4.5 {
        return Err(
            format!("board text {text:?} fails contrast on its actual theme canvas").into(),
        );
    }
    let label = resolve_command_label(
        CommandLabelInput {
            text,
            typography: style,
            minimum_size: SurfaceSize {
                width: f64::from(rect.width),
                height: f64::from(rect.height),
            },
            maximum_size: SurfaceSize {
                width: f64::from(rect.width),
                height: f64::from(rect.height),
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
    )?;
    let mut command = prepare_command_label_at(
        &label,
        family,
        color(foreground),
        PhysicalVector {
            x: f64::from(rect.x),
            y: f64::from(rect.y),
        },
        scale,
    )?;
    let DrawCommand::Text { align, .. } = &mut command else {
        unreachable!()
    };
    *align = guido::widgets::TextAlign::Start;
    Ok(command)
}

fn specimens(source: &str, text_scale: f64) -> Result<BTreeMap<String, Specimen>> {
    if !text_scale.is_finite() || !(1.0..=3.0).contains(&text_scale) {
        return Err("text scale must be finite and in [1, 3]".into());
    }
    let document: Value = serde_json::from_str(source)?;
    if document["schemaVersion"] != "0.1.0" {
        return Err("prepared scene schemaVersion must be 0.1.0".into());
    }
    let scenes = document["scenarios"]
        .as_array()
        .ok_or("prepared scenes need scenarios")?;
    let mut result = BTreeMap::new();
    for scene in scenes {
        match scene["kind"].as_str() {
            Some("surfacePaint") => (),
            Some("opaqueSurface" | "focusRing") => continue,
            _ => return Err("unsupported prepared scene kind".into()),
        }
        let name = scene["name"].as_str().ok_or("complete scene has no name")?;
        let mut request = scene["request"].clone();
        let capabilities = request["body"]["theme"]["environment"]["rendererCapabilities"]
            .as_object()
            .ok_or("scene needs explicit renderer capabilities")?;
        if capabilities.values().any(|value| value != &json!(false)) {
            return Err(format!("{name}: this board requires Tier 0 capabilities").into());
        }
        request["body"]["theme"]["environment"]["textScale"] = json!(text_scale);
        let paint = resolve_surface_paint_source(&request.to_string())
            .map_err(|error| format!("{name}: {error}"))?;
        if serde_json::to_value(paint.body().material_family())? != scene["expectedMaterialFamily"]
        {
            return Err(format!("{name}: material differs from catalog declaration").into());
        }
        let theme = resolve_theme_request_source(&request["body"]["theme"].to_string())?;
        if serde_json::to_value(theme.opaque_color_fallbacks()[&ColorRole::SurfaceBase].clone())?
            != request["surroundingColor"]
        {
            return Err(
                format!("{name}: actual surrounding color differs from board canvas").into(),
            );
        }
        if result
            .insert(name.to_owned(), Specimen { paint, theme })
            .is_some()
        {
            return Err(format!("duplicate complete scene {name}").into());
        }
    }
    for theme in ["light", "dark"] {
        for material in ["cast", "frost", "elastomer", "gel"] {
            for state in ["rest", "focused"] {
                let name = format!("{theme}-{material}-paint-{state}");
                let scene = result
                    .get(&name)
                    .ok_or_else(|| format!("missing complete scene {name}"))?;
                if serde_json::to_value(scene.paint.body().material_family())? != json!(material) {
                    return Err(format!("{name}: material differs from catalog name").into());
                }
                let baseline = &result[&format!("{theme}-cast-paint-rest")].theme;
                if serde_json::to_value(&scene.theme)? != serde_json::to_value(baseline)? {
                    return Err(
                        format!("{name}: resolved theme differs within its board column").into(),
                    );
                }
                if scene.paint.focus().is_some() != (state == "focused") {
                    return Err(format!("{name}: focus differs from catalog name").into());
                }
            }
        }
    }
    if result.len() != 16 {
        return Err("board requires exactly 16 complete material scenes".into());
    }
    Ok(result)
}

pub(crate) fn prepare(
    source: &str,
    family: FontFamily,
    scale: f32,
    text_scale: f64,
) -> Result<Board> {
    if !scale.is_finite() || !(0.5..=4.0).contains(&scale) {
        return Err("device scale must be finite and in [0.5, 4]".into());
    }
    let specimens = specimens(source, text_scale)?;
    let heading_height = (34.0 * text_scale) as f32;
    let caption_height = (24.0 * text_scale) as f32;
    let row_height = heading_height + caption_height + 120.0;
    let light = &specimens["light-cast-paint-rest"].theme;
    let title_height = heading_height.max(
        measure_command_label(
            family,
            resina_resolver::LabelMeasureInput {
                text: "Resina / Material Board",
                typography: &light.typography()[&TypographyRole::Heading],
                maximum_width: Some(f64::from(WIDTH - 72.0)),
            },
        )?
        .height as f32,
    );
    let intro_height = caption_height.max(
        measure_command_label(
            family,
            resina_resolver::LabelMeasureInput {
                text: "Tier 0 / opaque paint / rest and focus",
                typography: &light.typography()[&TypographyRole::Body],
                maximum_width: Some(f64::from(WIDTH - 72.0)),
            },
        )?
        .height as f32,
    );
    let top = title_height + intro_height + 84.0;
    let height = top + heading_height + 48.0 + row_height * 4.0 + 32.0;
    let (width_px, height_px) = (
        (WIDTH * scale).ceil() as u32,
        (height * scale).ceil() as u32,
    );
    if u64::from(width_px) * u64::from(height_px) > MAX_PIXELS {
        return Err("material board exceeds the 16 megapixel capture limit".into());
    }
    let mut commands = vec![rectangle(
        Rect::new(0.0, 0.0, width_px as f32 / scale, height_px as f32 / scale),
        color(&light.opaque_color_fallbacks()[&ColorRole::SurfaceBase]),
    )];
    commands.push(label(
        light,
        family,
        TypographyRole::Heading,
        "Resina / Material Board",
        Rect::new(36.0, 28.0, WIDTH - 72.0, title_height),
        ColorRole::ContentPrimary,
        scale,
    )?);
    commands.push(label(
        light,
        family,
        TypographyRole::Body,
        "Tier 0 / opaque paint / rest and focus",
        Rect::new(36.0, 40.0 + title_height, WIDTH - 72.0, intro_height),
        ColorRole::ContentSecondary,
        scale,
    )?);
    for (column, theme_name) in ["light", "dark"].into_iter().enumerate() {
        let theme = &specimens[&format!("{theme_name}-cast-paint-rest")].theme;
        let x = 24.0 + column as f32 * (COLUMN_WIDTH + 24.0);
        commands.push(rectangle(
            Rect::new(x, top, COLUMN_WIDTH, height - top - 24.0),
            color(&theme.opaque_color_fallbacks()[&ColorRole::SurfaceBase]),
        ));
        commands.push(label(
            theme,
            family,
            TypographyRole::Heading,
            if theme_name == "light" {
                "Light"
            } else {
                "Dark"
            },
            Rect::new(x + 24.0, top + 24.0, COLUMN_WIDTH - 48.0, heading_height),
            ColorRole::ContentPrimary,
            scale,
        )?);
        for (row, material) in ["Cast", "Frost", "Elastomer", "Gel"]
            .into_iter()
            .enumerate()
        {
            let y = top + heading_height + 56.0 + row as f32 * row_height;
            commands.push(label(
                theme,
                family,
                TypographyRole::Heading,
                material,
                Rect::new(x + 24.0, y, COLUMN_WIDTH - 48.0, heading_height),
                ColorRole::ContentPrimary,
                scale,
            )?);
            for (slot, state) in ["rest", "focused"].into_iter().enumerate() {
                let sx = x + 24.0 + slot as f32 * 216.0;
                commands.push(label(
                    theme,
                    family,
                    TypographyRole::Caption,
                    if state == "rest" { "Rest" } else { "Focus" },
                    Rect::new(sx, y + heading_height + 8.0, 180.0, caption_height),
                    ColorRole::ContentSecondary,
                    scale,
                )?);
                let scene =
                    &specimens[&format!("{theme_name}-{}-paint-{state}", material.to_lowercase())];
                let prepared = prepare_surface_paint_at(
                    &scene.paint,
                    PhysicalVector {
                        x: f64::from(sx),
                        y: f64::from(y + heading_height + caption_height + 24.0),
                    },
                    scale,
                    4,
                )?;
                let (ox, oy) = prepared.origin();
                let size = prepared.logical_size();
                if size.width > 204.0 || size.height > 104.0 {
                    return Err("catalog surface exceeds its board specimen slot".into());
                }
                commands.push(DrawCommand::Image {
                    source: prepared.image_source(),
                    decoded: None,
                    rect: Rect::new(ox, oy, size.width, size.height),
                    content_fit: ContentFit::Fill,
                    tint: None,
                });
            }
        }
    }
    Ok(Board {
        width: width_px,
        height: height_px,
        commands,
        scale,
        clear: color(&light.opaque_color_fallbacks()[&ColorRole::SurfaceBase]),
    })
}

pub(crate) fn render(board: &Board) -> Result<Vec<u8>> {
    let gpu = GpuContext::try_new().ok_or("a native GPU or software Vulkan adapter is required")?;
    let mut target = RenderTarget::offscreen(&gpu, board.width, board.height);
    let mut renderer = Renderer::new(gpu.device.clone(), gpu.queue.clone(), target.format());
    renderer.set_screen_size(board.width as f32, board.height as f32);
    renderer.set_scale_factor(board.scale);
    let mut root = RenderNode::new(1);
    root.commands
        .extend(board.commands.iter().cloned().map(Rc::new));
    let (mut flattened, mut layers, mut scratch) =
        (Vec::new(), Vec::new(), FlattenScratch::default());
    flatten_root_into(&root, &mut flattened, &mut layers, &mut scratch);
    if !renderer.render(&mut target, &flattened, &layers, board.clear) {
        return Err("native board frame was not rendered".into());
    }
    let RenderTarget::Offscreen(target) = target else {
        unreachable!()
    };
    let row_bytes = (board.width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("material board readback"),
        size: u64::from(row_bytes) * u64::from(board.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(board.height),
            },
        },
        wgpu::Extent3d {
            width: board.width,
            height: board.height,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    let (sent, received) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sent.send(result);
        });
    gpu.device.poll(wgpu::PollType::wait_indefinitely())?;
    received.recv()??;
    let mapped = buffer.slice(..).get_mapped_range()?;
    Ok(mapped
        .chunks_exact(row_bytes as usize)
        .flat_map(|row| row[..(board.width * 4) as usize].iter().copied())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> String {
        std::fs::read_to_string(std::env::var("RESINA_SCENES").expect("RESINA_SCENES required"))
            .unwrap()
    }
    fn font() -> FontFamily {
        let data =
            std::fs::read(std::env::var("RESINA_LABEL_FONT").expect("RESINA_LABEL_FONT required"))
                .unwrap();
        validate_font(&data, "DejaVu Sans").unwrap();
        guido::load_font(data);
        FontFamily::name("DejaVu Sans")
    }
    #[test]
    fn every_native_specimen_and_text_line_survives_placement_and_scaling() {
        let family = font();
        let source = source();
        for (scale, text_scale) in [
            (0.5, 1.0),
            (1.0, 1.0),
            (1.25, 1.0),
            (2.0, 2.0),
            (1.25, 2.0),
            (1.0, 3.0),
            (1.3, 1.25),
        ] {
            let board = prepare(&source, family, scale, text_scale).unwrap();
            let pixels = render(&board).unwrap();
            assert_eq!(pixels.len(), (board.width * board.height * 4) as usize);
            assert!(pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
            let mut count = 0;
            for command in &board.commands {
                let DrawCommand::Image {
                    source:
                        guido::prelude::ImageSource::Rgba {
                            width,
                            pixels: prepared,
                            ..
                        },
                    rect,
                    ..
                } = command
                else {
                    continue;
                };
                assert!(
                    rect.x >= 0.0
                        && rect.y >= 0.0
                        && (rect.x + rect.width) * scale <= board.width as f32
                        && (rect.y + rect.height) * scale <= board.height as f32
                );
                let slot_x = [48.0, 264.0, 540.0, 756.0][(count / 8) * 2 + count % 2];
                assert!(
                    (rect.x - slot_x).abs() < 12.0,
                    "specimen translated twice or assigned to wrong slot"
                );
                for value in [rect.x * scale, rect.y * scale] {
                    assert!((value - value.round()).abs() < 0.001);
                }
                let (ox, oy) = (
                    (rect.x * scale).round() as u32,
                    (rect.y * scale).round() as u32,
                );
                let mut opaque = 0;
                for (i, pixel) in prepared
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p[3] == 255)
                {
                    let at = (((oy + i as u32 / *width) * board.width + ox + i as u32 % *width) * 4)
                        as usize;
                    for channel in 0..4 {
                        assert!(pixels[at + channel].abs_diff(pixel[channel]) <= 1);
                    }
                    opaque += 1;
                }
                assert!(opaque > 100);
                count += 1;
            }
            assert_eq!(count, 16);
            let text_commands: Vec<_> = board
                .commands
                .iter()
                .filter(|c| matches!(c, DrawCommand::Text { .. }))
                .cloned()
                .collect();
            let mask_board = Board {
                width: board.width,
                height: board.height,
                commands: text_commands,
                scale,
                clear: Color::TRANSPARENT,
            };
            let mask = render(&mask_board).unwrap();
            assert_eq!(mask_board.commands.len(), 28);
            for command in &mask_board.commands {
                let DrawCommand::Text {
                    rect,
                    font_size,
                    line_height,
                    ..
                } = command
                else {
                    unreachable!()
                };
                let (x0, y0) = (
                    (rect.x * scale).floor() as u32,
                    (rect.y * scale).floor() as u32,
                );
                let (x1, y1) = (
                    ((rect.x + rect.width) * scale).ceil() as u32,
                    ((rect.y + rect.height) * scale).ceil() as u32,
                );
                assert!(x1 <= board.width && y1 <= board.height);
                let guido::widgets::font::LineHeight::Relative(relative) = line_height else {
                    panic!("resolved typography must supply explicit relative line height");
                };
                let line_height = font_size * relative;
                let lines = (rect.height / line_height).round() as u32;
                assert!(lines > 0);
                for line in 0..lines {
                    let start = ((rect.y + line as f32 * line_height) * scale).floor() as u32;
                    let end = ((rect.y + (line + 1) as f32 * line_height) * scale).ceil() as u32;
                    let ink = (start.max(y0)..end.min(y1))
                        .flat_map(|y| (x0..x1).map(move |x| ((y * board.width + x) * 4) as usize))
                        .filter(|at| mask[*at + 3] > 0)
                        .count();
                    assert!(ink > 8, "native board label line {line} missing");
                }
            }
        }
    }
    #[test]
    fn inconsistent_catalogs_and_unsupported_scales_fail_explicitly() {
        let original: Value = serde_json::from_str(&source()).unwrap();
        for (path, value, diagnostic) in [
            ("/schemaVersion", json!("9.0.0"), "schemaVersion"),
            ("/scenarios/0/kind", json!("unknown"), "unsupported"),
        ] {
            let mut bad = original.clone();
            *bad.pointer_mut(path).unwrap() = value;
            assert!(
                specimens(&bad.to_string(), 1.0)
                    .err()
                    .unwrap()
                    .to_string()
                    .contains(diagnostic)
            );
        }
        let index = original["scenarios"]
            .as_array()
            .unwrap()
            .iter()
            .position(|s| s["name"] == "light-cast-paint-rest")
            .unwrap();
        let mut duplicate = original.clone();
        let scene = duplicate["scenarios"][index].clone();
        duplicate["scenarios"].as_array_mut().unwrap().push(scene);
        assert!(
            specimens(&duplicate.to_string(), 1.0)
                .err()
                .unwrap()
                .to_string()
                .contains("duplicate")
        );
        let mut missing = original.clone();
        missing["scenarios"].as_array_mut().unwrap().remove(index);
        assert!(
            specimens(&missing.to_string(), 1.0)
                .err()
                .unwrap()
                .to_string()
                .contains("missing")
        );
        let mut capability = original.clone();
        capability["scenarios"][index]["request"]["body"]["theme"]["environment"]["rendererCapabilities"]
            ["gradients"] = json!(true);
        assert!(
            specimens(&capability.to_string(), 1.0)
                .err()
                .unwrap()
                .to_string()
                .contains("Tier 0")
        );
        let mut family = original.clone();
        family["scenarios"][index]["expectedMaterialFamily"] = json!("gel");
        assert!(
            specimens(&family.to_string(), 1.0)
                .err()
                .unwrap()
                .to_string()
                .contains("material differs")
        );
        let focus_index = original["scenarios"]
            .as_array()
            .unwrap()
            .iter()
            .position(|s| s["name"] == "light-cast-paint-focused")
            .unwrap();
        let mut focus = original.clone();
        focus["scenarios"][focus_index]["request"]["body"]["surface"]["states"]["states"] =
            json!(["rest"]);
        assert!(
            specimens(&focus.to_string(), 1.0)
                .err()
                .unwrap()
                .to_string()
                .contains("focus differs")
        );
        let mut inconsistent = original.clone();
        let mut theme: Value = serde_json::from_str(
            inconsistent["scenarios"][focus_index]["request"]["body"]["theme"]["themeSource"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        theme["typographyAssignments"]["roles"]["heading"]["fontSize"] = json!("type.size.5");
        inconsistent["scenarios"][focus_index]["request"]["body"]["theme"]["themeSource"] =
            json!(theme.to_string());
        assert!(
            specimens(&inconsistent.to_string(), 1.0)
                .err()
                .unwrap()
                .to_string()
                .contains("resolved theme differs")
        );
        assert!(
            prepare(&source(), font(), 4.0, 3.0)
                .err()
                .unwrap()
                .to_string()
                .contains("capture limit")
        );
        for scale in [0.0, f32::NAN, f32::INFINITY, 5.0] {
            assert!(prepare(&source(), FontFamily::name("unused"), scale, 1.0).is_err());
        }
        for scale in [0.0, f64::NAN, f64::INFINITY, 4.0] {
            assert!(specimens(&source(), scale).is_err());
        }
    }
    #[test]
    fn invalid_and_mismatched_font_files_are_rejected() {
        assert!(validate_font(b"not a font", "DejaVu Sans").is_err());
        let data =
            std::fs::read(std::env::var("RESINA_LABEL_FONT").expect("RESINA_LABEL_FONT required"))
                .unwrap();
        assert!(validate_font(&data, "unknown family").is_err());
        assert!(validate_font(&data, "").is_err());
        validate_font(&data, "DejaVu Sans").unwrap();
    }
}
