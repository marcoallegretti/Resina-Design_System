mod common;
use resina_raster::render_surface_paint;
use resina_resolver::resolve_toggle_part_paint_source;
use std::process::ExitCode;
const USAGE: &str = "Usage: resina-toggle-part-raster <path|-> <width> <height> <origin-x> <origin-y> <pixels-per-unit> <samples-per-axis>\nWrites an RGBA8 sRGB PNG to stdout. Input is a Toggle part paint request.";
fn main() -> ExitCode {
    common::run(
        USAGE,
        resolve_toggle_part_paint_source,
        |part, viewport, samples| render_surface_paint(part.paint(), viewport, samples),
    )
}
