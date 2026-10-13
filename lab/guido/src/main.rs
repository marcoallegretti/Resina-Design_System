mod board;
mod native_board;
mod relief_board;

use std::{error::Error, fs, io::Write, path::Path, process::ExitCode};

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let native = args
        .first()
        .is_some_and(|mode| mode == "relief-board-native");
    let relief = native || args.first().is_some_and(|mode| mode == "relief-board");
    if !(5..=if relief { 9 } else { 7 }).contains(&args.len())
        || (!relief && args[0] != "material-board")
    {
        return Err("usage: resina-lab <material-board|relief-board|relief-board-native> <scenes.json> <font-file> <font-family> <output.png> [device-scale=1] [text-scale=1] [relief-kind=convex] [relief-state=rest]".into());
    }
    let scale: f32 = args.get(5).map_or(Ok(1.0), |v| v.parse())?;
    let text_scale: f64 = args.get(6).map_or(Ok(1.0), |v| v.parse())?;
    let source = fs::read_to_string(&args[1])
        .map_err(|error| format!("cannot read scene catalog {:?}: {error}", args[1]))?;
    let data = fs::read(&args[2])
        .map_err(|error| format!("cannot read font file {:?}: {error}", args[2]))?;
    board::validate_font(&data, &args[3])?;
    let native_font = native.then(|| data.clone());
    guido::load_font(data);
    let family = guido::widgets::font::FontFamily::name(&args[3]);
    let board = if relief {
        relief_board::prepare(
            &source,
            family,
            scale,
            text_scale,
            args.get(7).map_or("convex", String::as_str),
            args.get(8).map_or("rest", String::as_str),
        )?
    } else {
        board::prepare(&source, family, scale, text_scale)?
    };
    let native_dimensions = if native {
        Some(native_board::dimensions(&board)?)
    } else {
        None
    };
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
        "{}: {}x{}, {}",
        path.display(),
        board.width,
        board.height,
        if relief {
            "10 relief specimens"
        } else {
            "16 material specimens"
        }
    );
    if let Some((width, height)) = native_dimensions {
        guido::load_font(native_font.ok_or("native font bytes are absent")?);
        println!(
            "RESINA_RELIEF_BOARD_PREPARED {}",
            serde_json::json!({"scale":scale,"textScale":text_scale,"width":board.width,"height":board.height,"logicalWidth":width,"logicalHeight":height,"reliefKind":args.get(7).map_or("convex", String::as_str),"reliefState":args.get(8).map_or("rest",String::as_str)})
        );
        native_board::present(board)?;
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let name = if matches!(
                std::env::args().nth(1).as_deref(),
                Some("relief-board" | "relief-board-native")
            ) {
                "relief board"
            } else {
                "material board"
            };
            eprintln!("{name}: {error}");
            ExitCode::FAILURE
        }
    }
}
