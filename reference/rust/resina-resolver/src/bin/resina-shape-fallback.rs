mod request_cli;

use resina_resolver::resolve_shape_fallback_source;
use std::process::ExitCode;

const USAGE: &str =
    "Usage: resina-shape-fallback <path|->\nPass - to read a UTF-8 JSON request from stdin.";
const MAX_REQUEST_BYTES: u64 = 1024 * 1024;

fn main() -> ExitCode {
    request_cli::run(USAGE, MAX_REQUEST_BYTES, resolve_shape_fallback_source)
}
