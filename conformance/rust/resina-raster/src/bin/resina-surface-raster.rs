use resina_model::PhysicalVector;
use resina_raster::{Viewport, render_surface};
use resina_resolver::resolve_opaque_surface_source;
use std::{
    env,
    error::Error,
    ffi::OsStr,
    fs,
    io::{self, Read},
    process::ExitCode,
    str::FromStr,
};

const USAGE: &str = "Usage: resina-surface-raster <path|-> <width> <height> <origin-x> <origin-y> <pixels-per-unit> <samples-per-axis>\nWrites an RGBA8 sRGB PNG to stdout. Input is an opaque surface request.";
const MAX_REQUEST_BYTES: u64 = 1024 * 1024;

fn main() -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments.len() == 1 && arguments[0] == OsStr::new("--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if arguments.len() != 7 {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[std::ffi::OsString]) -> Result<(), Box<dyn Error>> {
    let viewport = Viewport {
        width: number(&arguments[1], "width")?,
        height: number(&arguments[2], "height")?,
        origin: PhysicalVector {
            x: number(&arguments[3], "origin x")?,
            y: number(&arguments[4], "origin y")?,
        },
        pixels_per_unit: number(&arguments[5], "pixels per unit")?,
    };
    let samples = number(&arguments[6], "samples per axis")?;
    let source = if arguments[0] == OsStr::new("-") {
        read_request(io::stdin())?
    } else {
        read_request(fs::File::open(&arguments[0])?)?
    };
    let surface = resolve_opaque_surface_source(&source)?;
    let image = render_surface(&surface, viewport, samples)?;
    image.write_png(io::stdout().lock())?;
    Ok(())
}

fn number<T: FromStr>(argument: &OsStr, name: &str) -> Result<T, io::Error>
where
    T::Err: std::fmt::Display,
{
    let input = argument.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, format!("{name} must be UTF-8"))
    })?;
    input.parse().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid {name}: {error}"),
        )
    })
}

fn read_request(reader: impl Read) -> Result<String, Box<dyn Error>> {
    let mut bytes = Vec::new();
    reader.take(MAX_REQUEST_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_REQUEST_BYTES {
        return Err(
            io::Error::new(io::ErrorKind::InvalidData, "request size limit exceeded").into(),
        );
    }
    Ok(String::from_utf8(bytes)?)
}
