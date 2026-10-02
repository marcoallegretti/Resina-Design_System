use crate::{
    HeadlessBindingError, HeadlessResolution, SrgbFallback, TypographyResolutionError,
    resolve_frost_representation, resolve_minimum_hit_target, resolve_semantic_color_fallbacks,
    resolve_semantic_colors, resolve_semantic_opaque_color_fallbacks, resolve_semantic_space,
    resolve_semantic_typography, typography::resolve_semantic_typography_for_scale,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorAssignments, ColorRole, FrostPigment, MaterialAssignments, MaterialFamily, MaterialRole,
    OpaqueColorAssignments, SpatialAssignments, SpatialRole, TypographyAssignments,
};
use resina_tokens::{
    DocumentError, ResolvedToken, ResolverModuleError, parse_token_document,
    resolve_resolver_module_source, resolve_token_document,
};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ThemeSource {
    pub(crate) schema_version: String,
    pub(crate) tokens: Option<Value>,
    pub(crate) token_resolver: Option<Value>,
    pub(crate) token_input: Option<Value>,
    pub(crate) material_assignments: MaterialAssignments,
    pub(crate) frost_pigment: FrostPigment,
    pub(crate) color_assignments: ColorAssignments,
    pub(crate) opaque_color_assignments: OpaqueColorAssignments,
    pub(crate) spatial_assignments: SpatialAssignments,
    pub(crate) typography_assignments: TypographyAssignments,
}

pub struct CompiledTheme {
    tokens: BTreeMap<String, ResolvedToken>,
    materials: BTreeMap<MaterialRole, MaterialFamily>,
    colors: BTreeMap<ColorRole, Value>,
    color_fallbacks: BTreeMap<ColorRole, SrgbFallback>,
    opaque_color_fallbacks: BTreeMap<ColorRole, SrgbFallback>,
    space: BTreeMap<SpatialRole, Value>,
    typography_assignments: TypographyAssignments,
    frost_tint_strength: f64,
}

#[derive(Debug)]
pub enum ThemeCompilationError {
    Parse(serde_json::Error),
    Source(serde_json::Error),
    UnsupportedVersion,
    InvalidTokenSource,
    Tokens(Vec<DocumentError>),
    Resolver(ResolverModuleError),
    Bindings(Vec<HeadlessBindingError>),
}

impl fmt::Display for ThemeCompilationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "theme source parse failed: {error}"),
            Self::Source(error) => write!(formatter, "invalid theme source: {error}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidTokenSource => {
                formatter.write_str("theme needs either tokens or tokenResolver with tokenInput")
            }
            Self::Tokens(errors) => {
                formatter.write_str("theme token resolution failed")?;
                for error in errors {
                    write!(formatter, "\n  {error}")?;
                }
                Ok(())
            }
            Self::Bindings(errors) => {
                formatter.write_str("theme semantic binding failed")?;
                for error in errors {
                    write!(formatter, "\n  {error}")?;
                }
                Ok(())
            }
            Self::Resolver(error) => write!(formatter, "theme token composition failed: {error}"),
        }
    }
}

impl std::error::Error for ThemeCompilationError {}

pub fn compile_theme_source(source: &str) -> Result<CompiledTheme, ThemeCompilationError> {
    compile_theme_source_with_sources(source, &BTreeMap::new())
}

pub fn compile_theme_source_with_sources(
    source: &str,
    external_sources: &BTreeMap<String, String>,
) -> Result<CompiledTheme, ThemeCompilationError> {
    let document = parse_token_document(source).map_err(ThemeCompilationError::Parse)?;
    compile_theme_document(document, external_sources)
}

fn compile_theme_document(
    document: Value,
    external_sources: &BTreeMap<String, String>,
) -> Result<CompiledTheme, ThemeCompilationError> {
    if document
        .get("schemaVersion")
        .and_then(Value::as_str)
        .is_some_and(|version| version != "0.1.0")
    {
        return Err(ThemeCompilationError::UnsupportedVersion);
    }
    let source: ThemeSource =
        serde_json::from_value(document).map_err(ThemeCompilationError::Source)?;
    compile_theme_with_sources(source, 1.0, external_sources)
}

pub(crate) fn compile_theme(
    source: ThemeSource,
    text_scale: f64,
) -> Result<CompiledTheme, ThemeCompilationError> {
    compile_theme_with_sources(source, text_scale, &BTreeMap::new())
}

fn compile_theme_with_sources(
    source: ThemeSource,
    text_scale: f64,
    external_sources: &BTreeMap<String, String>,
) -> Result<CompiledTheme, ThemeCompilationError> {
    debug_assert_eq!(source.schema_version, "0.1.0");
    let tokens = match (&source.tokens, &source.token_resolver, &source.token_input) {
        (Some(tokens), None, None) => {
            resolve_token_document(tokens).map_err(ThemeCompilationError::Tokens)?
        }
        (None, Some(resolver), Some(input)) => resolve_resolver_module_source(
            &resolver.to_string(),
            &input.to_string(),
            external_sources,
        )
        .map_err(ThemeCompilationError::Resolver)?,
        _ => return Err(ThemeCompilationError::InvalidTokenSource),
    };
    let colors = resolve_semantic_colors(&source.color_assignments, &tokens);
    let color_fallbacks = colors.as_ref().ok().map(resolve_semantic_color_fallbacks);
    let opaque_color_fallbacks = colors.as_ref().ok().map(|colors| {
        resolve_semantic_opaque_color_fallbacks(colors, &source.opaque_color_assignments, &tokens)
    });
    let space = resolve_semantic_space(&source.spatial_assignments, &tokens);
    let typography =
        resolve_semantic_typography_for_scale(&source.typography_assignments, &tokens, text_scale);
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
    if let Some(Err(found)) = &opaque_color_fallbacks {
        errors.extend(
            found
                .iter()
                .cloned()
                .map(HeadlessBindingError::OpaqueColorFallback),
        );
    }
    if let Err(found) = &space {
        errors.extend(found.iter().cloned().map(HeadlessBindingError::Space));
    }
    if let Err(found) = &typography {
        errors.extend(found.iter().cloned().map(HeadlessBindingError::Typography));
    }
    match (colors, color_fallbacks, opaque_color_fallbacks, space) {
        (Ok(colors), Some(Ok(color_fallbacks)), Some(Ok(opaque_color_fallbacks)), Ok(space))
            if errors.is_empty() =>
        {
            let materials = MaterialRole::ALL
                .into_iter()
                .map(|role| (role, source.material_assignments.material_for(role)))
                .collect();
            Ok(CompiledTheme {
                tokens,
                materials,
                colors,
                color_fallbacks,
                opaque_color_fallbacks,
                space,
                typography_assignments: source.typography_assignments,
                frost_tint_strength: source.frost_pigment.tint_strength(),
            })
        }
        _ => Err(ThemeCompilationError::Bindings(errors)),
    }
}

impl CompiledTheme {
    pub fn resolve(
        &self,
        environment: &EnvironmentSnapshot,
    ) -> Result<HeadlessResolution, Vec<TypographyResolutionError>> {
        let typography =
            resolve_semantic_typography(&self.typography_assignments, &self.tokens, environment)?;
        Ok(HeadlessResolution {
            schema_version: "0.4.0",
            materials: self.materials.clone(),
            colors: self.colors.clone(),
            color_fallbacks: self.color_fallbacks.clone(),
            opaque_color_fallbacks: self.opaque_color_fallbacks.clone(),
            frost_tint_strength: self.frost_tint_strength,
            space: self.space.clone(),
            typography,
            frost_representation: resolve_frost_representation(
                environment.renderer_capabilities(),
                environment.accessibility_preferences(),
                environment.quality_policy(),
            ),
            minimum_hit_target: resolve_minimum_hit_target(environment),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve_headless_source;
    use serde_json::json;

    const SOURCE: &str = include_str!("../../../../conformance/themes/valid-source.json");
    const CASES: &str = include_str!("../../../../conformance/themes/source-cases.json");
    const REQUEST: &str = include_str!("../../../../conformance/headless/valid-request.json");
    const HEADLESS_CASES: &str =
        include_str!("../../../../conformance/headless/backend-cases.json");
    const EXPECTED: &str =
        include_str!("../../../../conformance/headless/expected-resolution.json");

    fn resolver_backed_inline_source() -> Value {
        let mut source: Value = serde_json::from_str(SOURCE).unwrap();
        let tokens = source.as_object_mut().unwrap().remove("tokens").unwrap();
        source["tokenResolver"] = json!({
            "version": "2025.10",
            "resolutionOrder": [{"type": "set", "name": "theme", "sources": [tokens]}]
        });
        source["tokenInput"] = json!({});
        source
    }

    #[test]
    fn compiled_theme_matches_headless_resolution_across_environments() {
        let theme = compile_theme_source(SOURCE).unwrap();
        let mut request: Value = serde_json::from_str(REQUEST).unwrap();
        for text_scale in [1.0, 2.0] {
            request["environment"]["textScale"] = json!(text_scale);
            let environment: EnvironmentSnapshot =
                serde_json::from_value(request["environment"].clone()).unwrap();
            let expected = resolve_headless_source(&request.to_string()).unwrap();
            let actual = theme.resolve(&environment).unwrap();
            assert_eq!(
                serde_json::to_value(actual).unwrap(),
                serde_json::to_value(expected).unwrap()
            );
        }
    }

    #[test]
    fn compiled_theme_follows_public_environment_cases() {
        let theme = compile_theme_source(SOURCE).unwrap();
        let baseline_request: Value = serde_json::from_str(REQUEST).unwrap();
        let baseline_expected: Value = serde_json::from_str(EXPECTED).unwrap();
        let cases: Vec<Value> = serde_json::from_str(HEADLESS_CASES).unwrap();
        let mut checked = 0;
        for case in cases {
            let changes = case["requestChanges"].as_array();
            if case["outcome"] != "valid"
                || case.get("sourceText").is_some()
                || case.get("sourceReplace").is_some()
                || changes.is_some_and(|changes| {
                    changes.iter().any(|change| {
                        !change["path"].as_str().is_some_and(|path| {
                            path == "/environment" || path.starts_with("/environment/")
                        })
                    })
                })
            {
                continue;
            }
            let mut request = baseline_request.clone();
            if let Some(changes) = changes {
                for change in changes {
                    let path = change["path"].as_str().unwrap();
                    *request.pointer_mut(path).expect("case path must exist") =
                        change["value"].clone();
                }
            }
            let mut expected = baseline_expected.clone();
            if let Some(changes) = case["expectedChanges"].as_array() {
                for change in changes {
                    let path = change["path"].as_str().unwrap();
                    *expected
                        .pointer_mut(path)
                        .expect("expected path must exist") = change["value"].clone();
                }
            }
            let environment: EnvironmentSnapshot =
                serde_json::from_value(request["environment"].clone()).unwrap();
            let actual = serde_json::to_value(theme.resolve(&environment).unwrap()).unwrap();
            assert_eq!(actual, expected, "{}", case["name"]);
            checked += 1;
        }
        assert_eq!(checked, 7);
    }

    #[test]
    fn resolver_backed_theme_uses_authored_foundation_tokens() {
        let mut source: Value = serde_json::from_str(SOURCE).unwrap();
        let mut theme_tokens = source.as_object_mut().unwrap().remove("tokens").unwrap();
        theme_tokens.as_object_mut().unwrap().remove("space");
        theme_tokens["type"].as_object_mut().unwrap().remove("size");
        source["tokenResolver"] = json!({
            "version": "2025.10",
            "sets": {
                "foundation": {"sources": [{"$ref": "foundation.json"}]},
                "theme": {"sources": [theme_tokens]}
            },
            "resolutionOrder": [
                {"$ref": "#/sets/foundation"},
                {"$ref": "#/sets/theme"}
            ]
        });
        source["tokenInput"] = json!({});
        for path in source["spatialAssignments"]["roles"]
            .as_object_mut()
            .unwrap()
            .values_mut()
        {
            *path = json!("space.4");
        }
        for role in source["typographyAssignments"]["roles"]
            .as_object_mut()
            .unwrap()
            .values_mut()
        {
            role["fontSize"] = json!("type.size.4");
        }
        let foundation = include_str!("../../../../tokens/foundation.json");
        let external = BTreeMap::from([("foundation.json".into(), foundation.into())]);
        let theme = compile_theme_source_with_sources(&source.to_string(), &external).unwrap();
        let request: Value = serde_json::from_str(REQUEST).unwrap();
        let environment: EnvironmentSnapshot =
            serde_json::from_value(request["environment"].clone()).unwrap();
        let result = theme.resolve(&environment).unwrap();
        assert_eq!(
            result.space()[&SpatialRole::ControlInline],
            json!({"value": 16, "unit": "px"})
        );
        assert_eq!(
            result.typography()[&resina_model::TypographyRole::Body].font_size(),
            44.0
        );
        assert!(matches!(
            compile_theme_source_with_sources(&source.to_string(), &BTreeMap::new()),
            Err(ThemeCompilationError::Resolver(_))
        ));
    }

    #[test]
    fn inline_resolver_backed_theme_matches_embedded_token_theme() {
        let source = resolver_backed_inline_source();
        let original = compile_theme_source(SOURCE).unwrap();
        let composed = compile_theme_source(&source.to_string()).unwrap();
        let request: Value = serde_json::from_str(REQUEST).unwrap();
        let environment: EnvironmentSnapshot =
            serde_json::from_value(request["environment"].clone()).unwrap();
        assert_eq!(
            serde_json::to_value(original.resolve(&environment).unwrap()).unwrap(),
            serde_json::to_value(composed.resolve(&environment).unwrap()).unwrap()
        );
    }

    #[test]
    fn theme_token_source_variants_are_exclusive() {
        let mut source: Value = serde_json::from_str(SOURCE).unwrap();
        source["tokenResolver"] = json!({"version": "2025.10", "resolutionOrder": []});
        source["tokenInput"] = json!({});
        assert!(matches!(
            compile_theme_source(&source.to_string()),
            Err(ThemeCompilationError::InvalidTokenSource)
        ));
        source.as_object_mut().unwrap().remove("tokens");
        source.as_object_mut().unwrap().remove("tokenInput");
        assert!(matches!(
            compile_theme_source(&source.to_string()),
            Err(ThemeCompilationError::InvalidTokenSource)
        ));
    }

    #[test]
    fn theme_source_conformance_cases() {
        let cases: Vec<Value> = serde_json::from_str(CASES).unwrap();
        for case in cases {
            let mut source = SOURCE.replace("\r\n", "\n");
            if case["sourceVariant"] == "resolverBackedInline" {
                source = resolver_backed_inline_source().to_string();
            }
            if let Some(changes) = case.get("sourceChanges") {
                let mut document: Value = serde_json::from_str(&source).unwrap();
                for change in changes.as_array().unwrap() {
                    let path = change["path"].as_str().unwrap();
                    let target = document.pointer_mut(path).expect("case path must exist");
                    *target = change["value"].clone();
                }
                source = document.to_string();
            }
            if let Some(replacement) = case.get("sourceReplace") {
                let find = replacement["find"].as_str().unwrap();
                let with = replacement["with"].as_str().unwrap();
                assert_eq!(source.matches(find).count(), 1, "{}", case["name"]);
                source = source.replacen(find, with, 1);
            }
            let result = compile_theme_source(&source);
            assert_eq!(
                result.is_ok(),
                case["outcome"] == "valid",
                "{}: {}",
                case["name"],
                result
                    .err()
                    .map(|error| error.to_string())
                    .unwrap_or_default()
            );
        }
    }
}
