use crate::{SrgbFallback, oklab::linearize_srgb_component};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContrastError {
    TranslucentForeground,
    TranslucentBackground,
}

impl fmt::Display for ContrastError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TranslucentForeground => formatter.write_str("foreground is translucent"),
            Self::TranslucentBackground => formatter.write_str("background is translucent"),
        }
    }
}

impl std::error::Error for ContrastError {}

pub fn opaque_contrast_ratio(
    foreground: &SrgbFallback,
    background: &SrgbFallback,
) -> Result<f64, ContrastError> {
    if foreground.alpha() != 1.0 {
        return Err(ContrastError::TranslucentForeground);
    }
    if background.alpha() != 1.0 {
        return Err(ContrastError::TranslucentBackground);
    }

    let foreground_luminance = relative_luminance(foreground.components());
    let background_luminance = relative_luminance(background.components());
    let lighter = foreground_luminance.max(background_luminance);
    let darker = foreground_luminance.min(background_luminance);
    Ok((lighter + 0.05) / (darker + 0.05))
}

fn relative_luminance(components: [f64; 3]) -> f64 {
    let [red, green, blue] = components.map(linearize_srgb_component);
    0.2126 * red + 0.7152 * green + 0.0722 * blue
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve_srgb_fallback;
    use serde_json::Value;

    #[test]
    fn contrast_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/color/contrast-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let foreground = resolve_srgb_fallback(&vector["foreground"]).unwrap();
            let background = resolve_srgb_fallback(&vector["background"]).unwrap();
            let result = opaque_contrast_ratio(&foreground, &background);
            if let Some(expected) = vector.get("expected") {
                let actual = result.unwrap();
                let expected = expected.as_f64().unwrap();
                assert!(
                    (actual - expected).abs() < 1e-12,
                    "{}: {actual}",
                    vector["name"]
                );
            } else {
                let actual = result.unwrap_err();
                let expected = match vector["error"].as_str().unwrap() {
                    "TranslucentForeground" => ContrastError::TranslucentForeground,
                    "TranslucentBackground" => ContrastError::TranslucentBackground,
                    other => panic!("unknown error: {other}"),
                };
                assert_eq!(actual, expected, "{}", vector["name"]);
            }
        }
    }
}
