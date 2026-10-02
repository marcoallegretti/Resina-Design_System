use crate::{SrgbFallback, opaque_contrast_ratio};
use resina_color::{ColorFallbackError, composite_srgb_over_opaque, resolve_srgb_fallback};
use resina_model::FrostRepresentation;
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ColorInput {
    color_space: String,
    components: [f64; 3],
    alpha: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrostLegibilityRequest {
    schema_version: String,
    representation: FrostRepresentation,
    portable_body: ColorInput,
    opaque_body: ColorInput,
    foreground: ColorInput,
    post_treatment_backdrop: ColorInput,
    minimum_contrast: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrostLegibilityResult {
    schema_version: &'static str,
    representation: FrostRepresentation,
    body: SrgbFallback,
    composited_body: SrgbFallback,
    contrast_ratio: f64,
    fallback_applied: bool,
}

impl FrostLegibilityResult {
    pub fn representation(&self) -> FrostRepresentation {
        self.representation
    }

    pub fn body(&self) -> &SrgbFallback {
        &self.body
    }

    pub fn composited_body(&self) -> &SrgbFallback {
        &self.composited_body
    }

    pub fn contrast_ratio(&self) -> f64 {
        self.contrast_ratio
    }

    pub fn fallback_applied(&self) -> bool {
        self.fallback_applied
    }
}

#[derive(Debug)]
pub enum FrostLegibilityError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    InvalidColorSpace(&'static str),
    Color(&'static str, ColorFallbackError),
    InvalidMinimumContrast,
    TranslucentOpaqueBody,
    TranslucentForeground,
    TranslucentBackdrop,
    InvalidRepresentationBody,
    InsufficientContrast { selected: f64, opaque: f64 },
}

impl fmt::Display for FrostLegibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "legibility request parse failed: {error}"),
            Self::Request(error) => write!(formatter, "invalid legibility request: {error}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidColorSpace(field) => write!(formatter, "{field} must use sRGB"),
            Self::Color(field, error) => write!(formatter, "invalid {field}: {error}"),
            Self::InvalidMinimumContrast => {
                formatter.write_str("minimumContrast must be finite and in [1, 21]")
            }
            Self::TranslucentOpaqueBody => formatter.write_str("opaqueBody must be opaque"),
            Self::TranslucentForeground => formatter.write_str("foreground must be opaque"),
            Self::TranslucentBackdrop => {
                formatter.write_str("postTreatmentBackdrop must be opaque")
            }
            Self::InvalidRepresentationBody => {
                formatter.write_str("portableBody does not match the selected representation")
            }
            Self::InsufficientContrast { selected, opaque } => write!(
                formatter,
                "Frost contrast is insufficient: selected {selected}, opaque fallback {opaque}"
            ),
        }
    }
}

impl std::error::Error for FrostLegibilityError {}

fn parse_color(
    field: &'static str,
    input: ColorInput,
) -> Result<SrgbFallback, FrostLegibilityError> {
    if input.color_space != "srgb" {
        return Err(FrostLegibilityError::InvalidColorSpace(field));
    }
    let value = serde_json::json!({
        "colorSpace": input.color_space,
        "components": input.components,
        "alpha": input.alpha,
    });
    resolve_srgb_fallback(&value).map_err(|error| FrostLegibilityError::Color(field, error))
}

pub fn resolve_frost_legibility_source(
    source: &str,
) -> Result<FrostLegibilityResult, FrostLegibilityError> {
    let document = parse_token_document(source).map_err(FrostLegibilityError::Parse)?;
    if document
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|version| version != "0.1.0")
    {
        return Err(FrostLegibilityError::UnsupportedVersion);
    }
    let request: FrostLegibilityRequest =
        serde_json::from_value(document).map_err(FrostLegibilityError::Request)?;
    debug_assert_eq!(request.schema_version, "0.1.0");
    let body = parse_color("portableBody", request.portable_body)?;
    let opaque_body = parse_color("opaqueBody", request.opaque_body)?;
    let foreground = parse_color("foreground", request.foreground)?;
    let backdrop = parse_color("postTreatmentBackdrop", request.post_treatment_backdrop)?;
    resolve_frost_legibility(
        request.representation,
        &body,
        &opaque_body,
        &foreground,
        &backdrop,
        request.minimum_contrast,
    )
}

pub fn resolve_frost_legibility(
    representation: FrostRepresentation,
    portable_body: &SrgbFallback,
    opaque_body: &SrgbFallback,
    foreground: &SrgbFallback,
    post_treatment_backdrop: &SrgbFallback,
    minimum_contrast: f64,
) -> Result<FrostLegibilityResult, FrostLegibilityError> {
    if !minimum_contrast.is_finite() || !(1.0..=21.0).contains(&minimum_contrast) {
        return Err(FrostLegibilityError::InvalidMinimumContrast);
    }
    if opaque_body.alpha() != 1.0 {
        return Err(FrostLegibilityError::TranslucentOpaqueBody);
    }
    if foreground.alpha() != 1.0 {
        return Err(FrostLegibilityError::TranslucentForeground);
    }
    if post_treatment_backdrop.alpha() != 1.0 {
        return Err(FrostLegibilityError::TranslucentBackdrop);
    }
    let composited = if representation == FrostRepresentation::OpaqueDimensional {
        if portable_body != opaque_body {
            return Err(FrostLegibilityError::InvalidRepresentationBody);
        }
        portable_body.clone()
    } else {
        if portable_body.alpha() <= 0.0 || portable_body.alpha() >= 1.0 {
            return Err(FrostLegibilityError::InvalidRepresentationBody);
        }
        composite_srgb_over_opaque(portable_body, post_treatment_backdrop)
            .map_err(|_| FrostLegibilityError::TranslucentBackdrop)?
    };
    let selected_ratio = opaque_contrast_ratio(foreground, &composited)
        .expect("validated foreground and composited body are opaque");
    if selected_ratio >= minimum_contrast {
        return Ok(FrostLegibilityResult {
            schema_version: "0.1.0",
            representation,
            body: portable_body.clone(),
            composited_body: composited,
            contrast_ratio: selected_ratio,
            fallback_applied: false,
        });
    }
    let opaque_ratio = opaque_contrast_ratio(foreground, opaque_body)
        .expect("validated foreground and opaque fallback are opaque");
    if representation != FrostRepresentation::OpaqueDimensional && opaque_ratio >= minimum_contrast
    {
        return Ok(FrostLegibilityResult {
            schema_version: "0.1.0",
            representation: FrostRepresentation::OpaqueDimensional,
            body: opaque_body.clone(),
            composited_body: opaque_body.clone(),
            contrast_ratio: opaque_ratio,
            fallback_applied: true,
        });
    }
    Err(FrostLegibilityError::InsufficientContrast {
        selected: selected_ratio,
        opaque: opaque_ratio,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn typed_request_rejects_nonfinite_threshold() {
        let black = resolve_srgb_fallback(&json!({
            "colorSpace": "srgb", "components": [0, 0, 0], "alpha": 1
        }))
        .unwrap();
        let white = resolve_srgb_fallback(&json!({
            "colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1
        }))
        .unwrap();
        assert!(matches!(
            resolve_frost_legibility(
                FrostRepresentation::OpaqueDimensional,
                &black,
                &black,
                &white,
                &white,
                f64::NAN,
            ),
            Err(FrostLegibilityError::InvalidMinimumContrast)
        ));
    }

    #[test]
    fn conformance_vectors() {
        let cases: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/materials/frost-legibility-vectors.json"
        ))
        .unwrap();
        for case in cases {
            let source = case["request"].to_string();
            let result = resolve_frost_legibility_source(&source);
            if let Some(expected) = case.get("expected") {
                let actual = serde_json::to_value(result.unwrap()).unwrap();
                for field in ["schemaVersion", "representation", "fallbackApplied"] {
                    assert_eq!(actual[field], expected[field], "{} {field}", case["name"]);
                }
                for field in ["body", "compositedBody"] {
                    let actual_color = resolve_srgb_fallback(&actual[field]).unwrap();
                    let expected_color = resolve_srgb_fallback(&expected[field]).unwrap();
                    assert_eq!(actual_color, expected_color, "{} {field}", case["name"]);
                }
                let actual_ratio = actual["contrastRatio"].as_f64().unwrap();
                let expected_ratio = expected["contrastRatio"].as_f64().unwrap();
                assert!(
                    (actual_ratio - expected_ratio).abs() < 1e-12,
                    "{}",
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
