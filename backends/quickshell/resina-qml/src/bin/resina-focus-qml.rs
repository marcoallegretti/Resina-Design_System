use resina_resolver::resolve_focus_ir_source;
use std::{
    env,
    error::Error,
    ffi::OsStr,
    fs,
    io::{self, Read, Write},
    process::ExitCode,
};

const USAGE: &str = "resina-focus-qml <path|->";
const MAX_REQUEST_BYTES: u64 = 1024 * 1024;

fn main() -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments.len() == 1 && arguments[0] == OsStr::new("--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if arguments.len() != 1 {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    match execute(&arguments[0]) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn execute(path: &OsStr) -> Result<(), Box<dyn Error>> {
    let reader: Box<dyn Read> = if path == OsStr::new("-") {
        Box::new(io::stdin())
    } else {
        Box::new(fs::File::open(path)?)
    };
    let mut source = Vec::new();
    reader
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut source)?;
    if source.len() as u64 > MAX_REQUEST_BYTES {
        return Err(
            io::Error::new(io::ErrorKind::InvalidData, "request size limit exceeded").into(),
        );
    }
    let ir = resolve_focus_ir_source(&String::from_utf8(source)?)?;
    let qml = resina_qml::render_focus(&ir)?;
    io::stdout().lock().write_all(qml.as_bytes())?;
    Ok(())
}
