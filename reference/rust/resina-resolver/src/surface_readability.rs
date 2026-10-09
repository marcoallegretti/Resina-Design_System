use crate::{
    BoundSurface, EdgeContrastError, EdgeContrastResult, FrostSurfaceReadabilityError,
    HeadlessResolution, SrgbFallback, SurfaceBindingError,
    background_contrast::EdgeBackground,
    bind_surface,
    frost_surface_readability::resolve_bound_frost_body_readability,
    opaque_contrast_ratio,
    scenario::{SurfaceScenarioError, resolve_surface_scenario_document},
    srgb_input::{SrgbInput, SrgbInputError},
};
use resina_color::{ColorFallbackError, OpaqueSrgbRange};
use resina_model::{
    ColorRole, FrostRepresentation, InteractionState, MaterialFamily, OpticalTreatment,
    SurfaceIntent,
};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::fmt;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SurfaceReadabilityRequest {
    schema_version: String,
    scenario: Value,
    foreground_role: ColorRole,
    #[serde(default, deserialize_with = "deserialize_present_backdrop")]
    post_treatment_backdrop: Option<SrgbInput>,
    adjacent_color: SrgbInput,
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceReadabilityResult {
    schema_version: &'static str,
    binding: BoundSurface,
    foreground_role: ColorRole,
    foreground: SrgbFallback,
    body: SrgbFallback,
    composited_body: SrgbFallback,
    content_contrast_ratio: f64,
    content_fallback_applied: bool,
    edge: EdgeContrastResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    frost_representation: Option<FrostRepresentation>,
}

impl SurfaceReadabilityResult {
    pub fn binding(&self) -> &BoundSurface {
        &self.binding
    }
    pub fn foreground_role(&self) -> ColorRole {
        self.foreground_role
    }
    pub fn foreground(&self) -> &SrgbFallback {
        &self.foreground
    }
    pub fn body(&self) -> &SrgbFallback {
        &self.body
    }
    pub fn composited_body(&self) -> &SrgbFallback {
        &self.composited_body
    }
    pub fn content_contrast_ratio(&self) -> f64 {
        self.content_contrast_ratio
    }
    pub fn content_fallback_applied(&self) -> bool {
        self.content_fallback_applied
    }
    pub fn edge(&self) -> &EdgeContrastResult {
        &self.edge
    }
    pub fn frost_representation(&self) -> Option<FrostRepresentation> {
        self.frost_representation
    }
}

#[derive(Debug)]
pub enum SurfaceReadabilityError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    InvalidRequestShape,
    UnsupportedVersion,
    Scenario(SurfaceScenarioError),
    Binding(SurfaceBindingError),
    NonBaseState,
    ActiveTreatment,
    InvalidColorSpace(&'static str),
    Color(&'static str, ColorFallbackError),
    TranslucentBackdrop,
    MissingFrostBackdrop,
    TranslucentAdjacentColor,
    MissingColor(ColorRole),
    InvalidContentThreshold,
    InsufficientContentContrast(f64),
    Frost(FrostSurfaceReadabilityError),
    Edge(EdgeContrastError),
}

impl fmt::Display for SurfaceReadabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "surface readability parse failed: {error}"),
            Self::Request(error) => {
                write!(formatter, "invalid surface readability request: {error}")
            }
            Self::InvalidRequestShape => {
                formatter.write_str("surface readability request must be a JSON object")
            }
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::Scenario(error) => write!(formatter, "surface readability scenario: {error}"),
            Self::Binding(error) => write!(formatter, "surface readability binding: {error}"),
            Self::NonBaseState => {
                formatter.write_str("surface body supports only rest and focused states")
            }
            Self::ActiveTreatment => formatter.write_str("bound surface treatment must be none"),
            Self::InvalidColorSpace(field) => write!(formatter, "{field} must use sRGB"),
            Self::Color(field, error) => write!(formatter, "invalid {field}: {error}"),
            Self::TranslucentBackdrop => {
                formatter.write_str("postTreatmentBackdrop must be opaque")
            }
            Self::MissingFrostBackdrop => {
                formatter.write_str("Frost requires postTreatmentBackdrop")
            }
            Self::TranslucentAdjacentColor => formatter.write_str("adjacentColor must be opaque"),
            Self::MissingColor(role) => write!(formatter, "missing opaque color role {role:?}"),
            Self::InvalidContentThreshold => {
                formatter.write_str("minimumContentContrast must be finite and in [1, 21]")
            }
            Self::InsufficientContentContrast(ratio) => write!(
                formatter,
                "surface content contrast is insufficient: {ratio}"
            ),
            Self::Frost(error) => write!(formatter, "surface readability Frost: {error}"),
            Self::Edge(error) => write!(formatter, "surface readability edge: {error}"),
        }
    }
}

impl std::error::Error for SurfaceReadabilityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) | Self::Request(error) => Some(error),
            Self::Scenario(error) => Some(error),
            Self::Binding(error) => Some(error),
            Self::Color(_, error) => Some(error),
            Self::Frost(error) => Some(error),
            Self::Edge(error) => Some(error),
            _ => None,
        }
    }
}

fn parse_color(
    field: &'static str,
    input: SrgbInput,
) -> Result<SrgbFallback, SurfaceReadabilityError> {
    input.into_fallback().map_err(|error| match error {
        SrgbInputError::InvalidColorSpace => SurfaceReadabilityError::InvalidColorSpace(field),
        SrgbInputError::Color(error) => SurfaceReadabilityError::Color(field, error),
    })
}

fn deserialize_present_backdrop<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<SrgbInput>, D::Error> {
    SrgbInput::deserialize(deserializer).map(Some)
}

pub fn resolve_surface_readability_source(
    source: &str,
) -> Result<SurfaceReadabilityResult, SurfaceReadabilityError> {
    let document = parse_token_document(source).map_err(SurfaceReadabilityError::Parse)?;
    if !document.is_object() {
        return Err(SurfaceReadabilityError::InvalidRequestShape);
    }
    if document
        .get("schemaVersion")
        .and_then(Value::as_str)
        .is_some_and(|version| version != "0.1.0")
    {
        return Err(SurfaceReadabilityError::UnsupportedVersion);
    }
    let request: SurfaceReadabilityRequest =
        serde_json::from_value(document).map_err(SurfaceReadabilityError::Request)?;
    debug_assert_eq!(request.schema_version, "0.1.0");
    let (context, binding) = resolve_surface_scenario_document(request.scenario)
        .map_err(SurfaceReadabilityError::Scenario)?;
    let backdrop = request
        .post_treatment_backdrop
        .map(|input| parse_color("postTreatmentBackdrop", input))
        .transpose()?;
    let adjacent = parse_color("adjacentColor", request.adjacent_color)?;
    resolve_bound_surface_readability(
        binding,
        &context,
        request.foreground_role,
        backdrop.as_ref(),
        EdgeBackground::Uniform(&adjacent),
        request.minimum_content_contrast,
        request.minimum_edge_contrast,
    )
}

pub fn resolve_surface_readability(
    intent: &SurfaceIntent,
    context: &HeadlessResolution,
    foreground_role: ColorRole,
    post_treatment_backdrop: Option<&SrgbFallback>,
    adjacent_color: &SrgbFallback,
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
) -> Result<SurfaceReadabilityResult, SurfaceReadabilityError> {
    let binding = bind_surface(intent, context).map_err(SurfaceReadabilityError::Binding)?;
    resolve_bound_surface_readability(
        binding,
        context,
        foreground_role,
        post_treatment_backdrop,
        EdgeBackground::Uniform(adjacent_color),
        minimum_content_contrast,
        minimum_edge_contrast,
    )
}

pub fn resolve_surface_readability_over_ranges(
    intent: &SurfaceIntent,
    context: &HeadlessResolution,
    foreground_role: ColorRole,
    post_treatment_backdrop: Option<&SrgbFallback>,
    adjacent_ranges: &[OpaqueSrgbRange],
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
) -> Result<SurfaceReadabilityResult, SurfaceReadabilityError> {
    let binding = bind_surface(intent, context).map_err(SurfaceReadabilityError::Binding)?;
    resolve_bound_surface_readability(
        binding,
        context,
        foreground_role,
        post_treatment_backdrop,
        EdgeBackground::Ranges(adjacent_ranges),
        minimum_content_contrast,
        minimum_edge_contrast,
    )
}

fn resolve_bound_surface_readability(
    binding: BoundSurface,
    context: &HeadlessResolution,
    foreground_role: ColorRole,
    backdrop: Option<&SrgbFallback>,
    adjacent: EdgeBackground<'_>,
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
) -> Result<SurfaceReadabilityResult, SurfaceReadabilityError> {
    if binding
        .states()
        .states()
        .iter()
        .any(|state| !matches!(state, InteractionState::Rest | InteractionState::Focused))
    {
        return Err(SurfaceReadabilityError::NonBaseState);
    }
    resolve_bound_body_readability(
        binding,
        context,
        foreground_role,
        backdrop,
        adjacent,
        minimum_content_contrast,
        minimum_edge_contrast,
    )
}

pub(crate) fn resolve_bound_body_readability(
    binding: BoundSurface,
    context: &HeadlessResolution,
    foreground_role: ColorRole,
    backdrop: Option<&SrgbFallback>,
    adjacent: EdgeBackground<'_>,
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
) -> Result<SurfaceReadabilityResult, SurfaceReadabilityError> {
    if binding.treatment_stack().treatments().last() != Some(&OpticalTreatment::None) {
        return Err(SurfaceReadabilityError::ActiveTreatment);
    }
    if backdrop.is_some_and(|color| color.alpha() != 1.0) {
        return Err(SurfaceReadabilityError::TranslucentBackdrop);
    }
    adjacent.validate().map_err(|error| match error {
        EdgeContrastError::TranslucentAdjacentColor => {
            SurfaceReadabilityError::TranslucentAdjacentColor
        }
        error => SurfaceReadabilityError::Edge(error),
    })?;
    if !minimum_content_contrast.is_finite() || !(1.0..=21.0).contains(&minimum_content_contrast) {
        return Err(SurfaceReadabilityError::InvalidContentThreshold);
    }
    if binding.material_family() == MaterialFamily::Frost {
        let backdrop = backdrop.ok_or(SurfaceReadabilityError::MissingFrostBackdrop)?;
        let resolved = resolve_bound_frost_body_readability(
            binding,
            context,
            foreground_role,
            backdrop,
            adjacent,
            minimum_content_contrast,
            minimum_edge_contrast,
        )
        .map_err(SurfaceReadabilityError::Frost)?;
        let legibility = resolved.legibility();
        return Ok(SurfaceReadabilityResult {
            schema_version: "0.1.0",
            binding: resolved.binding().clone(),
            foreground_role,
            foreground: resolved.foreground().clone(),
            body: legibility.body().clone(),
            composited_body: legibility.composited_body().clone(),
            content_contrast_ratio: legibility.contrast_ratio(),
            content_fallback_applied: legibility.fallback_applied(),
            edge: resolved.edge().clone(),
            frost_representation: Some(legibility.representation()),
        });
    }
    let foreground = context
        .opaque_color_fallbacks()
        .get(&foreground_role)
        .ok_or(SurfaceReadabilityError::MissingColor(foreground_role))?
        .clone();
    let body = binding.opaque_color_fallback().clone();
    let content_contrast_ratio =
        opaque_contrast_ratio(&foreground, &body).expect("headless opaque fallbacks are opaque");
    if content_contrast_ratio < minimum_content_contrast {
        return Err(SurfaceReadabilityError::InsufficientContentContrast(
            content_contrast_ratio,
        ));
    }
    let outline = context
        .opaque_color_fallbacks()
        .get(&ColorRole::Outline)
        .ok_or(SurfaceReadabilityError::MissingColor(ColorRole::Outline))?;
    let strong = context
        .opaque_color_fallbacks()
        .get(&ColorRole::OutlineStrong)
        .ok_or(SurfaceReadabilityError::MissingColor(
            ColorRole::OutlineStrong,
        ))?;
    let edge = adjacent
        .resolve(outline, strong, minimum_edge_contrast)
        .map_err(SurfaceReadabilityError::Edge)?;
    Ok(SurfaceReadabilityResult {
        schema_version: "0.1.0",
        binding,
        foreground_role,
        foreground,
        body: body.clone(),
        composited_body: body,
        content_contrast_ratio,
        content_fallback_applied: false,
        edge,
        frost_representation: None,
    })
}
