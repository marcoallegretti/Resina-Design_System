use resina_color::{ColorFallbackError, SrgbFallback, resolve_srgb_fallback};
use resina_model::ColorRole;
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorRoleFallbackErrorKind {
    MissingColor,
    InvalidFallback(ColorFallbackError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorRoleFallbackError {
    pub role: ColorRole,
    pub kind: ColorRoleFallbackErrorKind,
}

impl fmt::Display for ColorRoleFallbackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ColorRoleFallbackErrorKind::MissingColor => {
                write!(formatter, "missing {:?}", self.role)
            }
            ColorRoleFallbackErrorKind::InvalidFallback(error) => {
                write!(formatter, "{:?}: {error}", self.role)
            }
        }
    }
}

impl std::error::Error for ColorRoleFallbackError {}

pub fn resolve_semantic_color_fallbacks(
    colors: &BTreeMap<ColorRole, Value>,
) -> Result<BTreeMap<ColorRole, SrgbFallback>, Vec<ColorRoleFallbackError>> {
    let mut fallbacks = BTreeMap::new();
    let mut errors = Vec::new();
    for role in ColorRole::ALL {
        match colors.get(&role) {
            None => errors.push(ColorRoleFallbackError {
                role,
                kind: ColorRoleFallbackErrorKind::MissingColor,
            }),
            Some(value) => match resolve_srgb_fallback(value) {
                Ok(fallback) => {
                    fallbacks.insert(role, fallback);
                }
                Err(error) => errors.push(ColorRoleFallbackError {
                    role,
                    kind: ColorRoleFallbackErrorKind::InvalidFallback(error),
                }),
            },
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
    fn semantic_roles_reject_all_nonportable_colors_without_partial_output() {
        let mut colors: BTreeMap<_, _> = ColorRole::ALL
            .into_iter()
            .map(|role| {
                (
                    role,
                    json!({"colorSpace":"srgb","components":[0.2,0.4,0.6]}),
                )
            })
            .collect();
        assert_eq!(resolve_semantic_color_fallbacks(&colors).unwrap().len(), 19);
        colors.remove(&ColorRole::AccentSecondary);
        colors.insert(
            ColorRole::SurfaceBase,
            json!({"colorSpace":"display-p3","components":[0.2,0.4,0.6]}),
        );
        colors.insert(
            ColorRole::Focus,
            json!({"colorSpace":"srgb","components":[1,0,0],"hex":"#0000ff"}),
        );
        let errors = resolve_semantic_color_fallbacks(&colors).unwrap_err();
        assert_eq!(
            errors.iter().map(|error| error.role).collect::<Vec<_>>(),
            [
                ColorRole::AccentSecondary,
                ColorRole::SurfaceBase,
                ColorRole::Focus,
            ]
        );
        assert!(matches!(
            errors[0].kind,
            ColorRoleFallbackErrorKind::MissingColor
        ));
        assert!(matches!(
            errors[1].kind,
            ColorRoleFallbackErrorKind::InvalidFallback(ColorFallbackError::MissingHexFallback)
        ));
        assert!(matches!(
            errors[2].kind,
            ColorRoleFallbackErrorKind::InvalidFallback(ColorFallbackError::InconsistentSrgbHex)
        ));
    }
}
