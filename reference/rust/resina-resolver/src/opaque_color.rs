use crate::SrgbFallback;
use resina_color::{OpaqueFallbackError, resolve_opaque_srgb_fallback};
use resina_model::{ColorRole, OpaqueColorAssignments};
use resina_tokens::ResolvedToken;
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpaqueColorResolutionErrorKind {
    MissingColor,
    MissingAuthoredToken,
    WrongAuthoredTokenType,
    InvalidFallback(OpaqueFallbackError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueColorResolutionError {
    pub role: ColorRole,
    pub token_path: Option<String>,
    pub kind: OpaqueColorResolutionErrorKind,
}

impl fmt::Display for OpaqueColorResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?} for {:?}", self.kind, self.role)?;
        if let Some(path) = &self.token_path {
            write!(formatter, " at {path}")?;
        }
        Ok(())
    }
}

impl std::error::Error for OpaqueColorResolutionError {}

pub fn resolve_semantic_opaque_color_fallbacks(
    colors: &BTreeMap<ColorRole, Value>,
    assignments: &OpaqueColorAssignments,
    tokens: &BTreeMap<String, ResolvedToken>,
) -> Result<BTreeMap<ColorRole, SrgbFallback>, Vec<OpaqueColorResolutionError>> {
    let mut fallbacks = BTreeMap::new();
    let mut errors = Vec::new();
    for role in ColorRole::ALL {
        let path = assignments.token_path_for(role);
        let authored = match path {
            Some(path) => match tokens.get(path) {
                Some(token) if token.token_type == "color" => Some(&token.value),
                Some(_) => {
                    errors.push(OpaqueColorResolutionError {
                        role,
                        token_path: Some(path.to_owned()),
                        kind: OpaqueColorResolutionErrorKind::WrongAuthoredTokenType,
                    });
                    continue;
                }
                None => {
                    errors.push(OpaqueColorResolutionError {
                        role,
                        token_path: Some(path.to_owned()),
                        kind: OpaqueColorResolutionErrorKind::MissingAuthoredToken,
                    });
                    continue;
                }
            },
            None => None,
        };
        let Some(source) = colors.get(&role) else {
            errors.push(OpaqueColorResolutionError {
                role,
                token_path: path.map(str::to_owned),
                kind: OpaqueColorResolutionErrorKind::MissingColor,
            });
            continue;
        };
        match resolve_opaque_srgb_fallback(source, authored) {
            Ok(fallback) => {
                fallbacks.insert(role, fallback);
            }
            Err(error) => errors.push(OpaqueColorResolutionError {
                role,
                token_path: path.map(str::to_owned),
                kind: OpaqueColorResolutionErrorKind::InvalidFallback(error),
            }),
        }
    }
    if errors.is_empty() {
        Ok(fallbacks)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn errors_follow_role_order_without_publishing_partial_fallbacks() {
        let mut colors: BTreeMap<_, _> = ColorRole::ALL
            .into_iter()
            .map(|role| {
                (
                    role,
                    json!({"colorSpace":"srgb","components":[0.2,0.4,0.6]}),
                )
            })
            .collect();
        colors.insert(
            ColorRole::AccentPrimary,
            json!({"colorSpace":"srgb","components":[0.2,0.4,0.6],"alpha":0.4}),
        );
        let assignments: OpaqueColorAssignments = serde_json::from_value(json!({
            "schemaVersion":"0.1.0",
            "roles":{"surface.chrome":"missing.token","focus":"palette.wrong"}
        }))
        .unwrap();
        let tokens = BTreeMap::from([(
            "palette.wrong".to_owned(),
            ResolvedToken {
                token_type: "dimension".to_owned(),
                value: json!({"value":12,"unit":"px"}),
            },
        )]);
        let errors =
            resolve_semantic_opaque_color_fallbacks(&colors, &assignments, &tokens).unwrap_err();
        assert_eq!(
            errors.iter().map(|error| error.role).collect::<Vec<_>>(),
            [
                ColorRole::AccentPrimary,
                ColorRole::SurfaceChrome,
                ColorRole::Focus
            ]
        );
        assert!(matches!(
            errors[0].kind,
            OpaqueColorResolutionErrorKind::InvalidFallback(
                OpaqueFallbackError::MissingAuthoredFallback
            )
        ));
        assert!(matches!(
            errors[1].kind,
            OpaqueColorResolutionErrorKind::MissingAuthoredToken
        ));
        assert!(matches!(
            errors[2].kind,
            OpaqueColorResolutionErrorKind::WrongAuthoredTokenType
        ));
    }
}
