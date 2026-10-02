use crate::{
    BoundSurface, HeadlessResolution, SrgbFallback, SurfaceBindingError, bind_surface,
    opaque_contrast_ratio,
    scenario::{SurfaceScenarioError, resolve_surface_scenario_document},
    srgb_input::{SrgbInput, SrgbInputError},
};
use resina_color::ColorFallbackError;
use resina_model::{ColorRole, InteractionState, SurfaceIntent};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

const MINIMUM_CONTRAST: f64 = 3.0;
const STROKE_WIDTH: f64 = 2.0;
const GAP: f64 = 2.0;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FocusIndicatorRequest {
    schema_version: String,
    scenario: Value,
    surrounding_color: SrgbInput,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusIndicatorResult {
    schema_version: &'static str,
    binding: BoundSurface,
    color_role: ColorRole,
    color: SrgbFallback,
    contrast_ratio: f64,
    fallback_applied: bool,
    stroke_width: f64,
    gap: f64,
}

impl FocusIndicatorResult {
    pub fn binding(&self) -> &BoundSurface {
        &self.binding
    }

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

    pub fn stroke_width(&self) -> f64 {
        self.stroke_width
    }

    pub fn gap(&self) -> f64 {
        self.gap
    }
}

#[derive(Debug)]
pub enum FocusIndicatorError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    Scenario(SurfaceScenarioError),
    Binding(SurfaceBindingError),
    NotFocused,
    InvalidColorSpace,
    Color(ColorFallbackError),
    TranslucentSurround,
    MissingColor(ColorRole),
    InsufficientContrast { focus: f64, outline_strong: f64 },
}

impl fmt::Display for FocusIndicatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => {
                write!(formatter, "focus indicator request parse failed: {error}")
            }
            Self::Request(error) => write!(formatter, "invalid focus indicator request: {error}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::Scenario(error) => write!(formatter, "focus indicator scenario: {error}"),
            Self::Binding(error) => write!(formatter, "focus indicator binding: {error}"),
            Self::NotFocused => formatter.write_str("surface states must include focused"),
            Self::InvalidColorSpace => formatter.write_str("surroundingColor must use sRGB"),
            Self::Color(error) => write!(formatter, "invalid surroundingColor: {error}"),
            Self::TranslucentSurround => formatter.write_str("surroundingColor must be opaque"),
            Self::MissingColor(role) => write!(formatter, "missing opaque color role {role:?}"),
            Self::InsufficientContrast {
                focus,
                outline_strong,
            } => write!(
                formatter,
                "focus indicator contrast is insufficient: focus {focus}, outline.strong {outline_strong}"
            ),
        }
    }
}

impl std::error::Error for FocusIndicatorError {}

pub fn resolve_focus_indicator_source(
    source: &str,
) -> Result<FocusIndicatorResult, FocusIndicatorError> {
    let document = parse_token_document(source).map_err(FocusIndicatorError::Parse)?;
    if document
        .get("schemaVersion")
        .and_then(Value::as_str)
        .is_some_and(|version| version != "0.1.0")
    {
        return Err(FocusIndicatorError::UnsupportedVersion);
    }
    let request: FocusIndicatorRequest =
        serde_json::from_value(document).map_err(FocusIndicatorError::Request)?;
    debug_assert_eq!(request.schema_version, "0.1.0");
    let (context, binding) = resolve_surface_scenario_document(request.scenario)
        .map_err(FocusIndicatorError::Scenario)?;
    validate_focus(&binding)?;
    let surrounding = request
        .surrounding_color
        .into_fallback()
        .map_err(|error| match error {
            SrgbInputError::InvalidColorSpace => FocusIndicatorError::InvalidColorSpace,
            SrgbInputError::Color(error) => FocusIndicatorError::Color(error),
        })?;
    resolve_bound_focus_indicator(binding, &context, &surrounding)
}

pub fn resolve_focus_indicator(
    intent: &SurfaceIntent,
    context: &HeadlessResolution,
    surrounding_color: &SrgbFallback,
) -> Result<FocusIndicatorResult, FocusIndicatorError> {
    let binding = bind_surface(intent, context).map_err(FocusIndicatorError::Binding)?;
    resolve_bound_focus_indicator(binding, context, surrounding_color)
}

fn validate_focus(binding: &BoundSurface) -> Result<(), FocusIndicatorError> {
    if binding.states().contains(InteractionState::Focused) {
        Ok(())
    } else {
        Err(FocusIndicatorError::NotFocused)
    }
}

fn resolve_bound_focus_indicator(
    binding: BoundSurface,
    context: &HeadlessResolution,
    surrounding: &SrgbFallback,
) -> Result<FocusIndicatorResult, FocusIndicatorError> {
    validate_focus(&binding)?;
    if surrounding.alpha() != 1.0 {
        return Err(FocusIndicatorError::TranslucentSurround);
    }
    let focus = context
        .opaque_color_fallbacks()
        .get(&ColorRole::Focus)
        .ok_or(FocusIndicatorError::MissingColor(ColorRole::Focus))?;
    let strong = context
        .opaque_color_fallbacks()
        .get(&ColorRole::OutlineStrong)
        .ok_or(FocusIndicatorError::MissingColor(ColorRole::OutlineStrong))?;
    let focus_ratio = opaque_contrast_ratio(focus, surrounding)
        .expect("headless opaque fallbacks and surrounding color are opaque");
    let (color_role, color, contrast_ratio, fallback_applied) = if focus_ratio >= MINIMUM_CONTRAST {
        (ColorRole::Focus, focus, focus_ratio, false)
    } else {
        let strong_ratio = opaque_contrast_ratio(strong, surrounding)
            .expect("headless opaque fallbacks and surrounding color are opaque");
        if strong_ratio < MINIMUM_CONTRAST {
            return Err(FocusIndicatorError::InsufficientContrast {
                focus: focus_ratio,
                outline_strong: strong_ratio,
            });
        }
        (ColorRole::OutlineStrong, strong, strong_ratio, true)
    };
    Ok(FocusIndicatorResult {
        schema_version: "0.1.0",
        binding,
        color_role,
        color: color.clone(),
        contrast_ratio,
        fallback_applied,
        stroke_width: STROKE_WIDTH,
        gap: GAP,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve_headless_source;
    use resina_color::resolve_srgb_fallback;
    use serde_json::{Value, json};

    fn baseline() -> Value {
        let resolution: Value = serde_json::from_str(include_str!(
            "../../../../conformance/headless/valid-request.json"
        ))
        .unwrap();
        let vectors: Value = serde_json::from_str(include_str!(
            "../../../../conformance/surfaces/binding-vectors.json"
        ))
        .unwrap();
        json!({
            "schemaVersion": "0.1.0",
            "scenario": {
                "schemaVersion": "0.4.0",
                "resolution": resolution,
                "surface": vectors[0]["document"]
            },
            "surroundingColor": {
                "colorSpace": "srgb",
                "components": [1, 1, 1],
                "alpha": 1
            }
        })
    }

    #[test]
    fn public_focus_cases() {
        let cases: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/states/focus-indicator-cases.json"
        ))
        .unwrap();
        for case in cases {
            let mut request = baseline();
            for change in case["changes"].as_array().unwrap() {
                let target = request
                    .pointer_mut(change["path"].as_str().unwrap())
                    .unwrap();
                *target = change["value"].clone();
            }
            let result = resolve_focus_indicator_source(&request.to_string());
            if let Some(expected) = case.get("expected") {
                let result = result.unwrap();
                let actual = serde_json::to_value(result).unwrap();
                assert_eq!(
                    actual["colorRole"], expected["colorRole"],
                    "{}",
                    case["name"]
                );
                assert_eq!(
                    actual["fallbackApplied"], expected["fallbackApplied"],
                    "{}",
                    case["name"]
                );
                assert!(actual["contrastRatio"].as_f64().unwrap() >= MINIMUM_CONTRAST);
                assert_eq!(actual["strokeWidth"].as_f64(), Some(STROKE_WIDTH));
                assert_eq!(actual["gap"].as_f64(), Some(GAP));
                assert!(
                    actual["binding"]["states"]["states"]
                        .as_array()
                        .unwrap()
                        .contains(&json!("focused"))
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

    #[test]
    fn typed_and_source_paths_agree() {
        let request = baseline();
        let context =
            resolve_headless_source(&request["scenario"]["resolution"].to_string()).unwrap();
        let intent: SurfaceIntent =
            serde_json::from_value(request["scenario"]["surface"].clone()).unwrap();
        let surrounding = resolve_srgb_fallback(&request["surroundingColor"]).unwrap();
        assert_eq!(
            resolve_focus_indicator(&intent, &context, &surrounding).unwrap(),
            resolve_focus_indicator_source(&request.to_string()).unwrap()
        );
    }

    #[test]
    fn surface_state_precedes_invalid_local_color() {
        let mut request = baseline();
        request["scenario"]["surface"]["states"]["states"] = json!(["rest"]);
        request["surroundingColor"]["colorSpace"] = json!("display-p3");
        assert!(matches!(
            resolve_focus_indicator_source(&request.to_string()),
            Err(FocusIndicatorError::NotFocused)
        ));
    }

    #[test]
    fn duplicate_nested_members_fail_before_resolution() {
        let duplicate = baseline().to_string().replacen(
            "\"schemaVersion\":\"0.4.0\"",
            "\"schemaVersion\":\"0.4.0\",\"schemaVersion\":\"0.4.0\"",
            1,
        );
        assert!(
            resolve_focus_indicator_source(&duplicate)
                .unwrap_err()
                .to_string()
                .contains("duplicate JSON member")
        );
    }
}
