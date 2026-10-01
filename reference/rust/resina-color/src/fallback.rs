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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorFallbackError {
    InvalidValue(ValueError),
    MissingHexFallback,
    InconsistentSrgbHex,
    MalformedHexFallback,
}

impl fmt::Display for ColorFallbackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue(error) => write!(formatter, "invalid color value: {error}"),
            Self::MissingHexFallback => {
                formatter.write_str("sRGB hex fallback required for this color")
            }
            Self::InconsistentSrgbHex => {
                formatter.write_str("sRGB hex fallback differs from source components")
            }
            Self::MalformedHexFallback => formatter.write_str("malformed sRGB hex fallback"),
        }
    }
}

impl std::error::Error for ColorFallbackError {}

pub fn resolve_srgb_fallback(value: &Value) -> Result<SrgbFallback, ColorFallbackError> {
    validate_resolved_value("color", value).map_err(ColorFallbackError::InvalidValue)?;
    let alpha = value["alpha"].as_f64().unwrap_or(1.0);
    let numeric_srgb = if value["colorSpace"] == "srgb" {
        value["components"]
            .as_array()
            .and_then(|items| Some([items[0].as_f64()?, items[1].as_f64()?, items[2].as_f64()?]))
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
        (Some(source), Some(_)) => source,
        (Some(source), None) => source,
        (None, Some(fallback)) => fallback,
        (None, None) => return Err(ColorFallbackError::MissingHexFallback),
    };
    Ok(SrgbFallback {
        color_space: "srgb",
        components,
        alpha,
    })
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
    fn srgb_fallback_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/color/srgb-fallback-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = resolve_srgb_fallback(&vector["value"]);
            if let Some(expected) = vector.get("expected") {
                assert_eq!(
                    serde_json::to_value(result.unwrap()).unwrap(),
                    *expected,
                    "{}",
                    vector["name"]
                );
            } else {
                let error = result.unwrap_err();
                let kind = match error {
                    ColorFallbackError::InvalidValue(_) => "InvalidValue",
                    ColorFallbackError::MissingHexFallback => "MissingHexFallback",
                    ColorFallbackError::InconsistentSrgbHex => "InconsistentSrgbHex",
                    ColorFallbackError::MalformedHexFallback => "MalformedHexFallback",
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
