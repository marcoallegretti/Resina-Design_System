mod common;

use resina_raster::render_surface;
use resina_resolver::resolve_opaque_surface_source;
use std::process::ExitCode;

const USAGE: &str = "Usage: resina-surface-raster <path|-> <width> <height> <origin-x> <origin-y> <pixels-per-unit> <samples-per-axis>\nWrites an RGBA8 sRGB PNG to stdout. Input is an opaque surface request.";

fn main() -> ExitCode {
    common::run(USAGE, resolve_opaque_surface_source, render_surface)
}
