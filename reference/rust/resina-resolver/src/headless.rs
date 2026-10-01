use crate::{
    ColorResolutionError, ColorRoleFallbackError, MinimumHitTarget, ResolvedTypography,
    SpatialResolutionError, SrgbFallback, TypographyResolutionError, resolve_frost_representation,
    resolve_minimum_hit_target, resolve_semantic_color_fallbacks, resolve_semantic_colors,
    resolve_semantic_space, resolve_semantic_typography,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorAssignments, ColorRole, FrostRepresentation, MaterialAssignments, MaterialFamily,
    MaterialRole, SpatialAssignments, SpatialRole, TypographyAssignments, TypographyRole,
};
use resina_tokens::{DocumentError, parse_token_document, resolve_token_document};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HeadlessRequest {
    schema_version: String,
    tokens: Value,
    material_assignments: MaterialAssignments,
    color_assignments: ColorAssignments,
    spatial_assignments: SpatialAssignments,
    typography_assignments: TypographyAssignments,
    environment: EnvironmentSnapshot,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadlessResolution {
    schema_version: &'static str,
    materials: BTreeMap<MaterialRole, MaterialFamily>,
    colors: BTreeMap<ColorRole, Value>,
    color_fallbacks: BTreeMap<ColorRole, SrgbFallback>,
    space: BTreeMap<SpatialRole, Value>,
    typography: BTreeMap<TypographyRole, ResolvedTypography>,
    frost_representation: FrostRepresentation,
    minimum_hit_target: MinimumHitTarget,
}

impl HeadlessResolution {
    pub fn materials(&self) -> &BTreeMap<MaterialRole, MaterialFamily> {
        &self.materials
    }

    pub fn colors(&self) -> &BTreeMap<ColorRole, Value> {
        &self.colors
    }

    pub fn color_fallbacks(&self) -> &BTreeMap<ColorRole, SrgbFallback> {
        &self.color_fallbacks
    }

    pub fn space(&self) -> &BTreeMap<SpatialRole, Value> {
        &self.space
    }

    pub fn typography(&self) -> &BTreeMap<TypographyRole, ResolvedTypography> {
        &self.typography
    }

    pub fn frost_representation(&self) -> FrostRepresentation {
        self.frost_representation
    }

    pub fn minimum_hit_target(&self) -> MinimumHitTarget {
        self.minimum_hit_target
    }
}

#[derive(Debug)]
pub enum HeadlessBindingError {
    Color(ColorResolutionError),
    ColorFallback(ColorRoleFallbackError),
    Space(SpatialResolutionError),
    Typography(TypographyResolutionError),
}

impl fmt::Display for HeadlessBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Color(error) => write!(formatter, "color: {error}"),
            Self::ColorFallback(error) => write!(formatter, "color fallback: {error}"),
            Self::Space(error) => write!(formatter, "space: {error}"),
            Self::Typography(error) => write!(formatter, "typography: {error}"),
        }
    }
}

#[derive(Debug)]
pub enum HeadlessResolutionError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    Tokens(Vec<DocumentError>),
    Bindings(Vec<HeadlessBindingError>),
}

impl fmt::Display for HeadlessResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "headless source parse failed: {error}"),
            Self::Request(error) => write!(formatter, "invalid headless request: {error}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::Tokens(errors) => {
                formatter.write_str("token resolution failed")?;
                for error in errors {
                    write!(formatter, "\n  {error}")?;
                }
                Ok(())
            }
            Self::Bindings(errors) => {
                formatter.write_str("semantic binding failed")?;
                for error in errors {
                    write!(formatter, "\n  {error}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for HeadlessResolutionError {}

pub fn resolve_headless_source(
    source: &str,
) -> Result<HeadlessResolution, HeadlessResolutionError> {
    let document = parse_token_document(source).map_err(HeadlessResolutionError::Parse)?;
    let request: HeadlessRequest =
        serde_json::from_value(document).map_err(HeadlessResolutionError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(HeadlessResolutionError::UnsupportedVersion);
    }
    let tokens =
        resolve_token_document(&request.tokens).map_err(HeadlessResolutionError::Tokens)?;

    let colors = resolve_semantic_colors(&request.color_assignments, &tokens);
    let color_fallbacks = colors.as_ref().ok().map(resolve_semantic_color_fallbacks);
    let space = resolve_semantic_space(&request.spatial_assignments, &tokens);
    let typography = resolve_semantic_typography(
        &request.typography_assignments,
        &tokens,
        &request.environment,
    );
    let mut errors = Vec::new();
    if let Err(found) = &colors {
        errors.extend(found.iter().cloned().map(HeadlessBindingError::Color));
    }
    if let Some(Err(found)) = &color_fallbacks {
        errors.extend(
            found
                .iter()
                .cloned()
                .map(HeadlessBindingError::ColorFallback),
        );
    }
    if let Err(found) = &space {
        errors.extend(found.iter().cloned().map(HeadlessBindingError::Space));
    }
    if let Err(found) = &typography {
        errors.extend(found.iter().cloned().map(HeadlessBindingError::Typography));
    }
    match (colors, color_fallbacks, space, typography) {
        (Ok(colors), Some(Ok(color_fallbacks)), Ok(space), Ok(typography)) if errors.is_empty() => {
            let materials = MaterialRole::ALL
                .into_iter()
                .map(|role| (role, request.material_assignments.material_for(role)))
                .collect();
            Ok(HeadlessResolution {
                schema_version: "0.2.0",
                materials,
                colors,
                color_fallbacks,
                space,
                typography,
                frost_representation: resolve_frost_representation(
                    request.environment.renderer_capabilities(),
                    request.environment.accessibility_preferences(),
                    request.environment.quality_policy(),
                ),
                minimum_hit_target: resolve_minimum_hit_target(&request.environment),
            })
        }
        _ => Err(HeadlessResolutionError::Bindings(errors)),
    }
}
