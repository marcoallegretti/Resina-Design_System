mod request_cli;

fn main() -> std::process::ExitCode {
    request_cli::run(
        "Usage: resina-relief-paint <path|->\nPass - to read a UTF-8 JSON request from stdin.",
        1024 * 1024,
        resina_resolver::resolve_relief_paint_source,
    )
}
