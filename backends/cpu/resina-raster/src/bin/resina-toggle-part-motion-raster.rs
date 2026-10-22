mod common;

use resina_raster::render_lit_control_paint;
use resina_resolver::resolve_toggle_part_motion_source;
use std::process::ExitCode;

const USAGE: &str = "Usage: resina-toggle-part-motion-raster <path|-> <width> <height> <origin-x> <origin-y> <pixels-per-unit> <samples-per-axis>\nWrites an RGBA8 sRGB PNG to stdout. Input is a toggle part motion request.";

fn main() -> ExitCode {
    common::run(
        USAGE,
        resolve_toggle_part_motion_source,
        |part, viewport, samples| {
            render_lit_control_paint(part.part_paint().paint(), viewport, samples)
        },
    )
}
