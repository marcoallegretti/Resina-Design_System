use crate::{
    SrgbFallback, opaque_contrast_ratio,
    srgb_input::{SrgbInput, SrgbInputError},
};
use resina_color::ColorFallbackError;
use resina_model::ColorRole;
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EdgeContrastRequest {
    schema_version: String,
    outline: SrgbInput,
    outline_strong: SrgbInput,
    adjacent_color: SrgbInput,
    minimum_contrast: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeContrastResult {
    schema_version: &'static str,
    color_role: ColorRole,
    color: SrgbFallback,
    contrast_ratio: f64,
    fallback_applied: bool,
}

impl EdgeContrastResult {
    pub fn color_role(&self) -> ColorRole {
        self.color_role
    }

    pub fn color(&self) -> &SrgbFallback {
        &self.color
    }

    pub fn contrast_ratio(&self) -> f64 {
        self.contrast_ratio
    }

    pub fn fallback_applied(&self) -> bool {
        self.fallback_applied
    }
}

#[derive(Debug)]
pub enum EdgeContrastError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    InvalidColorSpace(&'static str),
    Color(&'static str, ColorFallbackError),
    InvalidMinimumContrast,
    TranslucentOutline,
    TranslucentStrongOutline,
    TranslucentAdjacentColor,
    InsufficientContrast { outline: f64, strong: f64 },
}

impl fmt::Display for EdgeContrastError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "edge request parse failed: {error}"),
            Self::Request(error) => write!(formatter, "invalid edge request: {error}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidColorSpace(field) => write!(formatter, "{field} must use sRGB"),
            Self::Color(field, error) => write!(formatter, "invalid {field}: {error}"),
            Self::InvalidMinimumContrast => {
                formatter.write_str("minimumContrast must be finite and in [1, 21]")
            }
            Self::TranslucentOutline => formatter.write_str("outline must be opaque"),
            Self::TranslucentStrongOutline => formatter.write_str("outlineStrong must be opaque"),
            Self::TranslucentAdjacentColor => formatter.write_str("adjacentColor must be opaque"),
            Self::InsufficientContrast { outline, strong } => write!(
                formatter,
                "edge contrast is insufficient: outline {outline}, outline.strong {strong}"
            ),
        }
    }
}

impl std::error::Error for EdgeContrastError {}

fn parse_color(field: &'static str, input: SrgbInput) -> Result<SrgbFallback, EdgeContrastError> {
    input.into_fallback().map_err(|error| match error {
        SrgbInputError::InvalidColorSpace => EdgeContrastError::InvalidColorSpace(field),
        SrgbInputError::Color(error) => EdgeContrastError::Color(field, error),
    })
}

pub fn resolve_edge_contrast_source(source: &str) -> Result<EdgeContrastResult, EdgeContrastError> {
    let document = parse_token_document(source).map_err(EdgeContrastError::Parse)?;
    if document
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|version| version != "0.1.0")
    {
        return Err(EdgeContrastError::UnsupportedVersion);
    }
    let request: EdgeContrastRequest =
        serde_json::from_value(document).map_err(EdgeContrastError::Request)?;
    debug_assert_eq!(request.schema_version, "0.1.0");
    let outline = parse_color("outline", request.outline)?;
    let strong = parse_color("outlineStrong", request.outline_strong)?;
    let adjacent = parse_color("adjacentColor", request.adjacent_color)?;
    resolve_edge_contrast(&outline, &strong, &adjacent, request.minimum_contrast)
}

pub fn resolve_edge_contrast(
    outline: &SrgbFallback,
    outline_strong: &SrgbFallback,
    adjacent_color: &SrgbFallback,
    minimum_contrast: f64,
) -> Result<EdgeContrastResult, EdgeContrastError> {
    if !minimum_contrast.is_finite() || !(1.0..=21.0).contains(&minimum_contrast) {
        return Err(EdgeContrastError::InvalidMinimumContrast);
    }
    if outline.alpha() != 1.0 {
        return Err(EdgeContrastError::TranslucentOutline);
    }
    if outline_strong.alpha() != 1.0 {
        return Err(EdgeContrastError::TranslucentStrongOutline);
    }
    if adjacent_color.alpha() != 1.0 {
        return Err(EdgeContrastError::TranslucentAdjacentColor);
    }
    let outline_ratio = opaque_contrast_ratio(outline, adjacent_color)
        .expect("validated outline and adjacent color are opaque");
    if outline_ratio >= minimum_contrast {
        return Ok(EdgeContrastResult {
            schema_version: "0.1.0",
            color_role: ColorRole::Outline,
            color: outline.clone(),
            contrast_ratio: outline_ratio,
            fallback_applied: false,
        });
    }
    let strong_ratio = opaque_contrast_ratio(outline_strong, adjacent_color)
        .expect("validated strong outline and adjacent color are opaque");
    if strong_ratio >= minimum_contrast {
        return Ok(EdgeContrastResult {
            schema_version: "0.1.0",
            color_role: ColorRole::OutlineStrong,
            color: outline_strong.clone(),
            contrast_ratio: strong_ratio,
            fallback_applied: true,
        });
    }
    Err(EdgeContrastError::InsufficientContrast {
        outline: outline_ratio,
        strong: strong_ratio,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use resina_color::resolve_srgb_fallback;
    use serde_json::{Value, json};

    #[test]
    fn typed_request_rejects_nonfinite_threshold() {
        let black = resolve_srgb_fallback(&json!({
            "colorSpace": "srgb", "components": [0, 0, 0], "alpha": 1
        }))
        .unwrap();
        assert!(matches!(
            resolve_edge_contrast(&black, &black, &black, f64::NAN),
            Err(EdgeContrastError::InvalidMinimumContrast)
        ));
    }

    #[test]
    fn conformance_vectors() {
        let cases: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/color/edge-contrast-vectors.json"
        ))
        .unwrap();
        for case in cases {
            let result = resolve_edge_contrast_source(&case["request"].to_string());
            if let Some(expected) = case.get("expected") {
                let actual = serde_json::to_value(result.unwrap()).unwrap();
                for field in ["schemaVersion", "colorRole", "fallbackApplied"] {
                    assert_eq!(actual[field], expected[field], "{} {field}", case["name"]);
                }
                assert_eq!(
                    resolve_srgb_fallback(&actual["color"]).unwrap(),
                    resolve_srgb_fallback(&expected["color"]).unwrap(),
                    "{} color",
                    case["name"]
                );
                assert!(
                    (actual["contrastRatio"].as_f64().unwrap()
                        - expected["contrastRatio"].as_f64().unwrap())
                    .abs()
                        < 1e-12,
                    "{} contrastRatio",
                    case["name"]
                );
            } else {
                let error = result.unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains(case["errorContains"].as_str().unwrap()),
                    "{}: {error}",
                    case["name"]
                );
            }
        }
    }
}
