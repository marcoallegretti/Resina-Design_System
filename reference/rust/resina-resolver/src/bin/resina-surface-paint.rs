use resina_resolver::resolve_surface_paint_source;
use std::{
    env,
    error::Error,
    ffi::{OsStr, OsString},
    fs,
    io::{self, Read, Write},
    process::ExitCode,
};

const USAGE: &str =
    "Usage: resina-surface-paint <path|->\nPass - to read a UTF-8 JSON request from stdin.";
const MAX_REQUEST_BYTES: u64 = 1024 * 1024;

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
        read_request(io::stdin())?
    } else {
        read_request(fs::File::open(&path)?)?
    };
    let result = resolve_surface_paint_source(&source)?;
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    serde_json::to_writer_pretty(&mut writer, &result)?;
    writeln!(writer)?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_reader_rejects_oversized_input() {
        let error = read_request(io::repeat(b'x')).unwrap_err();
        assert!(error.to_string().contains("request size limit exceeded"));
    }
}
