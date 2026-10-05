mod board;

use std::{error::Error, fs, io::Write, path::Path, process::ExitCode};

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(5..=7).contains(&args.len()) || args[0] != "material-board" {
        return Err("usage: resina-lab material-board <scenes.json> <font-file> <font-family> <output.png> [device-scale=1] [text-scale=1]".into());
    }
    let scale: f32 = args.get(5).map_or(Ok(1.0), |v| v.parse())?;
    let text_scale: f64 = args.get(6).map_or(Ok(1.0), |v| v.parse())?;
    let source = fs::read_to_string(&args[1])
        .map_err(|error| format!("cannot read scene catalog {:?}: {error}", args[1]))?;
    let data = fs::read(&args[2])
        .map_err(|error| format!("cannot read font file {:?}: {error}", args[2]))?;
    board::validate_font(&data, &args[3])?;
    guido::load_font(data);
    let family = guido::widgets::font::FontFamily::name(&args[3]);
    let board = board::prepare(&source, family, scale, text_scale)?;
    let pixels = board::render(&board)?;
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, board.width, board.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&pixels)?;
    writer.finish()?;
    let path = Path::new(&args[4]);
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("cannot create output {}: {error}", path.display()))?;
    if let Err(error) = output.write_all(&bytes) {
        drop(output);
        let cleanup = fs::remove_file(path);
        let detail = match cleanup {
            Ok(()) => format!("cannot write output {}: {error}", path.display()),
            Err(cleanup) => format!(
                "cannot write output {}: {error}; cannot remove partial output: {cleanup}",
                path.display()
            ),
        };
        return Err(detail.into());
    }
    println!(
        "{}: {}x{}, 16 material specimens",
        path.display(),
        board.width,
        board.height
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("material board: {error}");
            ExitCode::FAILURE
        }
    }
}
