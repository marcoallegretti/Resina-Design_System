use resina_resolver::resolve_surface_paint_source;
use std::{
    env,
    error::Error,
    ffi::OsStr,
    fs,
    io::{self, Read, Write},
    process::ExitCode,
};

const USAGE: &str =
    "resina-paint-slint <path|-> <device-scale> <samples-per-axis> <origin-x> <origin-y>";
const MAX_REQUEST_BYTES: u64 = 1024 * 1024;

fn main() -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments.len() == 1 && arguments[0] == OsStr::new("--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if arguments.len() != 5 {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    match execute(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn execute(arguments: &[std::ffi::OsString]) -> Result<(), Box<dyn Error>> {
    let path = &arguments[0];
    let number = |index: usize, name: &str| -> Result<f64, Box<dyn Error>> {
        Ok(arguments[index]
            .to_str()
            .ok_or_else(|| format!("{name} must be UTF-8"))?
            .parse()
            .map_err(|error| format!("invalid {name}: {error}"))?)
    };
    let scale = number(1, "device scale")?;
    let samples: u8 = arguments[2]
        .to_str()
        .ok_or("samples per axis must be UTF-8")?
        .parse()
        .map_err(|error| format!("invalid samples per axis: {error}"))?;
    let origin = resina_model::PhysicalVector {
        x: number(3, "origin x")?,
        y: number(4, "origin y")?,
    };
    let reader: Box<dyn Read> = if path == OsStr::new("-") {
        Box::new(io::stdin())
    } else {
        Box::new(
            fs::File::open(path)
                .map_err(|error| format!("cannot read request {:?}: {error}", path))?,
        )
    };
    let mut source = Vec::new();
    reader
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut source)
        .map_err(|error| format!("cannot read request {:?}: {error}", path))?;
    if source.len() as u64 > MAX_REQUEST_BYTES {
        return Err(
            io::Error::new(io::ErrorKind::InvalidData, "request size limit exceeded").into(),
        );
    }
    let ir = resolve_surface_paint_source(&String::from_utf8(source)?)?;
    let slint = resina_slint::render_surface_paint(&ir, origin, scale, samples)?;
    io::stdout().lock().write_all(slint.as_bytes())?;
    Ok(())
}
