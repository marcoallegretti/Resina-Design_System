mod common;

use resina_raster::render_material_relief_paint;
use resina_resolver::resolve_material_relief_paint_source;
use std::process::ExitCode;

const USAGE: &str = "Usage: resina-material-relief-raster <path|-> <width> <height> <origin-x> <origin-y> <pixels-per-unit> <samples-per-axis>\nWrites an RGBA8 sRGB PNG to stdout. Input is a material-lit relief paint request.";

fn main() -> ExitCode {
    common::run(
        USAGE,
        resolve_material_relief_paint_source,
        |result, viewport, samples| render_material_relief_paint(&result.paint, viewport, samples),
    )
}
