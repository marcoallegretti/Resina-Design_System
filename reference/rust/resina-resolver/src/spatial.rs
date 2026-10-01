use resina_model::{SpatialAssignments, SpatialRole};
use resina_tokens::{ResolvedToken, validate_resolved_value};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialResolutionErrorKind {
    MissingToken,
    WrongTokenType,
    InvalidDimensionValue,
    NegativeDistance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpatialResolutionError {
    pub kind: SpatialResolutionErrorKind,
    pub role: SpatialRole,
    pub token_path: String,
    pub detail: Option<String>,
}

impl fmt::Display for SpatialResolutionError {
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

impl std::error::Error for SpatialResolutionError {}

pub fn resolve_semantic_space(
    assignments: &SpatialAssignments,
    tokens: &BTreeMap<String, ResolvedToken>,
) -> Result<BTreeMap<SpatialRole, Value>, Vec<SpatialResolutionError>> {
    let mut values = BTreeMap::new();
    let mut errors = Vec::new();
    for role in SpatialRole::ALL {
        let path = assignments.token_path_for(role);
        let result = match tokens.get(path) {
            None => Err((SpatialResolutionErrorKind::MissingToken, None)),
            Some(token) if token.token_type != "dimension" => Err((
                SpatialResolutionErrorKind::WrongTokenType,
                Some(token.token_type.clone()),
            )),
            Some(token) => match validate_resolved_value("dimension", &token.value) {
                Err(error) => Err((
                    SpatialResolutionErrorKind::InvalidDimensionValue,
                    Some(error.to_string()),
                )),
                Ok(())
                    if token.value["value"]
                        .as_f64()
                        .is_some_and(|value| value < 0.0) =>
                {
                    Err((SpatialResolutionErrorKind::NegativeDistance, None))
                }
                Ok(()) => Ok(token.value.clone()),
            },
        };
        match result {
            Ok(value) => {
                values.insert(role, value);
            }
            Err((kind, detail)) => errors.push(SpatialResolutionError {
                kind,
                role,
                token_path: path.to_owned(),
                detail,
            }),
        }
    }
    if errors.is_empty() {
        Ok(values)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use resina_tokens::resolve_token_document;
    use serde_json::json;

    #[test]
    fn spatial_roles_bind_resolved_dimension_aliases() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/spatial/assignment-vectors.json"
        ))
        .unwrap();
        let assignments: SpatialAssignments =
            serde_json::from_value(vectors[0]["document"].clone()).unwrap();
        let source = json!({
            "foundation": {
                "space": {
                    "$type": "dimension",
                    "0": {"$value": {"value": 0, "unit": "px"}},
                    "1": {"$value": {"value": 4, "unit": "px"}},
                    "2": {"$value": {"value": 8, "unit": "px"}},
                    "3": {"$value": {"value": 1, "unit": "rem"}},
                    "4": {"$value": {"value": 20, "unit": "px"}},
                    "5": {"$value": {"value": 32, "unit": "px"}},
                    "6": {"$value": "{foundation.space.0}"}
                }
            }
        });
        let tokens = resolve_token_document(&source).unwrap();
        let values = resolve_semantic_space(&assignments, &tokens).unwrap();
        assert_eq!(
            values[&SpatialRole::ContainerOuter],
            json!({"value": 1, "unit": "rem"})
        );
        assert_eq!(
            values[&SpatialRole::Page],
            values[&SpatialRole::ControlInline]
        );
    }

    #[test]
    fn spatial_resolution_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/spatial/resolution-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let assignments: SpatialAssignments =
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
            let result = resolve_semantic_space(&assignments, &tokens);
            if let Some(expected) = vector.get("expected") {
                let values = result.unwrap();
                assert_eq!(values.len(), SpatialRole::ALL.len(), "{}", vector["name"]);
                for role in SpatialRole::ALL {
                    let name = serde_json::to_value(role).unwrap();
                    assert_eq!(
                        values[&role],
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
                        json!({
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
}
