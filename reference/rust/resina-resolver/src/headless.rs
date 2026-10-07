use crate::{
    ColorResolutionError, ColorRoleFallbackError, MinimumHitTarget, OpaqueColorResolutionError,
    ResolvedTypography, SpatialResolutionError, SrgbFallback, TypographyResolutionError,
    theme::{ThemeCompilationError, ThemeSource, compile_theme},
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorAssignments, ColorRole, FrostPigment, FrostRepresentation, MaterialAssignments,
    MaterialFamily, MaterialRole, OpaqueColorAssignments, SpatialAssignments, SpatialRole,
    TypographyAssignments, TypographyRole,
};
use resina_tokens::{DocumentError, ResolverModuleError, parse_token_document};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HeadlessRequest {
    schema_version: String,
    tokens: Value,
    material_assignments: MaterialAssignments,
    frost_pigment: FrostPigment,
    color_assignments: ColorAssignments,
    opaque_color_assignments: OpaqueColorAssignments,
    spatial_assignments: SpatialAssignments,
    typography_assignments: TypographyAssignments,
    environment: EnvironmentSnapshot,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadlessResolution {
    pub(crate) schema_version: &'static str,
    pub(crate) materials: BTreeMap<MaterialRole, MaterialFamily>,
    pub(crate) colors: BTreeMap<ColorRole, Value>,
    pub(crate) color_fallbacks: BTreeMap<ColorRole, SrgbFallback>,
    pub(crate) opaque_color_fallbacks: BTreeMap<ColorRole, SrgbFallback>,
    pub(crate) frost_tint_strength: f64,
    pub(crate) space: BTreeMap<SpatialRole, Value>,
    pub(crate) typography: BTreeMap<TypographyRole, ResolvedTypography>,
    pub(crate) frost_representation: FrostRepresentation,
    pub(crate) minimum_hit_target: MinimumHitTarget,
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

    pub fn opaque_color_fallbacks(&self) -> &BTreeMap<ColorRole, SrgbFallback> {
        &self.opaque_color_fallbacks
    }

    pub fn frost_tint_strength(&self) -> f64 {
        self.frost_tint_strength
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
    OpaqueColorFallback(OpaqueColorResolutionError),
    Space(SpatialResolutionError),
    Typography(TypographyResolutionError),
}

impl fmt::Display for HeadlessBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Color(error) => write!(formatter, "color: {error}"),
            Self::ColorFallback(error) => write!(formatter, "color fallback: {error}"),
            Self::OpaqueColorFallback(error) => write!(formatter, "opaque color fallback: {error}"),
            Self::Space(error) => write!(formatter, "space: {error}"),
            Self::Typography(error) => write!(formatter, "typography: {error}"),
        }
    }
}

#[derive(Debug)]
pub enum HeadlessResolutionError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    InvalidRequestShape,
    UnsupportedVersion,
    InvalidThemeSource,
    Tokens(Vec<DocumentError>),
    TokenResolver(ResolverModuleError),
    Bindings(Vec<HeadlessBindingError>),
}

impl fmt::Display for HeadlessResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "headless source parse failed: {error}"),
            Self::Request(error) => write!(formatter, "invalid headless request: {error}"),
            Self::InvalidRequestShape => {
                formatter.write_str("headless request must be a JSON object")
            }
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.3.0"),
            Self::InvalidThemeSource => {
                formatter.write_str("headless theme source construction failed")
            }
            Self::Tokens(errors) => {
                formatter.write_str("token resolution failed")?;
                for error in errors {
                    write!(formatter, "\n  {error}")?;
                }
                Ok(())
            }
            Self::TokenResolver(error) => write!(formatter, "token composition failed: {error}"),
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
    resolve_headless_document(document)
}

pub(crate) fn resolve_headless_document(
    document: Value,
) -> Result<HeadlessResolution, HeadlessResolutionError> {
    if !document.is_object() {
        return Err(HeadlessResolutionError::InvalidRequestShape);
    }
    if document
        .get("schemaVersion")
        .and_then(Value::as_str)
        .is_some_and(|version| version != "0.3.0")
    {
        return Err(HeadlessResolutionError::UnsupportedVersion);
    }
    let request: HeadlessRequest =
        serde_json::from_value(document).map_err(HeadlessResolutionError::Request)?;
    debug_assert_eq!(request.schema_version, "0.3.0");
    let environment = request.environment;
    let source = ThemeSource {
        schema_version: "0.1.0".to_owned(),
        tokens: Some(request.tokens),
        token_resolver: None,
        token_input: None,
        material_assignments: request.material_assignments,
        frost_pigment: request.frost_pigment,
        color_assignments: request.color_assignments,
        opaque_color_assignments: request.opaque_color_assignments,
        spatial_assignments: request.spatial_assignments,
        typography_assignments: request.typography_assignments,
    };
    let theme = compile_theme(source, environment.text_scale()).map_err(|error| match error {
        ThemeCompilationError::Parse(error) => HeadlessResolutionError::Parse(error),
        ThemeCompilationError::Source(error) => HeadlessResolutionError::Request(error),
        ThemeCompilationError::UnsupportedVersion => HeadlessResolutionError::UnsupportedVersion,
        ThemeCompilationError::InvalidTokenSource => HeadlessResolutionError::InvalidThemeSource,
        ThemeCompilationError::Tokens(errors) => HeadlessResolutionError::Tokens(errors),
        ThemeCompilationError::Resolver(error) => HeadlessResolutionError::TokenResolver(error),
        ThemeCompilationError::Bindings(errors) => HeadlessResolutionError::Bindings(errors),
    })?;
    theme.resolve(&environment).map_err(|errors| {
        HeadlessResolutionError::Bindings(
            errors
                .into_iter()
                .map(HeadlessBindingError::Typography)
                .collect(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unsupported_version_precedes_current_request_fields() {
        let archived = json!({"schemaVersion": "0.1.0", "tokens": {}});
        assert!(matches!(
            resolve_headless_document(archived),
            Err(HeadlessResolutionError::UnsupportedVersion)
        ));
        assert!(matches!(
            resolve_headless_document(json!({"schemaVersion": 1, "tokens": {}})),
            Err(HeadlessResolutionError::Request(_))
        ));
    }
}
