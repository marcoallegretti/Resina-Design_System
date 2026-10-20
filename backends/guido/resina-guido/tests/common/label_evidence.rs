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
    pub post_script_name: String,
    pub weight: u16,
    pub path: PathBuf,
}

const AUTHORED_WEIGHTS: [u16; 3] = [400, 500, 600];

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

/// Identifies the face that fontdb's CSS matching selects for a requested weight.
///
/// The pinned cosmic-text tries this face first for GUIdo text. Every visible
/// upright, normal-width face of the family at an authored weight must be
/// byte-identical to the pinned master of that weight, so the selection cannot
/// depend on load order. A face whose own weight differs from the request is a
/// substitution.
pub fn resolve(fonts: &fontdb::Database, weight: u16) -> Result<Master, String> {
    let masters = super::conformance_font::masters()
        .into_iter()
        .zip(AUTHORED_WEIGHTS)
        .map(|(path, weight)| {
            fs::read(&path)
                .map(|data| (weight, data, path.clone()))
                .map_err(|error| format!("{}: {error}", path.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let pinned = |id: fontdb::ID| {
        fonts
            .with_face_data(id, |data, index| {
                masters
                    .iter()
                    .find(|(_, bytes, _)| index == 0 && bytes.as_slice() == data)
                    .map(|(weight, _, path)| (*weight, path.clone()))
            })
            .flatten()
    };
    for face in fonts.faces() {
        if face.families.iter().any(|(name, _)| name == FAMILY)
            && face.style == fontdb::Style::Normal
            && face.stretch == fontdb::Stretch::Normal
            && AUTHORED_WEIGHTS.contains(&face.weight.0)
            && pinned(face.id).map(|(weight, _)| weight) != Some(face.weight.0)
        {
            return Err(format!(
                "{} ({}, {}) is visible as {FAMILY} weight {} but is not the pinned master",
                face.post_script_name,
                face.weight.0,
                source(fonts, face.id),
                face.weight.0
            ));
        }
    }
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
    let (_, path) = pinned(id).ok_or_else(|| {
        format!(
            "weight {weight} resolved to {} ({}), which is not a pinned master",
            face.post_script_name,
            source(fonts, id)
        )
    })?;
    Ok(Master {
        post_script_name: face.post_script_name.clone(),
        weight,
        path,
    })
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
    let fonts = database(&super::conformance_font::masters(), true).unwrap();
    let master = resolve(&fonts, weight as u16)
        .expect("native label evidence requires the pinned master for the requested weight");
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
