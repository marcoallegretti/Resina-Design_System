use crate::{
    BoundSurface, EdgeContrastError, EdgeContrastResult, FrostLegibilityError,
    FrostLegibilityResult, HeadlessResolution, SrgbFallback, SurfaceBindingError,
    background_contrast::EdgeBackground,
    bind_surface, resolve_frost_legibility,
    scenario::{SurfaceScenarioError, resolve_surface_scenario_document},
    srgb_input::{SrgbInput, SrgbInputError},
};
use resina_color::{ColorFallbackError, OpaqueSrgbRange};
use resina_model::{ColorRole, InteractionState, MaterialFamily, OpticalTreatment, SurfaceIntent};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrostSurfaceReadabilityRequest {
    schema_version: String,
    scenario: Value,
    foreground_role: ColorRole,
    post_treatment_backdrop: SrgbInput,
    adjacent_color: SrgbInput,
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrostSurfaceReadabilityResult {
    schema_version: &'static str,
    binding: BoundSurface,
    foreground_role: ColorRole,
    foreground: SrgbFallback,
    legibility: FrostLegibilityResult,
    edge: EdgeContrastResult,
}

impl FrostSurfaceReadabilityResult {
    pub fn binding(&self) -> &BoundSurface {
        &self.binding
    }

    pub fn foreground_role(&self) -> ColorRole {
        self.foreground_role
    }

    pub fn foreground(&self) -> &SrgbFallback {
        &self.foreground
    }

    pub fn legibility(&self) -> &FrostLegibilityResult {
        &self.legibility
    }

    pub fn edge(&self) -> &EdgeContrastResult {
        &self.edge
    }
}

#[derive(Debug)]
pub enum FrostSurfaceReadabilityError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    Scenario(SurfaceScenarioError),
    Binding(SurfaceBindingError),
    NonFrostSurface,
    NonBaseState,
    ActiveTreatment,
    InvalidFrostBinding,
    MissingColor(ColorRole),
    TranslucentForeground(ColorRole),
    InvalidColorSpace(&'static str),
    Color(&'static str, ColorFallbackError),
    TranslucentBackdrop,
    TranslucentAdjacentColor,
    Legibility(FrostLegibilityError),
    Edge(EdgeContrastError),
}

impl fmt::Display for FrostSurfaceReadabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "readability request parse failed: {error}"),
            Self::Request(error) => write!(formatter, "invalid readability request: {error}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::Scenario(error) => write!(formatter, "readability scenario: {error}"),
            Self::Binding(error) => write!(formatter, "readability binding: {error}"),
            Self::NonFrostSurface => formatter.write_str("surface material family must be Frost"),
            Self::NonBaseState => {
                formatter.write_str("surface body supports only rest and focused states")
            }
            Self::ActiveTreatment => formatter.write_str("bound surface treatment must be none"),
            Self::InvalidFrostBinding => {
                formatter.write_str("Frost binding lacks its representation or body")
            }
            Self::MissingColor(role) => write!(formatter, "missing resolved color role {role:?}"),
            Self::TranslucentForeground(role) => {
                write!(
                    formatter,
                    "foreground role {role:?} must resolve to an opaque color"
                )
            }
            Self::InvalidColorSpace(field) => write!(formatter, "{field} must use sRGB"),
            Self::Color(field, error) => write!(formatter, "invalid {field}: {error}"),
            Self::TranslucentBackdrop => {
                formatter.write_str("postTreatmentBackdrop must be opaque")
            }
            Self::TranslucentAdjacentColor => formatter.write_str("adjacentColor must be opaque"),
            Self::Legibility(error) => write!(formatter, "readability content: {error}"),
            Self::Edge(error) => write!(formatter, "readability edge: {error}"),
        }
    }
}

impl std::error::Error for FrostSurfaceReadabilityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) | Self::Request(error) => Some(error),
            Self::Scenario(error) => Some(error),
            Self::Binding(error) => Some(error),
            Self::Color(_, error) => Some(error),
            Self::Legibility(error) => Some(error),
            Self::Edge(error) => Some(error),
            _ => None,
        }
    }
}

fn parse_color(
    field: &'static str,
    input: SrgbInput,
) -> Result<SrgbFallback, FrostSurfaceReadabilityError> {
    input.into_fallback().map_err(|error| match error {
        SrgbInputError::InvalidColorSpace => FrostSurfaceReadabilityError::InvalidColorSpace(field),
        SrgbInputError::Color(error) => FrostSurfaceReadabilityError::Color(field, error),
    })
}

pub fn resolve_frost_surface_readability_source(
    source: &str,
) -> Result<FrostSurfaceReadabilityResult, FrostSurfaceReadabilityError> {
    let document = parse_token_document(source).map_err(FrostSurfaceReadabilityError::Parse)?;
    if document
        .get("schemaVersion")
        .and_then(Value::as_str)
        .is_some_and(|version| version != "0.1.0")
    {
        return Err(FrostSurfaceReadabilityError::UnsupportedVersion);
    }
    let request: FrostSurfaceReadabilityRequest =
        serde_json::from_value(document).map_err(FrostSurfaceReadabilityError::Request)?;
    debug_assert_eq!(request.schema_version, "0.1.0");

    let (context, binding) = resolve_surface_scenario_document(request.scenario)
        .map_err(FrostSurfaceReadabilityError::Scenario)?;
    validate_surface_scope(&binding)?;
    let backdrop = parse_color("postTreatmentBackdrop", request.post_treatment_backdrop)?;
    let adjacent = parse_color("adjacentColor", request.adjacent_color)?;
    resolve_bound_frost_surface_readability(
        binding,
        &context,
        request.foreground_role,
        &backdrop,
        EdgeBackground::Uniform(&adjacent),
        request.minimum_content_contrast,
        request.minimum_edge_contrast,
    )
}

pub fn resolve_frost_surface_readability(
    intent: &SurfaceIntent,
    context: &HeadlessResolution,
    foreground_role: ColorRole,
    post_treatment_backdrop: &SrgbFallback,
    adjacent_color: &SrgbFallback,
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
) -> Result<FrostSurfaceReadabilityResult, FrostSurfaceReadabilityError> {
    let binding = bind_surface(intent, context).map_err(FrostSurfaceReadabilityError::Binding)?;
    resolve_bound_frost_surface_readability(
        binding,
        context,
        foreground_role,
        post_treatment_backdrop,
        EdgeBackground::Uniform(adjacent_color),
        minimum_content_contrast,
        minimum_edge_contrast,
    )
}

pub fn resolve_frost_surface_readability_over_ranges(
    intent: &SurfaceIntent,
    context: &HeadlessResolution,
    foreground_role: ColorRole,
    post_treatment_backdrop: &SrgbFallback,
    adjacent_ranges: &[OpaqueSrgbRange],
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
) -> Result<FrostSurfaceReadabilityResult, FrostSurfaceReadabilityError> {
    let binding = bind_surface(intent, context).map_err(FrostSurfaceReadabilityError::Binding)?;
    resolve_bound_frost_surface_readability(
        binding,
        context,
        foreground_role,
        post_treatment_backdrop,
        EdgeBackground::Ranges(adjacent_ranges),
        minimum_content_contrast,
        minimum_edge_contrast,
    )
}

pub(crate) fn resolve_bound_frost_surface_readability(
    binding: BoundSurface,
    context: &HeadlessResolution,
    foreground_role: ColorRole,
    backdrop: &SrgbFallback,
    adjacent: EdgeBackground<'_>,
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
) -> Result<FrostSurfaceReadabilityResult, FrostSurfaceReadabilityError> {
    validate_surface_scope(&binding)?;
    resolve_bound_frost_body_readability(
        binding,
        context,
        foreground_role,
        backdrop,
        adjacent,
        minimum_content_contrast,
        minimum_edge_contrast,
    )
}

pub(crate) fn resolve_bound_frost_body_readability(
    binding: BoundSurface,
    context: &HeadlessResolution,
    foreground_role: ColorRole,
    backdrop: &SrgbFallback,
    adjacent: EdgeBackground<'_>,
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
) -> Result<FrostSurfaceReadabilityResult, FrostSurfaceReadabilityError> {
    if binding.material_family() != MaterialFamily::Frost {
        return Err(FrostSurfaceReadabilityError::NonFrostSurface);
    }
    if binding.treatment_stack().treatments().last() != Some(&OpticalTreatment::None) {
        return Err(FrostSurfaceReadabilityError::ActiveTreatment);
    }
    let foreground = context
        .color_fallbacks()
        .get(&foreground_role)
        .ok_or(FrostSurfaceReadabilityError::MissingColor(foreground_role))?
        .clone();
    if foreground.alpha() != 1.0 {
        return Err(FrostSurfaceReadabilityError::TranslucentForeground(
            foreground_role,
        ));
    }
    if backdrop.alpha() != 1.0 {
        return Err(FrostSurfaceReadabilityError::TranslucentBackdrop);
    }
    adjacent.validate().map_err(|error| match error {
        EdgeContrastError::TranslucentAdjacentColor => {
            FrostSurfaceReadabilityError::TranslucentAdjacentColor
        }
        error => FrostSurfaceReadabilityError::Edge(error),
    })?;
    let representation = binding
        .frost_representation()
        .ok_or(FrostSurfaceReadabilityError::InvalidFrostBinding)?;
    let body = binding
        .frost_portable_body()
        .ok_or(FrostSurfaceReadabilityError::InvalidFrostBinding)?;
    let legibility = resolve_frost_legibility(
        representation,
        body,
        binding.opaque_color_fallback(),
        &foreground,
        backdrop,
        minimum_content_contrast,
    )
    .map_err(FrostSurfaceReadabilityError::Legibility)?;
    let outline = context
        .opaque_color_fallbacks()
        .get(&ColorRole::Outline)
        .ok_or(FrostSurfaceReadabilityError::MissingColor(
            ColorRole::Outline,
        ))?;
    let strong = context
        .opaque_color_fallbacks()
        .get(&ColorRole::OutlineStrong)
        .ok_or(FrostSurfaceReadabilityError::MissingColor(
            ColorRole::OutlineStrong,
        ))?;
    let edge = adjacent
        .resolve(outline, strong, minimum_edge_contrast)
        .map_err(FrostSurfaceReadabilityError::Edge)?;
    Ok(FrostSurfaceReadabilityResult {
        schema_version: "0.1.0",
        binding,
        foreground_role,
        foreground,
        legibility,
        edge,
    })
}

fn validate_surface_scope(binding: &BoundSurface) -> Result<(), FrostSurfaceReadabilityError> {
    if binding.material_family() != MaterialFamily::Frost {
        return Err(FrostSurfaceReadabilityError::NonFrostSurface);
    }
    if binding
        .states()
        .states()
        .iter()
        .any(|state| !matches!(state, InteractionState::Rest | InteractionState::Focused))
    {
        return Err(FrostSurfaceReadabilityError::NonBaseState);
    }
    if binding.treatment_stack().treatments().last() != Some(&OpticalTreatment::None) {
        return Err(FrostSurfaceReadabilityError::ActiveTreatment);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve_headless_source;
    use resina_color::resolve_srgb_fallback;
    use serde_json::{Value, json};

    fn baseline() -> Value {
        let mut resolution: Value = serde_json::from_str(include_str!(
            "../../../../conformance/headless/valid-request.json"
        ))
        .unwrap();
        resolution["colorAssignments"]["roles"]["content.primary"] = json!("palette.opaqueAlt");
        resolution["opaqueColorAssignments"]["roles"]["outline.strong"] =
            json!("palette.opaqueAlt");
        let binding: Value = serde_json::from_str(include_str!(
            "../../../../conformance/surfaces/binding-vectors.json"
        ))
        .unwrap();
        let mut surface = binding[0]["document"].clone();
        surface["states"]["states"] = json!(["rest"]);
        surface["treatmentStack"]["treatments"] = json!(["none"]);
        json!({
            "schemaVersion": "0.1.0",
            "scenario": {
                "schemaVersion": "0.4.0",
                "resolution": resolution,
                "surface": surface
            },
            "foregroundRole": "content.primary",
            "postTreatmentBackdrop": {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1},
            "adjacentColor": {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1},
            "minimumContentContrast": 3,
            "minimumEdgeContrast": 3
        })
    }

    #[test]
    fn public_readability_cases() {
        let cases: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/surfaces/frost-readability-cases.json"
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
            let result = resolve_frost_surface_readability_source(&request.to_string());
            if let Some(expected) = case.get("expected") {
                let result = result.unwrap();
                let actual = serde_json::to_value(result).unwrap();
                assert_eq!(
                    actual["legibility"]["representation"], expected["representation"],
                    "{}",
                    case["name"]
                );
                assert_eq!(
                    actual["legibility"]["fallbackApplied"], expected["contentFallback"],
                    "{}",
                    case["name"]
                );
                assert_eq!(actual["edge"]["colorRole"], expected["edgeRole"]);
                assert_eq!(actual["edge"]["fallbackApplied"], expected["edgeFallback"]);
            } else {
                let error = result.unwrap_err();
                assert!(
                    error.to_string().contains(&crate::diagnostics::expected(
                        "frost_surface_readability",
                        &case["name"]
                    )),
                    "{}: {error}",
                    case["name"]
                );
            }
        }
    }

    #[test]
    fn duplicate_nested_members_fail_before_resolution() {
        let source = baseline().to_string();
        let duplicate = source.replacen(
            "\"schemaVersion\":\"0.4.0\"",
            "\"schemaVersion\":\"0.4.0\",\"schemaVersion\":\"0.4.0\"",
            1,
        );
        assert!(
            resolve_frost_surface_readability_source(&duplicate)
                .unwrap_err()
                .to_string()
                .contains("duplicate JSON member")
        );
    }

    #[test]
    fn typed_and_source_paths_agree() {
        let request = baseline();
        let context =
            resolve_headless_source(&request["scenario"]["resolution"].to_string()).unwrap();
        let intent: SurfaceIntent =
            serde_json::from_value(request["scenario"]["surface"].clone()).unwrap();
        let backdrop = resolve_srgb_fallback(&request["postTreatmentBackdrop"]).unwrap();
        let adjacent = resolve_srgb_fallback(&request["adjacentColor"]).unwrap();
        let typed = resolve_frost_surface_readability(
            &intent,
            &context,
            ColorRole::ContentPrimary,
            &backdrop,
            &adjacent,
            3.0,
            3.0,
        )
        .unwrap();
        let source = resolve_frost_surface_readability_source(&request.to_string()).unwrap();
        assert_eq!(typed, source);
    }

    #[test]
    fn surface_scope_error_precedes_invalid_local_color() {
        let mut request = baseline();
        request["scenario"]["surface"]["materialRole"] = json!("surface.base");
        request["postTreatmentBackdrop"]["colorSpace"] = json!("display-p3");
        assert!(matches!(
            resolve_frost_surface_readability_source(&request.to_string()),
            Err(FrostSurfaceReadabilityError::NonFrostSurface)
        ));
    }
}
