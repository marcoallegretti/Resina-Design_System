mod request_cli;

fn main() -> std::process::ExitCode {
    request_cli::run(
        "Usage: resina-surface-relief <path|->\nPass - to read a UTF-8 JSON request from stdin.",
        1024 * 1024,
        resina_resolver::resolve_surface_relief_source,
    )
}
