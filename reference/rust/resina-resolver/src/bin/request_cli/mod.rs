use serde::Serialize;
use std::{
    env,
    error::Error,
    ffi::{OsStr, OsString},
    fs,
    io::{self, Read, Write},
    process::ExitCode,
};

pub(super) fn run<T: Serialize, E: Error + 'static>(
    usage: &str,
    max_request_bytes: u64,
    resolve_source: fn(&str) -> Result<T, E>,
) -> ExitCode {
    let mut arguments = env::args_os().skip(1);
    match (arguments.next(), arguments.next()) {
        (Some(argument), None) if argument == OsStr::new("--help") => {
            println!("{usage}");
            ExitCode::SUCCESS
        }
        (Some(path), None) => match resolve_request(path, max_request_bytes, resolve_source) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("{usage}");
            ExitCode::from(2)
        }
    }
}

fn resolve_request<T: Serialize, E: Error + 'static>(
    path: OsString,
    max_request_bytes: u64,
    resolve_source: fn(&str) -> Result<T, E>,
) -> Result<(), Box<dyn Error>> {
    let source = if path == OsStr::new("-") {
        read_request(io::stdin(), max_request_bytes)?
    } else {
        read_request(fs::File::open(&path)?, max_request_bytes)?
    };
    let result = resolve_source(&source)?;
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    serde_json::to_writer_pretty(&mut writer, &result)?;
    writeln!(writer)?;
    Ok(())
}

fn read_request(reader: impl Read, max_request_bytes: u64) -> Result<String, Box<dyn Error>> {
    let mut bytes = Vec::new();
    reader.take(max_request_bytes + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_request_bytes {
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
        let error = read_request(io::repeat(b'x'), 1024 * 1024).unwrap_err();
        assert!(error.to_string().contains("request size limit exceeded"));
    }

    #[test]
    fn request_reader_consumes_only_one_byte_beyond_the_limit() {
        let mut reader = io::repeat(b'x').take(16);
        let error = read_request(&mut reader, 4).unwrap_err();
        assert!(error.to_string().contains("request size limit exceeded"));
        assert_eq!(reader.limit(), 11);
    }
}
