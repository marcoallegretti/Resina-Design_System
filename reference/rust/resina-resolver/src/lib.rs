use resina_environment::{AccessibilityPreferences, QualityPolicy, RendererCapabilities};
use resina_model::{ColorAssignments, ColorRole, FrostRepresentation};
use resina_tokens::{ResolvedToken, validate_resolved_value};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

mod spatial;
pub use spatial::{SpatialResolutionError, SpatialResolutionErrorKind, resolve_semantic_space};
mod headless;
pub use headless::{
    HeadlessBindingError, HeadlessResolution, HeadlessResolutionError, resolve_headless_source,
};
mod target;
pub use target::{MinimumHitTarget, resolve_minimum_hit_target};
mod typography;
pub use typography::{
    PxDimension, ResolvedTypography, TypographyResolutionError, TypographyResolutionErrorKind,
    resolve_semantic_typography,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorResolutionErrorKind {
    MissingToken,
    WrongTokenType,
    InvalidColorValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorResolutionError {
    pub kind: ColorResolutionErrorKind,
    pub role: ColorRole,
    pub token_path: String,
    pub detail: Option<String>,
}

impl fmt::Display for ColorResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} for {:?} at {}",
            self.kind, self.role, self.token_path
        )?;
        if let Some(detail) = &self.detail {
            write!(formatter, ": {detail}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ColorResolutionError {}

pub fn resolve_semantic_colors(
    assignments: &ColorAssignments,
    tokens: &BTreeMap<String, ResolvedToken>,
) -> Result<BTreeMap<ColorRole, Value>, Vec<ColorResolutionError>> {
    let mut colors = BTreeMap::new();
    let mut errors = Vec::new();
    for role in ColorRole::ALL {
        let path = assignments.token_path_for(role);
        let result = match tokens.get(path) {
            None => Err((ColorResolutionErrorKind::MissingToken, None)),
            Some(token) if token.token_type != "color" => Err((
                ColorResolutionErrorKind::WrongTokenType,
                Some(token.token_type.clone()),
            )),
            Some(token) => validate_resolved_value("color", &token.value)
                .map(|()| token.value.clone())
                .map_err(|error| {
                    (
                        ColorResolutionErrorKind::InvalidColorValue,
                        Some(error.to_string()),
                    )
                }),
        };
        match result {
            Ok(value) => {
                colors.insert(role, value);
            }
            Err((kind, detail)) => errors.push(ColorResolutionError {
                kind,
                role,
                token_path: path.to_owned(),
                detail,
            }),
        }
    }
    if errors.is_empty() {
        Ok(colors)
    } else {
        Err(errors)
    }
}

pub fn resolve_frost_representation(
    capabilities: &RendererCapabilities,
    preferences: &AccessibilityPreferences,
    quality: QualityPolicy,
) -> FrostRepresentation {
    if preferences.reduced_transparency
        || preferences.high_contrast
        || !capabilities.translucent_surfaces
    {
        return FrostRepresentation::OpaqueDimensional;
    }
    if quality == QualityPolicy::Economy {
        return FrostRepresentation::TranslucentPigmented;
    }
    if capabilities.backdrop_effect && capabilities.backdrop_blur {
        if capabilities.shaped_backdrop && quality == QualityPolicy::Full {
            FrostRepresentation::ShapedBackdrop
        } else {
            FrostRepresentation::RegularBackdrop
        }
    } else {
        FrostRepresentation::TranslucentPigmented
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use resina_tokens::resolve_token_document;

    #[test]
    fn semantic_colors_bind_resolved_document_aliases() {
        let assignments: ColorAssignments = serde_json::from_value(
            serde_json::from_str::<Value>(include_str!(
                "../../../../conformance/color/role-assignment-vectors.json"
            ))
            .unwrap()[0]["document"]
                .clone(),
        )
        .unwrap();
        let source = serde_json::json!({
            "palette": {
                "$type": "color",
                "sample0": {"$value": {"colorSpace": "srgb", "components": [0, 0, 0]}},
                "sample1": {"$value": {"colorSpace": "srgb", "components": [1, 1, 1]}},
                "sample2": {"$value": "{palette.sample0}"}
            }
        });
        let tokens = resolve_token_document(&source).unwrap();
        let colors = resolve_semantic_colors(&assignments, &tokens).unwrap();
        assert_eq!(colors.len(), ColorRole::ALL.len());
        assert_eq!(
            colors[&ColorRole::AccentTertiary],
            colors[&ColorRole::AccentPrimary]
        );
        assert_eq!(
            colors[&ColorRole::AccentSecondary],
            tokens["palette.sample1"].value
        );
    }

    #[test]
    fn semantic_color_resolution_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/color/resolution-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let assignments: ColorAssignments =
                serde_json::from_value(vector["assignments"].clone()).unwrap();
            let tokens = vector["tokens"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(path, token)| {
                    (
                        path.clone(),
                        ResolvedToken {
                            token_type: token["token_type"].as_str().unwrap().to_owned(),
                            value: token["value"].clone(),
                        },
                    )
                })
                .collect();
            let result = resolve_semantic_colors(&assignments, &tokens);
            if let Some(expected) = vector.get("expected") {
                let actual = result.unwrap();
                assert_eq!(actual.len(), ColorRole::ALL.len(), "{}", vector["name"]);
                for role in ColorRole::ALL {
                    let name = serde_json::to_value(role).unwrap();
                    assert_eq!(
                        actual[&role],
                        expected[name.as_str().unwrap()],
                        "{}: {name}",
                        vector["name"]
                    );
                }
            } else {
                let errors = result.unwrap_err();
                let actual: Vec<_> = errors
                    .iter()
                    .map(|error| {
                        serde_json::json!({
                            "kind": format!("{:?}", error.kind),
                            "role": error.role,
                            "tokenPath": error.token_path,
                            "detail": error.detail
                        })
                    })
                    .collect();
                assert_eq!(
                    serde_json::to_value(actual).unwrap(),
                    vector["errors"],
                    "{}",
                    vector["name"]
                );
            }
        }
    }

    #[test]
    fn frost_fallback_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/materials/frost-fallback-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let capabilities: RendererCapabilities =
                serde_json::from_value(vector["capabilities"].clone()).unwrap();
            let preferences: AccessibilityPreferences =
                serde_json::from_value(vector["preferences"].clone()).unwrap();
            let quality: QualityPolicy = serde_json::from_value(vector["quality"].clone()).unwrap();
            let actual = resolve_frost_representation(&capabilities, &preferences, quality);
            assert_eq!(
                serde_json::to_value(actual).unwrap(),
                vector["expected"],
                "{}",
                vector["name"]
            );
        }
    }
}
