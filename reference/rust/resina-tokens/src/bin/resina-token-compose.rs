use resina_tokens::{parse_token_document, resolve_resolver_module_source};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    env,
    error::Error,
    ffi::{OsStr, OsString},
    fs,
    io::{self, Read, Write},
    process::ExitCode,
};

const USAGE: &str =
    "Usage: resina-token-compose <path|->\nPass - to read a UTF-8 JSON request from stdin.";
const MAX_REQUEST_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    resolver: String,
    input: String,
    external_sources: BTreeMap<String, String>,
}

fn main() -> ExitCode {
    let mut arguments = env::args_os().skip(1);
    match (arguments.next(), arguments.next()) {
        (Some(argument), None) if argument == OsStr::new("--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        (Some(path), None) => match compose(path) {
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

fn compose(path: OsString) -> Result<(), Box<dyn Error>> {
    let source = if path == OsStr::new("-") {
        read_request(io::stdin())?
    } else {
        read_request(fs::File::open(&path)?)?
    };
    let document = parse_token_document(&source)?;
    let request: Request = serde_json::from_value(document)?;
    if request.schema_version != "0.1.0" {
        return Err(
            io::Error::new(io::ErrorKind::InvalidData, "schemaVersion must be 0.1.0").into(),
        );
    }
    let tokens = resolve_resolver_module_source(
        &request.resolver,
        &request.input,
        &request.external_sources,
    )?;
    let output: BTreeMap<String, Value> = tokens
        .into_iter()
        .map(|(path, token)| {
            (
                path,
                serde_json::json!({"token_type": token.token_type, "value": token.value}),
            )
        })
        .collect();
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    serde_json::to_writer_pretty(&mut writer, &output)?;
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
