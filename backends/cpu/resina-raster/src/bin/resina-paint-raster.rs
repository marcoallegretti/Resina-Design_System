mod common;

use resina_raster::render_surface_paint;
use resina_resolver::resolve_surface_paint_source;
use std::process::ExitCode;

const USAGE: &str = "Usage: resina-paint-raster <path|-> <width> <height> <origin-x> <origin-y> <pixels-per-unit> <samples-per-axis>\nWrites an RGBA8 sRGB PNG to stdout. Input is a surface paint request.";

fn main() -> ExitCode {
    common::run(USAGE, resolve_surface_paint_source, render_surface_paint)
}
