use resina_tokens::resolve_token_source;
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

const USAGE: &str = "Usage: resina-token-resolve <path|->\nPass - to read UTF-8 JSON from stdin.";

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
    let tokens = resolve_token_source(&source)?;
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
