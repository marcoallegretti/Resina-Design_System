mod common;

use resina_raster::render_surface_paint;
use resina_resolver::resolve_command_motion_source;
use std::process::ExitCode;

const USAGE: &str = "Usage: resina-command-motion-raster <path|-> <width> <height> <origin-x> <origin-y> <pixels-per-unit> <samples-per-axis>\nWrites an RGBA8 sRGB PNG to stdout. Input is a command motion request.";

fn main() -> ExitCode {
    common::run(
        USAGE,
        resolve_command_motion_source,
        |command, viewport, samples| {
            render_surface_paint(command.command().paint(), viewport, samples)
        },
    )
}
