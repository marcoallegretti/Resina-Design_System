#![allow(dead_code)] // each test crate that includes this file uses a subset
use guido::widgets::font::FontFamily;
use std::path::PathBuf;

pub const FAMILY: &str = "Inter";

fn directory() -> PathBuf {
    PathBuf::from(
        std::env::var_os("RESINA_LABEL_FONT_DIR")
            .expect("RESINA_LABEL_FONT_DIR must name the pinned static masters"),
    )
}

/// Every pinned static master as family, authored weight and file name;
/// `tools/conformance_font.json` holds their SHA-256.
pub const FACES: [(&str, u16, &str); 9] = [
    (FAMILY, 400, "Inter-Regular.otf"),
    (FAMILY, 500, "Inter-Medium.otf"),
    (FAMILY, 600, "Inter-SemiBold.otf"),
    ("Noto Sans Arabic", 400, "NotoSansArabic-Regular.ttf"),
    ("Noto Sans Arabic", 500, "NotoSansArabic-Medium.ttf"),
    ("Noto Sans Arabic", 600, "NotoSansArabic-SemiBold.ttf"),
    ("Noto Sans Hebrew", 400, "NotoSansHebrew-Regular.ttf"),
    ("Noto Sans Hebrew", 500, "NotoSansHebrew-Medium.ttf"),
    ("Noto Sans Hebrew", 600, "NotoSansHebrew-SemiBold.ttf"),
];

/// Every pinned master with its path, in `FACES` order.
pub fn faces() -> Vec<(&'static str, u16, PathBuf)> {
    FACES
        .into_iter()
        .map(|(family, weight, file)| (family, weight, directory().join(file)))
        .collect()
}

/// The pinned master of a family at an authored weight.
pub fn face(family: &str, weight: u16) -> PathBuf {
    faces()
        .into_iter()
        .find(|face| face.0 == family && face.1 == weight)
        .unwrap_or_else(|| panic!("no pinned {family} master at weight {weight}"))
        .2
}

/// The pinned Inter static masters by weight.
pub fn masters() -> Vec<PathBuf> {
    faces()
        .into_iter()
        .filter(|face| face.0 == FAMILY)
        .map(|face| face.2)
        .collect()
}

/// Registers every master before measurement, so authored weights resolve to real faces
/// and Arabic and Hebrew text falls back to the pinned script faces.
pub fn load() -> FontFamily {
    for (_, _, path) in faces() {
        let data = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", path.display()));
        guido::load_font(data);
    }
    FontFamily::name(FAMILY)
}
