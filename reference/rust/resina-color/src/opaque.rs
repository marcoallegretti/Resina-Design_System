use crate::{ColorFallbackError, SrgbFallback, resolve_srgb_fallback};
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpaqueFallbackError {
    Source(ColorFallbackError),
    MissingAuthoredFallback,
    Authored(ColorFallbackError),
    TranslucentAuthoredFallback,
}

impl fmt::Display for OpaqueFallbackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "source color: {error}"),
            Self::MissingAuthoredFallback => {
                formatter.write_str("translucent source requires an authored opaque fallback")
            }
            Self::Authored(error) => write!(formatter, "authored opaque fallback: {error}"),
            Self::TranslucentAuthoredFallback => {
                formatter.write_str("authored opaque fallback must have alpha 1")
            }
        }
    }
}

impl std::error::Error for OpaqueFallbackError {}

pub fn resolve_opaque_srgb_fallback(
    source: &Value,
    authored: Option<&Value>,
) -> Result<SrgbFallback, OpaqueFallbackError> {
    let source = resolve_srgb_fallback(source).map_err(OpaqueFallbackError::Source)?;
    match authored {
        Some(value) => {
            let fallback = resolve_srgb_fallback(value).map_err(OpaqueFallbackError::Authored)?;
            if fallback.alpha() != 1.0 {
                return Err(OpaqueFallbackError::TranslucentAuthoredFallback);
            }
            Ok(fallback)
        }
        None if source.alpha() == 1.0 => Ok(source),
        None => Err(OpaqueFallbackError::MissingAuthoredFallback),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn opaque_fallback_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/color/opaque-fallback-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = resolve_opaque_srgb_fallback(&vector["source"], vector.get("authored"));
            if let Some(expected) = vector.get("expected") {
                assert_eq!(
                    serde_json::to_value(result.unwrap()).unwrap(),
                    *expected,
                    "{}",
                    vector["name"]
                );
            } else {
                let error = result.unwrap_err();
                assert_eq!(
                    format!("{error:?}"),
                    vector["error"].as_str().unwrap(),
                    "{}",
                    vector["name"]
                );
            }
        }
    }
}
