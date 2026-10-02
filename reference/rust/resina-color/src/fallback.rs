use crate::{
    ColorConversionError, a98_rgb_to_extended_srgb, display_p3_to_extended_srgb, hsl_to_srgb,
    hwb_to_srgb, lab_to_extended_srgb, lch_to_lab, linear_srgb_to_srgb, oklab_to_extended_srgb,
    oklch_to_oklab, prophoto_rgb_to_extended_srgb, rec2020_to_extended_srgb,
    xyz_d50_to_extended_srgb, xyz_d65_to_extended_srgb,
};
use resina_tokens::{ValueError, validate_resolved_value};
use serde::Serialize;
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SrgbFallback {
    color_space: &'static str,
    components: [f64; 3],
    alpha: f64,
}

impl SrgbFallback {
    pub fn components(&self) -> [f64; 3] {
        self.components
    }

    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    pub fn with_alpha(&self, alpha: f64) -> Result<Self, ColorFallbackError> {
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(ColorFallbackError::InvalidAlpha);
        }
        Ok(Self {
            color_space: self.color_space,
            components: self.components,
            alpha,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorFallbackError {
    InvalidValue(ValueError),
    InvalidAlpha,
    MissingHexFallback,
    InconsistentSrgbHex,
    MalformedHexFallback,
    OklabConversion(ColorConversionError),
    OutOfGamutOklab,
    OklchConversion(ColorConversionError),
    OutOfGamutOklch,
    LabConversion(ColorConversionError),
    OutOfGamutLab,
    LchConversion(ColorConversionError),
    OutOfGamutLch,
    DirectConversion(ColorConversionError),
    OutOfGamutDirectColor,
    XyzConversion(ColorConversionError),
    OutOfGamutXyz,
    RgbConversion(ColorConversionError),
    OutOfGamutRgb,
}

impl fmt::Display for ColorFallbackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue(error) => write!(formatter, "invalid color value: {error}"),
            Self::InvalidAlpha => formatter.write_str("alpha must be finite and in [0, 1]"),
            Self::MissingHexFallback => {
                formatter.write_str("sRGB hex fallback required for this color")
            }
            Self::InconsistentSrgbHex => {
                formatter.write_str("sRGB hex fallback differs from source components")
            }
            Self::MalformedHexFallback => formatter.write_str("malformed sRGB hex fallback"),
            Self::OklabConversion(error) => write!(formatter, "Oklab conversion failed: {error}"),
            Self::OutOfGamutOklab => {
                formatter.write_str("Oklab color converts outside the sRGB gamut")
            }
            Self::OklchConversion(error) => write!(formatter, "Oklch conversion failed: {error}"),
            Self::OutOfGamutOklch => {
                formatter.write_str("Oklch color converts outside the sRGB gamut")
            }
            Self::LabConversion(error) => write!(formatter, "Lab conversion failed: {error}"),
            Self::OutOfGamutLab => formatter.write_str("Lab color converts outside the sRGB gamut"),
            Self::LchConversion(error) => write!(formatter, "LCH conversion failed: {error}"),
            Self::OutOfGamutLch => formatter.write_str("LCH color converts outside the sRGB gamut"),
            Self::DirectConversion(error) => write!(formatter, "color conversion failed: {error}"),
            Self::OutOfGamutDirectColor => {
                formatter.write_str("direct color conversion outside the sRGB gamut")
            }
            Self::XyzConversion(error) => write!(formatter, "XYZ conversion failed: {error}"),
            Self::OutOfGamutXyz => formatter.write_str("XYZ color converts outside the sRGB gamut"),
            Self::RgbConversion(error) => write!(formatter, "RGB conversion failed: {error}"),
            Self::OutOfGamutRgb => formatter.write_str("RGB color converts outside the sRGB gamut"),
        }
    }
}

impl std::error::Error for ColorFallbackError {}

pub fn resolve_srgb_fallback(value: &Value) -> Result<SrgbFallback, ColorFallbackError> {
    validate_resolved_value("color", value).map_err(ColorFallbackError::InvalidValue)?;
    let alpha = value["alpha"].as_f64().unwrap_or(1.0);
    let numeric_components = value["components"]
        .as_array()
        .and_then(|items| Some([items[0].as_f64()?, items[1].as_f64()?, items[2].as_f64()?]));
    let numeric_srgb = if value["colorSpace"] == "srgb" {
        numeric_components
    } else {
        None
    };
    let authored_hex = value["hex"].as_str().map(decode_hex).transpose()?;
    if value["colorSpace"] == "srgb"
        && let Some(fallback) = authored_hex
        && value["components"].as_array().is_some_and(|items| {
            items.iter().zip(fallback).any(|(source, channel)| {
                source
                    .as_f64()
                    .is_some_and(|source| (source - channel).abs() > 0.5 / 255.0 + 1e-12)
            })
        })
    {
        return Err(ColorFallbackError::InconsistentSrgbHex);
    }
    let components = match (numeric_srgb, authored_hex) {
        (Some(source), _) => source,
        (None, Some(fallback)) => fallback,
        (None, None) if value["colorSpace"] == "oklab" || value["colorSpace"] == "oklch" => {
            let Some(source) = numeric_components else {
                return Err(ColorFallbackError::MissingHexFallback);
            };
            let converted = if value["colorSpace"] == "oklab" {
                oklab_to_extended_srgb(source).map_err(ColorFallbackError::OklabConversion)?
            } else {
                let oklab = oklch_to_oklab(source).map_err(ColorFallbackError::OklchConversion)?;
                oklab_to_extended_srgb(oklab).map_err(ColorFallbackError::OklchConversion)?
            };
            let gamut_error = if value["colorSpace"] == "oklab" {
                ColorFallbackError::OutOfGamutOklab
            } else {
                ColorFallbackError::OutOfGamutOklch
            };
            portable_components(converted, gamut_error)?
        }
        (None, None) if value["colorSpace"] == "lab" || value["colorSpace"] == "lch" => {
            let Some(source) = numeric_components else {
                return Err(ColorFallbackError::MissingHexFallback);
            };
            let converted = if value["colorSpace"] == "lab" {
                lab_to_extended_srgb(source).map_err(ColorFallbackError::LabConversion)?
            } else {
                let lab = lch_to_lab(source).map_err(ColorFallbackError::LchConversion)?;
                lab_to_extended_srgb(lab).map_err(ColorFallbackError::LchConversion)?
            };
            let gamut_error = if value["colorSpace"] == "lab" {
                ColorFallbackError::OutOfGamutLab
            } else {
                ColorFallbackError::OutOfGamutLch
            };
            portable_components(converted, gamut_error)?
        }
        (None, None)
            if matches!(
                value["colorSpace"].as_str(),
                Some("srgb-linear" | "hsl" | "hwb")
            ) =>
        {
            let Some(source) = numeric_components else {
                return Err(ColorFallbackError::MissingHexFallback);
            };
            let converted = match value["colorSpace"].as_str().unwrap() {
                "srgb-linear" => linear_srgb_to_srgb(source),
                "hsl" => hsl_to_srgb(source),
                "hwb" => hwb_to_srgb(source),
                _ => return Err(ColorFallbackError::MissingHexFallback),
            }
            .map_err(ColorFallbackError::DirectConversion)?;
            portable_components(converted, ColorFallbackError::OutOfGamutDirectColor)?
        }
        (None, None) if value["colorSpace"] == "xyz-d65" || value["colorSpace"] == "xyz-d50" => {
            let Some(source) = numeric_components else {
                return Err(ColorFallbackError::MissingHexFallback);
            };
            let converted = if value["colorSpace"] == "xyz-d65" {
                xyz_d65_to_extended_srgb(source)
            } else {
                xyz_d50_to_extended_srgb(source)
            }
            .map_err(ColorFallbackError::XyzConversion)?;
            portable_components(converted, ColorFallbackError::OutOfGamutXyz)?
        }
        (None, None)
            if matches!(
                value["colorSpace"].as_str(),
                Some("display-p3" | "a98-rgb" | "prophoto-rgb" | "rec2020")
            ) =>
        {
            let Some(source) = numeric_components else {
                return Err(ColorFallbackError::MissingHexFallback);
            };
            let converted = match value["colorSpace"].as_str().unwrap() {
                "display-p3" => display_p3_to_extended_srgb(source),
                "a98-rgb" => a98_rgb_to_extended_srgb(source),
                "prophoto-rgb" => prophoto_rgb_to_extended_srgb(source),
                "rec2020" => rec2020_to_extended_srgb(source),
                _ => unreachable!(),
            }
            .map_err(ColorFallbackError::RgbConversion)?;
            portable_components(converted, ColorFallbackError::OutOfGamutRgb)?
        }
        (None, None) => return Err(ColorFallbackError::MissingHexFallback),
    };
    Ok(SrgbFallback {
        color_space: "srgb",
        components,
        alpha,
    })
}

fn portable_components(
    components: [f64; 3],
    gamut_error: ColorFallbackError,
) -> Result<[f64; 3], ColorFallbackError> {
    const ROUNDING_TOLERANCE: f64 = 1e-12;
    if components
        .iter()
        .any(|channel| !(-ROUNDING_TOLERANCE..=1.0 + ROUNDING_TOLERANCE).contains(channel))
    {
        return Err(gamut_error);
    }
    Ok(components.map(|channel| channel.clamp(0.0, 1.0)))
}

fn decode_hex(hex: &str) -> Result<[f64; 3], ColorFallbackError> {
    let mut components = [0.0; 3];
    for (index, component) in components.iter_mut().enumerate() {
        let start = 1 + index * 2;
        let channel = u8::from_str_radix(&hex[start..start + 2], 16)
            .map_err(|_| ColorFallbackError::MalformedHexFallback)?;
        *component = f64::from(channel) / 255.0;
    }
    Ok(components)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_fallback_alpha_preserves_channels_and_rejects_invalid_values() {
        let color = resolve_srgb_fallback(&serde_json::json!({
            "colorSpace":"srgb","components":[0.15,0.2,0.3]
        }))
        .unwrap();
        let tinted = color.with_alpha(0.35).unwrap();
        assert_eq!(tinted.components(), color.components());
        assert_eq!(tinted.alpha(), 0.35);
        for invalid in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            assert_eq!(
                color.with_alpha(invalid),
                Err(ColorFallbackError::InvalidAlpha)
            );
        }
    }

    #[test]
    fn srgb_fallback_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/color/srgb-fallback-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = resolve_srgb_fallback(&vector["value"]);
            if let Some(expected) = vector.get("expected") {
                let actual = serde_json::to_value(result.unwrap()).unwrap();
                if let Some(tolerance) = vector.get("tolerance") {
                    let tolerance = tolerance.as_f64().unwrap();
                    assert_eq!(actual["colorSpace"], expected["colorSpace"]);
                    assert_eq!(
                        actual["alpha"].as_f64().unwrap(),
                        expected["alpha"].as_f64().unwrap()
                    );
                    for index in 0..3 {
                        let actual_channel = actual["components"][index].as_f64().unwrap();
                        let expected_channel = expected["components"][index].as_f64().unwrap();
                        assert!(
                            (actual_channel - expected_channel).abs() <= tolerance,
                            "{}: channel {index}",
                            vector["name"]
                        );
                    }
                } else {
                    assert_eq!(actual, *expected, "{}", vector["name"]);
                }
            } else {
                let error = result.unwrap_err();
                let kind = match error {
                    ColorFallbackError::InvalidValue(_) => "InvalidValue",
                    ColorFallbackError::InvalidAlpha => "InvalidAlpha",
                    ColorFallbackError::MissingHexFallback => "MissingHexFallback",
                    ColorFallbackError::InconsistentSrgbHex => "InconsistentSrgbHex",
                    ColorFallbackError::MalformedHexFallback => "MalformedHexFallback",
                    ColorFallbackError::OklabConversion(_) => "OklabConversion",
                    ColorFallbackError::OutOfGamutOklab => "OutOfGamutOklab",
                    ColorFallbackError::OklchConversion(_) => "OklchConversion",
                    ColorFallbackError::OutOfGamutOklch => "OutOfGamutOklch",
                    ColorFallbackError::LabConversion(_) => "LabConversion",
                    ColorFallbackError::OutOfGamutLab => "OutOfGamutLab",
                    ColorFallbackError::LchConversion(_) => "LchConversion",
                    ColorFallbackError::OutOfGamutLch => "OutOfGamutLch",
                    ColorFallbackError::DirectConversion(_) => "DirectConversion",
                    ColorFallbackError::OutOfGamutDirectColor => "OutOfGamutDirectColor",
                    ColorFallbackError::XyzConversion(_) => "XyzConversion",
                    ColorFallbackError::OutOfGamutXyz => "OutOfGamutXyz",
                    ColorFallbackError::RgbConversion(_) => "RgbConversion",
                    ColorFallbackError::OutOfGamutRgb => "OutOfGamutRgb",
                };
                assert_eq!(
                    kind,
                    vector["error"].as_str().unwrap(),
                    "{}",
                    vector["name"]
                );
            }
        }
    }
}
