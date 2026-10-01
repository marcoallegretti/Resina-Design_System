use resina_resolver::resolve_headless_source;
use std::{
    env,
    error::Error,
    ffi::{OsStr, OsString},
    fs,
    io::{self, Read, Write},
    process::ExitCode,
};

const USAGE: &str =
    "Usage: resina-headless <path|->\nPass - to read a UTF-8 JSON request from stdin.";

fn main() -> ExitCode {
    let mut arguments = env::args_os().skip(1);
    match (arguments.next(), arguments.next()) {
        (Some(argument), None) if argument == OsStr::new("--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        (Some(path), None) => match resolve(path) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn resolve(path: OsString) -> Result<(), Box<dyn Error>> {
    let source = if path == OsStr::new("-") {
        let mut source = String::new();
        io::stdin().read_to_string(&mut source)?;
        source
    } else {
        fs::read_to_string(&path)?
    };
    let resolution = resolve_headless_source(&source)?;
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    serde_json::to_writer_pretty(&mut writer, &resolution)?;
    writeln!(writer)?;
    Ok(())
}
