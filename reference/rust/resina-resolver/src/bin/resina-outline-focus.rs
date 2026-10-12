mod request_cli;

use resina_resolver::resolve_outline_focus_source;
use std::process::ExitCode;

fn main() -> ExitCode {
    request_cli::run(
        "Usage: resina-outline-focus <path|->\nPass - to read a UTF-8 JSON request from stdin.",
        1024 * 1024,
        resolve_outline_focus_source,
    )
}
