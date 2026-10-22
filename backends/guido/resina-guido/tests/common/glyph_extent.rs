use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight};
use std::sync::Arc;

use super::conformance_font::{self, FAMILY};

/// How far the shaped glyphs of a text reach below the room that a line leaves under
/// the baseline, in em, from the pinned masters' own metrics.
///
/// A line of `line_height` em centers the ascent and descent of the face that draws a
/// glyph, so the room under its baseline is half of what the face leaves of the line
/// plus its descent. The text is shaped unwrapped in an `en-US` standalone
/// `FontSystem` over the pinned masters only, as `label_evidence::resolve_text` does.
/// Glyphs on any line count, because the line a glyph lands on depends on wrapping.
pub fn descent_beyond_line_em(text: &str, weight: u16, line_height: f64) -> f64 {
    let mut fonts = fontdb::Database::new();
    for (_, _, path) in conformance_font::faces() {
        let data = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", path.display()));
        fonts.load_font_source(fontdb::Source::Binary(Arc::new(data)));
    }
    let mut system = FontSystem::new_with_locale_and_db("en-US".into(), fonts);
    let mut buffer = Buffer::new(&mut system, Metrics::new(20.0, 28.0));
    buffer.set_size(None, None);
    let attrs = Attrs::new()
        .family(Family::Name(FAMILY))
        .weight(Weight(weight));
    buffer.set_text(text, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut system, true);
    let mut beyond = 0.0f64;
    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            let (reach, room) = system
                .db()
                .with_face_data(glyph.font_id, |data, index| {
                    let face = ttf_parser::Face::parse(data, index).unwrap();
                    let em = f64::from(face.units_per_em());
                    let (ascent, descent) = (
                        f64::from(face.ascender()) / em,
                        -f64::from(face.descender()) / em,
                    );
                    let reach = face
                        .glyph_bounding_box(ttf_parser::GlyphId(glyph.glyph_id))
                        .map_or(0.0, |bounds| -f64::from(bounds.y_min) / em);
                    (reach, (line_height - ascent - descent) / 2.0 + descent)
                })
                .expect("a shaped glyph names a registered face");
            beyond = beyond.max(reach - room);
        }
    }
    beyond
}
