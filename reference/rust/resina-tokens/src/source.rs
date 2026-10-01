use crate::{DocumentError, ResolvedToken, parse_token_document, resolve_token_document};
use std::{collections::BTreeMap, fmt};

#[derive(Debug)]
pub enum TokenSourceError {
    Parse(serde_json::Error),
    Resolve(Vec<DocumentError>),
}

impl fmt::Display for TokenSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "token source parse failed: {error}"),
            Self::Resolve(errors) => {
                write!(formatter, "token document resolution failed")?;
                for error in errors {
                    write!(formatter, "\n  {error}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for TokenSourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) => Some(error),
            Self::Resolve(_) => None,
        }
    }
}

pub fn resolve_token_source(
    source: &str,
) -> Result<BTreeMap<String, ResolvedToken>, TokenSourceError> {
    let document = parse_token_document(source).map_err(TokenSourceError::Parse)?;
    resolve_token_document(&document).map_err(TokenSourceError::Resolve)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn source_pipeline_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/tokens/source-pipeline-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = resolve_token_source(vector["source"].as_str().unwrap());
            if let Some(expected) = vector.get("expected") {
                let actual = serde_json::json!(
                    result
                        .unwrap()
                        .into_iter()
                        .map(|(path, token)| (
                            path,
                            serde_json::json!({"token_type": token.token_type, "value": token.value})
                        ))
                        .collect::<BTreeMap<_, _>>()
                );
                assert_eq!(actual, *expected, "{}", vector["name"]);
            } else {
                match result.unwrap_err() {
                    TokenSourceError::Parse(error) => {
                        assert_eq!(vector["stage"], "parse", "{}", vector["name"]);
                        assert!(
                            error
                                .to_string()
                                .contains(vector["error"].as_str().unwrap()),
                            "{}: {error}",
                            vector["name"]
                        );
                    }
                    TokenSourceError::Resolve(errors) => {
                        assert_eq!(vector["stage"], "resolve", "{}", vector["name"]);
                        assert!(
                            errors.iter().any(|error| {
                                format!("{:?}", error.kind) == vector["error"].as_str().unwrap()
                                    && error.location == vector["location"].as_str().unwrap()
                            }),
                            "{}: {errors:?}",
                            vector["name"]
                        );
                    }
                }
            }
        }
    }
}
