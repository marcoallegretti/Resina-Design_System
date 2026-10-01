use resina_model::ColorRole;
use resina_tokens::{ValueError, validate_resolved_value};
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorRoleFallbackErrorKind {
    MissingColor,
    InvalidFallback(ColorFallbackError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorRoleFallbackError {
    pub role: ColorRole,
    pub kind: ColorRoleFallbackErrorKind,
}

impl fmt::Display for ColorRoleFallbackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ColorRoleFallbackErrorKind::MissingColor => {
                write!(formatter, "missing {:?}", self.role)
            }
            ColorRoleFallbackErrorKind::InvalidFallback(error) => {
                write!(formatter, "{:?}: {error}", self.role)
            }
        }
    }
}

impl std::error::Error for ColorRoleFallbackError {}

pub fn resolve_semantic_color_fallbacks(
    colors: &BTreeMap<ColorRole, Value>,
) -> Result<BTreeMap<ColorRole, SrgbFallback>, Vec<ColorRoleFallbackError>> {
    let mut fallbacks = BTreeMap::new();
    let mut errors = Vec::new();
    for role in ColorRole::ALL {
        match colors.get(&role) {
            None => errors.push(ColorRoleFallbackError {
                role,
                kind: ColorRoleFallbackErrorKind::MissingColor,
            }),
            Some(value) => match resolve_srgb_fallback(value) {
                Ok(fallback) => {
                    fallbacks.insert(role, fallback);
                }
                Err(error) => errors.push(ColorRoleFallbackError {
                    role,
                    kind: ColorRoleFallbackErrorKind::InvalidFallback(error),
                }),
            },
        }
    }
    if errors.is_empty() {
        Ok(fallbacks)
    } else {
        Err(errors)
    }
}

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
    use serde_json::json;

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

    #[test]
    fn semantic_roles_reject_all_nonportable_colors_without_partial_output() {
        let mut colors: BTreeMap<_, _> = ColorRole::ALL
            .into_iter()
            .map(|role| {
                (
                    role,
                    json!({"colorSpace":"srgb","components":[0.2,0.4,0.6]}),
                )
            })
            .collect();
        assert_eq!(resolve_semantic_color_fallbacks(&colors).unwrap().len(), 19);
        colors.remove(&ColorRole::AccentSecondary);
        colors.insert(
            ColorRole::SurfaceBase,
            json!({"colorSpace":"display-p3","components":[0.2,0.4,0.6]}),
        );
        colors.insert(
            ColorRole::Focus,
            json!({"colorSpace":"srgb","components":[1,0,0],"hex":"#0000ff"}),
        );
        let errors = resolve_semantic_color_fallbacks(&colors).unwrap_err();
        assert_eq!(
            errors.iter().map(|error| error.role).collect::<Vec<_>>(),
            [
                ColorRole::AccentSecondary,
                ColorRole::SurfaceBase,
                ColorRole::Focus,
            ]
        );
        assert!(matches!(
            errors[0].kind,
            ColorRoleFallbackErrorKind::MissingColor
        ));
        assert!(matches!(
            errors[1].kind,
            ColorRoleFallbackErrorKind::InvalidFallback(ColorFallbackError::MissingHexFallback)
        ));
        assert!(matches!(
            errors[2].kind,
            ColorRoleFallbackErrorKind::InvalidFallback(ColorFallbackError::InconsistentSrgbHex)
        ));
    }
}
