use crate::{SrgbFallback, conversion::linearize_srgb_component};
use serde::Serialize;
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpaqueSrgbRange {
    lower: SrgbFallback,
    upper: SrgbFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContrastRangeError {
    TranslucentLower,
    TranslucentUpper,
    InvertedChannel(usize),
}

impl fmt::Display for ContrastRangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TranslucentLower => formatter.write_str("lower range bound must be opaque"),
            Self::TranslucentUpper => formatter.write_str("upper range bound must be opaque"),
            Self::InvertedChannel(index) => {
                write!(formatter, "lower exceeds upper at sRGB channel {index}")
            }
        }
    }
}

impl std::error::Error for ContrastRangeError {}

impl OpaqueSrgbRange {
    pub fn try_new(lower: SrgbFallback, upper: SrgbFallback) -> Result<Self, ContrastRangeError> {
        if lower.alpha() != 1.0 {
            return Err(ContrastRangeError::TranslucentLower);
        }
        if upper.alpha() != 1.0 {
            return Err(ContrastRangeError::TranslucentUpper);
        }
        for (index, (low, high)) in lower
            .components()
            .into_iter()
            .zip(upper.components())
            .enumerate()
        {
            if low > high {
                return Err(ContrastRangeError::InvertedChannel(index));
            }
        }
        Ok(Self { lower, upper })
    }

    pub fn lower(&self) -> &SrgbFallback {
        &self.lower
    }

    pub fn upper(&self) -> &SrgbFallback {
        &self.upper
    }
}

pub fn opaque_contrast_over_range(
    foreground: &SrgbFallback,
    background: &OpaqueSrgbRange,
) -> Result<f64, ContrastError> {
    if foreground.alpha() != 1.0 {
        return Err(ContrastError::TranslucentForeground);
    }
    let foreground_luminance = relative_luminance(foreground.components());
    let lower_luminance = relative_luminance(background.lower.components());
    let upper_luminance = relative_luminance(background.upper.components());
    let closest_luminance = foreground_luminance.clamp(lower_luminance, upper_luminance);
    Ok(luminance_contrast(foreground_luminance, closest_luminance))
}

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
    Ok(luminance_contrast(
        foreground_luminance,
        background_luminance,
    ))
}

fn luminance_contrast(foreground: f64, background: f64) -> f64 {
    let lighter = foreground.max(background);
    let darker = foreground.min(background);
    (lighter + 0.05) / (darker + 0.05)
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
