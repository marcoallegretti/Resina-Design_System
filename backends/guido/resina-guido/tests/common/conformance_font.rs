use guido::widgets::font::FontFamily;
use std::path::PathBuf;

pub const FAMILY: &str = "Inter";

fn directory() -> PathBuf {
    PathBuf::from(
        std::env::var_os("RESINA_LABEL_FONT_DIR")
            .expect("RESINA_LABEL_FONT_DIR must name the pinned static masters"),
    )
}

/// The pinned Inter static masters; `tools/conformance_font.json` holds their SHA-256.
pub fn masters() -> [PathBuf; 3] {
    [
        "Inter-Regular.otf",
        "Inter-Medium.otf",
        "Inter-SemiBold.otf",
    ]
    .map(|file| directory().join(file))
}

/// The pinned Noto Sans Arabic masters, then the pinned Noto Sans Hebrew masters, by weight.
pub fn script_masters() -> [PathBuf; 6] {
    [
        "NotoSansArabic-Regular.ttf",
        "NotoSansArabic-Medium.ttf",
        "NotoSansArabic-SemiBold.ttf",
        "NotoSansHebrew-Regular.ttf",
        "NotoSansHebrew-Medium.ttf",
        "NotoSansHebrew-SemiBold.ttf",
    ]
    .map(|file| directory().join(file))
}

/// Registers every master before measurement, so authored weights resolve to real faces
/// and Arabic and Hebrew text falls back to the pinned script faces.
pub fn load() -> FontFamily {
    for path in masters().into_iter().chain(script_masters()) {
        let data = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", path.display()));
        guido::load_font(data);
    }
    FontFamily::name(FAMILY)
}
