use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight};
use guido::renderer::GpuContext;
use resina_resolver::CommandLabelIr;
use serde_json::json;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use super::conformance_font::FAMILY;

fn git_output(args: &[&str]) -> String {
    let output = Command::new("git").args(args).output().unwrap();
    assert!(output.status.success(), "Git capture identity failed");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn blob(path: &Path) -> String {
    git_output(&["hash-object", "--no-filters", path.to_str().unwrap()])
}

pub struct Master {
    pub family: String,
    pub post_script_name: String,
    pub weight: u16,
    pub path: PathBuf,
}

const AUTHORED_WEIGHTS: [u16; 3] = [400, 500, 600];

/// The script fallback families that cosmic-text names for Arabic and Hebrew text.
const SCRIPT_FAMILIES: [(&str, &str); 2] = [
    ("Arabic", "Noto Sans Arabic"),
    ("Hebrew", "Noto Sans Hebrew"),
];

/// Every file the label evidence registers: the Inter masters, then the script masters.
pub fn registered() -> Vec<PathBuf> {
    super::conformance_font::masters()
        .into_iter()
        .chain(super::conformance_font::script_masters())
        .collect()
}

struct Pinned {
    family: &'static str,
    weight: u16,
    path: PathBuf,
    data: Vec<u8>,
}

fn pinned() -> Result<Vec<Pinned>, String> {
    let families = std::iter::repeat_n(FAMILY, 3).chain(
        SCRIPT_FAMILIES
            .iter()
            .flat_map(|(_, family)| std::iter::repeat_n(*family, 3)),
    );
    registered()
        .into_iter()
        .zip(families)
        .zip(AUTHORED_WEIGHTS.into_iter().cycle())
        .map(|((path, family), weight)| {
            fs::read(&path)
                .map(|data| Pinned {
                    family,
                    weight,
                    path: path.clone(),
                    data,
                })
                .map_err(|error| format!("{}: {error}", path.display()))
        })
        .collect()
}

/// Builds the face database GUIdo's text stack matches against.
///
/// The pinned GUIdo creates its cosmic-text `FontSystem` with the system fonts and
/// then appends every registered file. `system_fonts: false` isolates the
/// registered files.
pub fn database(registered: &[PathBuf], system_fonts: bool) -> Result<fontdb::Database, String> {
    let mut fonts = fontdb::Database::new();
    if system_fonts {
        fonts.load_system_fonts();
    }
    for path in registered {
        let data = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        fonts.load_font_source(fontdb::Source::Binary(Arc::new(data)));
    }
    Ok(fonts)
}

fn source(fonts: &fontdb::Database, id: fontdb::ID) -> String {
    match fonts.face_source(id) {
        Some((fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _), index)) => {
            format!("{} face {index}", path.display())
        }
        Some((_, index)) => format!("registered data face {index}"),
        None => "an unknown source".to_owned(),
    }
}

/// The pinned master that a visible face is byte-identical to.
fn identify<'a>(
    fonts: &fontdb::Database,
    pinned: &'a [Pinned],
    id: fontdb::ID,
) -> Option<&'a Pinned> {
    fonts
        .with_face_data(id, |data, index| {
            pinned
                .iter()
                .find(|master| index == 0 && master.data.as_slice() == data)
        })
        .flatten()
}

/// Rejects a visible upright, normal-width face of the family at an authored weight
/// that is not the pinned master of that weight, so a selection cannot depend on load order.
fn check_visible(fonts: &fontdb::Database, family: &str, pinned: &[Pinned]) -> Result<(), String> {
    for face in fonts.faces() {
        if face.families.iter().any(|(name, _)| name == family)
            && face.style == fontdb::Style::Normal
            && face.stretch == fontdb::Stretch::Normal
            && AUTHORED_WEIGHTS.contains(&face.weight.0)
            && identify(fonts, pinned, face.id).map(|master| (master.family, master.weight))
                != Some((family, face.weight.0))
        {
            return Err(format!(
                "{} ({}, {}) is visible as {family} weight {} but is not the pinned master",
                face.post_script_name,
                face.weight.0,
                source(fonts, face.id),
                face.weight.0
            ));
        }
    }
    Ok(())
}

fn master(fonts: &fontdb::Database, pinned: &Pinned, id: fontdb::ID) -> Master {
    Master {
        family: pinned.family.to_owned(),
        post_script_name: fonts.face(id).unwrap().post_script_name.clone(),
        weight: pinned.weight,
        path: pinned.path.clone(),
    }
}

/// Identifies the face that fontdb's CSS matching selects for a requested weight.
///
/// The pinned cosmic-text tries this face first for GUIdo text. Every visible
/// upright, normal-width face of the family at an authored weight must be
/// byte-identical to the pinned master of that weight, so the selection cannot
/// depend on load order. A face whose own weight differs from the request is a
/// substitution.
pub fn resolve(fonts: &fontdb::Database, weight: u16) -> Result<Master, String> {
    let pinned = pinned()?;
    check_visible(fonts, FAMILY, &pinned)?;
    let id = fonts
        .query(&fontdb::Query {
            families: &[fontdb::Family::Name(FAMILY)],
            weight: fontdb::Weight(weight),
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        })
        .ok_or_else(|| format!("no {FAMILY} face matches weight {weight}"))?;
    let face = fonts.face(id).unwrap();
    if face.weight.0 != weight {
        return Err(format!(
            "weight {weight} resolved to {} ({}, {}); the requested weight was substituted",
            face.post_script_name,
            face.weight.0,
            source(fonts, id)
        ));
    }
    let found = identify(fonts, &pinned, id).ok_or_else(|| {
        format!(
            "weight {weight} resolved to {} ({}), which is not a pinned master",
            face.post_script_name,
            source(fonts, id)
        )
    })?;
    Ok(master(fonts, found, id))
}

/// Identifies every face that draws a label text when it is shaped as GUIdo shapes it.
///
/// Arabic and Hebrew have no glyphs in Inter, so cosmic-text takes them from the
/// script fallback families it names. Each drawing face must be a pinned master at
/// the requested weight, and no glyph may be missing.
pub fn resolve_text(
    fonts: &fontdb::Database,
    text: &str,
    weight: u16,
) -> Result<Vec<Master>, String> {
    let pinned = pinned()?;
    for family in std::iter::once(FAMILY).chain(SCRIPT_FAMILIES.iter().map(|(_, family)| *family)) {
        check_visible(fonts, family, &pinned)?;
    }
    let mut system = FontSystem::new_with_locale_and_db("en-US".into(), fonts.clone());
    let mut buffer = Buffer::new(&mut system, Metrics::new(20.0, 28.0));
    buffer.set_size(None, None);
    let attrs = Attrs::new()
        .family(Family::Name(FAMILY))
        .weight(Weight(weight));
    buffer.set_text(text, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut system, true);
    let mut drawn: Vec<(fontdb::ID, char)> = Vec::new();
    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            let character = run.text[glyph.start..glyph.end].chars().next().unwrap();
            if glyph.glyph_id == 0 {
                return Err(format!(
                    "{character:?} has no glyph in any face at weight {weight}"
                ));
            }
            if !drawn.iter().any(|(id, _)| *id == glyph.font_id) {
                drawn.push((glyph.font_id, character));
            }
        }
    }
    drawn
        .into_iter()
        .map(|(id, character)| {
            let face = fonts.face(id).unwrap();
            let found = identify(fonts, &pinned, id).ok_or_else(|| {
                format!(
                    "{character:?} was drawn with {} ({}, {}), which is not a pinned master",
                    face.post_script_name,
                    face.weight.0,
                    source(fonts, id)
                )
            })?;
            if found.weight != weight {
                return Err(format!(
                    "{character:?} at weight {weight} was drawn with {} ({}, {}); the requested weight was substituted",
                    face.post_script_name,
                    face.weight.0,
                    source(fonts, id)
                ));
            }
            Ok(master(fonts, found, id))
        })
        .collect()
}

pub fn capture(
    path: &Path,
    size: (u32, u32),
    pixels: &[u8],
    gpu: &GpuContext,
    ir: &CommandLabelIr,
    scales: (f32, f64),
) {
    assert!(
        git_output(&["status", "--porcelain"]).is_empty(),
        "native evidence requires a committed clean source tree"
    );
    assert_eq!(pixels.len(), size.0 as usize * size.1 as usize * 4);
    assert!(pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
    let image = path.with_extension("png");
    let file = fs::File::options()
        .write(true)
        .create_new(true)
        .open(&image)
        .unwrap();
    let mut encoder = png::Encoder::new(file, size.0, size.1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(pixels).unwrap();
    writer.finish().unwrap();
    let weight = ir.typography().font_weight();
    assert_eq!(weight.fract(), 0.0);
    let fonts = database(&registered(), true).unwrap();
    let master = resolve(&fonts, weight as u16)
        .expect("native label evidence requires the pinned master for the requested weight");
    let script_fonts: Vec<_> = resolve_text(&fonts, ir.text(), weight as u16)
        .expect("native label evidence requires pinned masters for every script of the text")
        .into_iter()
        .filter(|face| face.family != FAMILY)
        .map(|face| {
            json!({
                "family": face.family, "postScriptName": face.post_script_name, "weight": face.weight,
                "file": face.path.file_name().unwrap().to_str().unwrap(),
                "gitBlob": blob(&face.path)
            })
        })
        .collect();
    let adapter = &gpu.adapter_info;
    let record = json!({
        "schemaVersion": "0.1.0", "kind": "nativeLabel", "purpose": "diagnostic",
        "commit": git_output(&["rev-parse", "HEAD"]),
        "scenario": path.file_stem().unwrap().to_str().unwrap(),
        "semanticResult": ir,
        "width": size.0, "height": size.1,
        "deviceScale": scales.0, "textScale": scales.1,
        "sampling": {"time": 0, "inputTrajectory": []},
        "font": {
            "family": FAMILY, "postScriptName": master.post_script_name, "weight": master.weight,
            "file": master.path.file_name().unwrap().to_str().unwrap(),
            "gitBlob": blob(&master.path)
        },
        "scriptFonts": script_fonts,
        "renderer": {
            "name": "guido-native-label-test", "gitBlob": blob(&std::env::current_exe().unwrap()),
            "gpu": {
                "name": adapter.name, "vendor": adapter.vendor, "device": adapter.device,
                "backend": format!("{:?}", adapter.backend),
                "deviceType": format!("{:?}", adapter.device_type),
                "driver": adapter.driver, "driverInfo": adapter.driver_info
            }
        },
        "runtime": {"system": std::env::consts::OS, "machine": std::env::consts::ARCH},
        "colorEncoding": "rgba8-srgb-straight",
        "image": {"file": image.file_name().unwrap().to_str().unwrap(), "gitBlob": blob(&image)}
    });
    let mut metadata = fs::File::options()
        .write(true)
        .create_new(true)
        .open(path.with_extension("json"))
        .unwrap();
    metadata
        .write_all(serde_json::to_string_pretty(&record).unwrap().as_bytes())
        .unwrap();
}
