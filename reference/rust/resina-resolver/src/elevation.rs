use resina_model::{ElevationDepthAssignments, ElevationRole};
use resina_tokens::{
    DocumentError, ResolvedToken, parse_token_document, resolve_token_document,
    validate_resolved_value,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ElevationDepthRequest {
    schema_version: String,
    tokens: Value,
    assignments: ElevationDepthAssignments,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElevationDepthResult {
    schema_version: &'static str,
    roles: BTreeMap<ElevationRole, Value>,
}

impl ElevationDepthResult {
    pub fn roles(&self) -> &BTreeMap<ElevationRole, Value> {
        &self.roles
    }
}

#[derive(Debug)]
pub enum ElevationDepthError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    Tokens(Vec<DocumentError>),
    Bindings(Vec<ElevationDepthResolutionError>),
}

impl fmt::Display for ElevationDepthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => {
                write!(formatter, "elevation depth request parse failed: {error}")
            }
            Self::Request(error) => write!(formatter, "invalid elevation depth request: {error}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::Tokens(errors) => {
                formatter.write_str("elevation depth token resolution failed")?;
                for error in errors {
                    write!(formatter, "\n  {error}")?;
                }
                Ok(())
            }
            Self::Bindings(errors) => {
                formatter.write_str("elevation depth binding failed")?;
                for error in errors {
                    write!(formatter, "\n  {error}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ElevationDepthError {}

pub fn resolve_elevation_depth_source(
    source: &str,
) -> Result<ElevationDepthResult, ElevationDepthError> {
    let document = parse_token_document(source).map_err(ElevationDepthError::Parse)?;
    if !document.is_object() {
        return Err(ElevationDepthError::Request(serde::de::Error::custom(
            "elevation depth request must be a JSON object",
        )));
    }
    let request: ElevationDepthRequest =
        serde_json::from_value(document).map_err(ElevationDepthError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(ElevationDepthError::UnsupportedVersion);
    }
    let tokens = resolve_token_document(&request.tokens).map_err(ElevationDepthError::Tokens)?;
    let roles = resolve_elevation_depth(&request.assignments, &tokens)
        .map_err(ElevationDepthError::Bindings)?;
    Ok(ElevationDepthResult {
        schema_version: "0.1.0",
        roles,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevationDepthResolutionErrorKind {
    MissingToken,
    WrongTokenType,
    InvalidDimensionValue,
    UnsupportedUnit,
    NegativeExtent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElevationDepthResolutionError {
    pub kind: ElevationDepthResolutionErrorKind,
    pub role: ElevationRole,
    pub token_path: String,
    pub detail: Option<String>,
}

impl fmt::Display for ElevationDepthResolutionError {
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

impl std::error::Error for ElevationDepthResolutionError {}

pub fn resolve_elevation_depth(
    assignments: &ElevationDepthAssignments,
    tokens: &BTreeMap<String, ResolvedToken>,
) -> Result<BTreeMap<ElevationRole, Value>, Vec<ElevationDepthResolutionError>> {
    let mut values = BTreeMap::new();
    let mut errors = Vec::new();
    for role in ElevationRole::ALL {
        let path = assignments.token_path_for(role);
        let result = match tokens.get(path) {
            None => Err((ElevationDepthResolutionErrorKind::MissingToken, None)),
            Some(token) if token.token_type != "dimension" => Err((
                ElevationDepthResolutionErrorKind::WrongTokenType,
                Some(token.token_type.clone()),
            )),
            Some(token) => match validate_resolved_value("dimension", &token.value) {
                Err(error) => Err((
                    ElevationDepthResolutionErrorKind::InvalidDimensionValue,
                    Some(error.to_string()),
                )),
                Ok(()) if token.value["unit"] != "px" => Err((
                    ElevationDepthResolutionErrorKind::UnsupportedUnit,
                    Some(token.value["unit"].to_string()),
                )),
                Ok(())
                    if token.value["value"]
                        .as_f64()
                        .is_some_and(|value| value < 0.0) =>
                {
                    Err((ElevationDepthResolutionErrorKind::NegativeExtent, None))
                }
                Ok(()) => Ok(token.value.clone()),
            },
        };
        match result {
            Ok(value) => {
                values.insert(role, value);
            }
            Err((kind, detail)) => errors.push(ElevationDepthResolutionError {
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
    use serde_json::{Value, json};

    #[test]
    fn depth_source_rejects_duplicate_request_and_token_members() {
        for source in [
            r#"{"schemaVersion":"0.1.0","schemaVersion":"0.1.0"}"#,
            r#"{"schemaVersion":"0.1.0","tokens":{"depth":{"a":{"$value":1,"$value":2}}},"assignments":{}}"#,
        ] {
            assert!(matches!(
                resolve_elevation_depth_source(source),
                Err(ElevationDepthError::Parse(_))
            ));
        }
    }

    #[test]
    fn depth_roles_bind_resolved_dimension_aliases() {
        let assignments: ElevationDepthAssignments = serde_json::from_value(json!({
            "schemaVersion": "0.1.0",
            "roles": {
                "embedded": "depth.zero",
                "base": "depth.zero",
                "raised": "depth.raised",
                "floating": "depth.floating",
                "overlay": "depth.overlay",
                "modal": "depth.modal"
            }
        }))
        .unwrap();
        let source = json!({
            "depth": {
                "$type": "dimension",
                "zero": {"$value": {"value": 0, "unit": "px"}},
                "raised": {"$value": {"value": 2, "unit": "px"}},
                "floating": {"$value": "{depth.raised}"},
                "overlay": {"$value": {"value": 4, "unit": "px"}},
                "modal": {"$value": {"value": 5, "unit": "px"}}
            }
        });
        let tokens = resolve_token_document(&source).unwrap();
        let values = resolve_elevation_depth(&assignments, &tokens).unwrap();
        assert_eq!(
            values[&ElevationRole::Floating],
            values[&ElevationRole::Raised]
        );
        assert_eq!(
            values[&ElevationRole::Embedded],
            json!({"value": 0, "unit": "px"})
        );
    }

    #[test]
    fn depth_resolution_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/elevation/depth-resolution-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let assignments: ElevationDepthAssignments =
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
            let result = resolve_elevation_depth(&assignments, &tokens);
            if let Some(expected) = vector.get("expected") {
                let values = result.unwrap();
                assert_eq!(values.len(), ElevationRole::ALL.len(), "{}", vector["name"]);
                for role in ElevationRole::ALL {
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
