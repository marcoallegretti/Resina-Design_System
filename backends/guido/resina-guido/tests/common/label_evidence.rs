use guido::renderer::GpuContext;
use resina_resolver::CommandLabelIr;
use serde_json::json;
use std::{fs, io::Write, path::Path, process::Command};

fn git_output(args: &[&str]) -> String {
    let output = Command::new("git").args(args).output().unwrap();
    assert!(output.status.success(), "Git capture identity failed");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn blob(path: &Path) -> String {
    git_output(&["hash-object", "--no-filters", path.to_str().unwrap()])
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
    let font = std::env::var("RESINA_LABEL_FONT").unwrap();
    let mut fonts = fontdb::Database::new();
    fonts.load_font_data(fs::read(&font).unwrap());
    assert!(
        fonts
            .faces()
            .any(|face| face.families.iter().any(|(name, _)| name == "DejaVu Sans")),
        "native label evidence requires the actual DejaVu Sans font file"
    );
    let adapter = &gpu.adapter_info;
    let record = json!({
        "schemaVersion": "0.1.0", "kind": "nativeLabel", "purpose": "diagnostic",
        "commit": git_output(&["rev-parse", "HEAD"]),
        "scenario": path.file_stem().unwrap().to_str().unwrap(),
        "semanticResult": ir,
        "width": size.0, "height": size.1,
        "deviceScale": scales.0, "textScale": scales.1,
        "sampling": {"time": 0, "inputTrajectory": []},
        "font": {"family": "DejaVu Sans", "gitBlob": blob(Path::new(&font))},
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
