use guido::widgets::font::FontFamily;
use std::path::PathBuf;

pub const FAMILY: &str = "Inter";

/// The pinned Inter static masters; `tools/conformance_font.json` holds their SHA-256.
pub fn masters() -> [PathBuf; 3] {
    let directory = PathBuf::from(
        std::env::var_os("RESINA_LABEL_FONT_DIR")
            .expect("RESINA_LABEL_FONT_DIR must name the Inter static masters"),
    );
    [
        "Inter-Regular.otf",
        "Inter-Medium.otf",
        "Inter-SemiBold.otf",
    ]
    .map(|file| directory.join(file))
}

/// Registers every master before measurement, so authored weights resolve to real faces.
pub fn load() -> FontFamily {
    for path in masters() {
        let data = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", path.display()));
        guido::load_font(data);
    }
    FontFamily::name(FAMILY)
}
