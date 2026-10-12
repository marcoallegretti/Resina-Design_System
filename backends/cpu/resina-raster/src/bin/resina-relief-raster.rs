mod common;

use resina_raster::render_relief_paint;
use resina_resolver::resolve_relief_paint_source;
use std::process::ExitCode;

const USAGE: &str = "Usage: resina-relief-raster <path|-> <width> <height> <origin-x> <origin-y> <pixels-per-unit> <samples-per-axis>\nWrites an RGBA8 sRGB PNG to stdout. Input is an opaque relief paint request.";

fn main() -> ExitCode {
    common::run(
        USAGE,
        resolve_relief_paint_source,
        |result, viewport, samples| render_relief_paint(&result.paint, viewport, samples),
    )
}
