mod request_cli;
use resina_resolver::resolve_material_relief_paint_source;
use std::process::ExitCode;

const USAGE: &str =
    "Usage: resina-material-relief-paint <path|->\nPass - to read a UTF-8 JSON request from stdin.";

fn main() -> ExitCode {
    request_cli::run(USAGE, 1024 * 1024, resolve_material_relief_paint_source)
}
