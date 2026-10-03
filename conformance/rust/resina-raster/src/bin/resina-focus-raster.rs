mod common;

use resina_raster::render_focus;
use resina_resolver::resolve_focus_ir_source;
use std::process::ExitCode;

const USAGE: &str = "Usage: resina-focus-raster <path|-> <width> <height> <origin-x> <origin-y> <pixels-per-unit> <samples-per-axis>\nWrites an RGBA8 sRGB PNG to stdout. Input is a focus IR request.";

fn main() -> ExitCode {
    common::run(USAGE, resolve_focus_ir_source, render_focus)
}
