mod request_cli;

use resina_resolver::resolve_surface_outline_source;
use std::process::ExitCode;

fn main() -> ExitCode {
    request_cli::run(
        "Usage: resina-surface-outline <path|->\nPass - to read a UTF-8 JSON request from stdin.",
        1024 * 1024,
        resolve_surface_outline_source,
    )
}
