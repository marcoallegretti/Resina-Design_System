use resina_model::PhysicalVector;
use resina_raster::{RasterError, RasterImage, Viewport};
use std::{
    env,
    error::Error,
    ffi::OsStr,
    fs,
    io::{self, Read},
    process::ExitCode,
    str::FromStr,
};

const MAX_REQUEST_BYTES: u64 = 1024 * 1024;

pub fn run<T, E: Error + 'static>(
    usage: &str,
    resolve: fn(&str) -> Result<T, E>,
    render: fn(&T, Viewport, u8) -> Result<RasterImage, RasterError>,
) -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments.len() == 1 && arguments[0] == OsStr::new("--help") {
        println!("{usage}");
        return ExitCode::SUCCESS;
    }
    if arguments.len() != 7 {
        eprintln!("{usage}");
        return ExitCode::from(2);
    }
    match execute(&arguments, resolve, render) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn execute<T, E: Error + 'static>(
    arguments: &[std::ffi::OsString],
    resolve: fn(&str) -> Result<T, E>,
    render: fn(&T, Viewport, u8) -> Result<RasterImage, RasterError>,
) -> Result<(), Box<dyn Error>> {
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
    let resolved = resolve(&source)?;
    let image = render(&resolved, viewport, samples)?;
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
