use crate::{CornerGeometryError, normalize_corner_radii};
use resina_model::{
    CornerRadius, LogicalCornerRadii, ShapeFallbackAssignments, ShapeFallbackProfile, ShapeIntent,
    SurfaceSize,
};
use resina_tokens::{ResolvedToken, validate_resolved_value};
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeFallbackError {
    EmptyTokenPath,
    MissingToken(String),
    WrongTokenType { path: String, found: String },
    InvalidDimension { path: String, detail: String },
    UnsupportedUnit { path: String, unit: String },
    NegativeRadius(String),
    InvalidGeometry(CornerGeometryError),
}

impl fmt::Display for ShapeFallbackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTokenPath => formatter.write_str("shape token path must not be empty"),
            Self::MissingToken(path) => write!(formatter, "missing shape radius token {path}"),
            Self::WrongTokenType { path, found } => {
                write!(
                    formatter,
                    "shape radius token {path} must be dimension, found {found}"
                )
            }
            Self::InvalidDimension { path, detail } => {
                write!(formatter, "invalid shape radius token {path}: {detail}")
            }
            Self::UnsupportedUnit { path, unit } => {
                write!(
                    formatter,
                    "shape radius token {path} uses unsupported unit {unit}"
                )
            }
            Self::NegativeRadius(path) => {
                write!(formatter, "shape radius token {path} must be nonnegative")
            }
            Self::InvalidGeometry(error) => write!(formatter, "invalid shape bounds: {error}"),
        }
    }
}

impl std::error::Error for ShapeFallbackError {}

pub fn resolve_shape_fallback(
    shape: ShapeIntent,
    size: SurfaceSize,
    assignments: &ShapeFallbackAssignments,
    tokens: &BTreeMap<String, ResolvedToken>,
) -> Result<LogicalCornerRadii, ShapeFallbackError> {
    let radii = match assignments.profile_for(shape) {
        ShapeFallbackProfile::Uniform { radius } => {
            let value = resolve_radius(radius, tokens)?;
            let corner = CornerRadius { x: value, y: value };
            LogicalCornerRadii {
                top_start: corner,
                top_end: corner,
                bottom_end: corner,
                bottom_start: corner,
            }
        }
        ShapeFallbackProfile::Corners { radii } => {
            let top_start = resolve_radius(&radii.top_start, tokens)?;
            let top_end = resolve_radius(&radii.top_end, tokens)?;
            let bottom_end = resolve_radius(&radii.bottom_end, tokens)?;
            let bottom_start = resolve_radius(&radii.bottom_start, tokens)?;
            LogicalCornerRadii {
                top_start: CornerRadius {
                    x: top_start,
                    y: top_start,
                },
                top_end: CornerRadius {
                    x: top_end,
                    y: top_end,
                },
                bottom_end: CornerRadius {
                    x: bottom_end,
                    y: bottom_end,
                },
                bottom_start: CornerRadius {
                    x: bottom_start,
                    y: bottom_start,
                },
            }
        }
        ShapeFallbackProfile::Capsule => {
            let value = size.width.min(size.height) / 2.0;
            let corner = CornerRadius { x: value, y: value };
            LogicalCornerRadii {
                top_start: corner,
                top_end: corner,
                bottom_end: corner,
                bottom_start: corner,
            }
        }
    };
    normalize_corner_radii(size, radii).map_err(ShapeFallbackError::InvalidGeometry)
}

fn resolve_radius(
    path: &str,
    tokens: &BTreeMap<String, ResolvedToken>,
) -> Result<f64, ShapeFallbackError> {
    if path.is_empty() {
        return Err(ShapeFallbackError::EmptyTokenPath);
    }
    let token = tokens
        .get(path)
        .ok_or_else(|| ShapeFallbackError::MissingToken(path.to_owned()))?;
    if token.token_type != "dimension" {
        return Err(ShapeFallbackError::WrongTokenType {
            path: path.to_owned(),
            found: token.token_type.clone(),
        });
    }
    validate_resolved_value("dimension", &token.value).map_err(|error| {
        ShapeFallbackError::InvalidDimension {
            path: path.to_owned(),
            detail: error.to_string(),
        }
    })?;
    let unit = token.value["unit"].as_str().unwrap();
    if unit != "px" {
        return Err(ShapeFallbackError::UnsupportedUnit {
            path: path.to_owned(),
            unit: unit.to_owned(),
        });
    }
    let value = token.value["value"].as_f64().unwrap();
    if !value.is_finite() {
        return Err(ShapeFallbackError::InvalidDimension {
            path: path.to_owned(),
            detail: "radius must be finite".into(),
        });
    }
    if value < 0.0 {
        return Err(ShapeFallbackError::NegativeRadius(path.to_owned()));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use resina_tokens::resolve_token_source;
    use serde_json::{Value, json};

    fn authored_inputs() -> (ShapeFallbackAssignments, BTreeMap<String, ResolvedToken>) {
        let assignments =
            serde_json::from_str(include_str!("../../../../definitions/tier0-shapes.json"))
                .unwrap();
        let tokens =
            resolve_token_source(include_str!("../../../../tokens/foundation.json")).unwrap();
        (assignments, tokens)
    }

    #[test]
    fn authored_tier_zero_shape_vectors() {
        let (assignments, tokens) = authored_inputs();
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/geometry/shape-fallback-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let shape: ShapeIntent = serde_json::from_value(vector["shape"].clone()).unwrap();
            let size: SurfaceSize = serde_json::from_value(vector["size"].clone()).unwrap();
            let actual = resolve_shape_fallback(shape, size, &assignments, &tokens).unwrap();
            let expected: LogicalCornerRadii =
                serde_json::from_value(vector["expected"].clone()).unwrap();
            for (got, want) in [
                (actual.top_start, expected.top_start),
                (actual.top_end, expected.top_end),
                (actual.bottom_end, expected.bottom_end),
                (actual.bottom_start, expected.bottom_start),
            ] {
                assert_eq!(got, want, "{}", vector["name"]);
            }
        }
    }

    #[test]
    fn invalid_radius_bindings_fail_without_geometry() {
        let (assignments, mut tokens) = authored_inputs();
        let size = SurfaceSize {
            width: 200.0,
            height: 80.0,
        };
        tokens.remove("radius.3");
        assert_eq!(
            resolve_shape_fallback(ShapeIntent::Structural, size, &assignments, &tokens),
            Err(ShapeFallbackError::MissingToken("radius.3".into()))
        );
        tokens.insert(
            "radius.3".into(),
            ResolvedToken {
                token_type: "color".into(),
                value: json!({"colorSpace": "srgb", "components": [0, 0, 0]}),
            },
        );
        assert!(matches!(
            resolve_shape_fallback(ShapeIntent::Structural, size, &assignments, &tokens),
            Err(ShapeFallbackError::WrongTokenType { .. })
        ));
        tokens.insert(
            "radius.3".into(),
            ResolvedToken {
                token_type: "dimension".into(),
                value: json!({"value": 1, "unit": "rem"}),
            },
        );
        assert!(matches!(
            resolve_shape_fallback(ShapeIntent::Structural, size, &assignments, &tokens),
            Err(ShapeFallbackError::UnsupportedUnit { .. })
        ));
        tokens.insert(
            "radius.3".into(),
            ResolvedToken {
                token_type: "dimension".into(),
                value: json!({"value": -1, "unit": "px"}),
            },
        );
        assert_eq!(
            resolve_shape_fallback(ShapeIntent::Structural, size, &assignments, &tokens),
            Err(ShapeFallbackError::NegativeRadius("radius.3".into()))
        );
        tokens.insert(
            "radius.3".into(),
            ResolvedToken {
                token_type: "dimension".into(),
                value: json!({"value": "wide", "unit": "px"}),
            },
        );
        assert!(matches!(
            resolve_shape_fallback(ShapeIntent::Structural, size, &assignments, &tokens),
            Err(ShapeFallbackError::InvalidDimension { .. })
        ));
        assert_eq!(
            resolve_shape_fallback(
                ShapeIntent::Capsule,
                SurfaceSize {
                    width: -1.0,
                    height: 80.0,
                },
                &assignments,
                &tokens,
            ),
            Err(ShapeFallbackError::InvalidGeometry(
                CornerGeometryError::InvalidWidth
            ))
        );
    }

    #[test]
    fn custom_assignment_changes_resolved_fallback() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/geometry/shape-fallback-assignment-vectors.json"
        ))
        .unwrap();
        let assignments: ShapeFallbackAssignments =
            serde_json::from_value(vectors[0]["document"].clone()).unwrap();
        let (_, tokens) = authored_inputs();
        let size = SurfaceSize {
            width: 200.0,
            height: 80.0,
        };
        let soft = resolve_shape_fallback(ShapeIntent::Soft, size, &assignments, &tokens).unwrap();
        assert_eq!(soft.top_start, CornerRadius { x: 8.0, y: 8.0 });
        let organic =
            resolve_shape_fallback(ShapeIntent::Organic, size, &assignments, &tokens).unwrap();
        assert_eq!(organic.top_start, CornerRadius { x: 24.0, y: 24.0 });
        assert_eq!(organic.top_end, CornerRadius { x: 16.0, y: 16.0 });
        assert_eq!(organic.bottom_end, CornerRadius { x: 32.0, y: 32.0 });
    }
}
