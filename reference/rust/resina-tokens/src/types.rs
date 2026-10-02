use crate::{
    canonical_pointer, curly_path_to_pointer, materialize_group_extensions, parse_pointer,
};
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeErrorKind {
    InvalidDocument,
    InvalidTokenPath,
    MissingTarget,
    NonTokenTarget,
    MissingType,
    CircularReference,
    ReferenceDepthExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeError {
    pub kind: TypeErrorKind,
    pub location: String,
}

impl fmt::Display for TypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?} at {}", self.kind, self.location)
    }
}

impl std::error::Error for TypeError {}

pub fn resolve_token_type(document: &Value, path: &str) -> Result<String, TypeError> {
    let expanded = materialize_group_extensions(document).map_err(|error| TypeError {
        kind: TypeErrorKind::InvalidDocument,
        location: error.location,
    })?;
    resolve_type_in_expanded_document(&expanded, path)
}

pub(crate) fn resolve_type_in_expanded_document(
    expanded: &Value,
    path: &str,
) -> Result<String, TypeError> {
    let pointer = curly_path_to_pointer(path).map_err(|_| TypeError {
        kind: TypeErrorKind::InvalidTokenPath,
        location: path.to_owned(),
    })?;
    TypeResolver { document: expanded }.resolve(&pointer, &mut Vec::new())
}

struct TypeResolver<'a> {
    document: &'a Value,
}

impl TypeResolver<'_> {
    fn resolve(&self, pointer: &str, stack: &mut Vec<String>) -> Result<String, TypeError> {
        let canonical = canonical_pointer(pointer).map_err(|_| TypeError {
            kind: TypeErrorKind::InvalidTokenPath,
            location: pointer.to_owned(),
        })?;
        if stack.contains(&canonical) {
            return Err(TypeError {
                kind: TypeErrorKind::CircularReference,
                location: canonical,
            });
        }
        if stack.len() >= crate::MAX_REFERENCE_DEPTH {
            return Err(TypeError {
                kind: TypeErrorKind::ReferenceDepthExceeded,
                location: canonical,
            });
        }
        let token = self
            .lookup(&canonical)?
            .as_object()
            .ok_or_else(|| TypeError {
                kind: TypeErrorKind::NonTokenTarget,
                location: canonical.clone(),
            })?;
        if !token.contains_key("$value") && !token.contains_key("$ref") {
            return Err(TypeError {
                kind: TypeErrorKind::NonTokenTarget,
                location: canonical,
            });
        }
        if let Some(kind) = token.get("$type").and_then(Value::as_str) {
            return Ok(kind.to_owned());
        }

        stack.push(canonical.clone());
        let curly_reference = token
            .get("$value")
            .and_then(Value::as_str)
            .and_then(|value| value.strip_prefix('{')?.strip_suffix('}'))
            .and_then(|path| curly_path_to_pointer(path).ok());
        let pointer_reference = token
            .get("$ref")
            .and_then(Value::as_str)
            .and_then(|pointer| canonical_pointer(pointer).ok());
        let result =
            if let Some(target) = curly_reference.as_deref().or(pointer_reference.as_deref()) {
                let target_value = self.lookup(target)?;
                if curly_reference.is_some() && target_value.get("$value").is_none() {
                    return Err(TypeError {
                        kind: TypeErrorKind::MissingTarget,
                        location: format!("{target}/$value"),
                    });
                }
                let token_target = if curly_reference.is_some() {
                    Some(target)
                } else {
                    target.strip_suffix("/$value")
                };
                if let Some(token_target) = token_target
                    .filter(|path| crate::token_at_pointer(self.document, path).is_some())
                {
                    self.resolve(token_target, stack)
                } else {
                    self.inherited_type(&canonical)
                }
            } else {
                self.inherited_type(&canonical)
            };
        stack.pop();
        result
    }

    fn inherited_type(&self, pointer: &str) -> Result<String, TypeError> {
        let mut segments = parse_pointer(pointer).unwrap();
        segments.pop();
        loop {
            let group_pointer = segments
                .iter()
                .fold(String::from("#"), |mut path, segment| {
                    path.push('/');
                    path.push_str(&crate::escape_pointer_segment(segment));
                    path
                });
            if let Some(kind) = self
                .lookup(&group_pointer)?
                .get("$type")
                .and_then(Value::as_str)
            {
                return Ok(kind.to_owned());
            }
            if segments.pop().is_none() {
                return Err(TypeError {
                    kind: TypeErrorKind::MissingType,
                    location: pointer.to_owned(),
                });
            }
        }
    }

    fn lookup(&self, pointer: &str) -> Result<&Value, TypeError> {
        let segments = parse_pointer(pointer).map_err(|_| TypeError {
            kind: TypeErrorKind::InvalidTokenPath,
            location: pointer.to_owned(),
        })?;
        let mut current = self.document;
        for segment in segments {
            current = match current {
                Value::Object(object) => object.get(&segment),
                Value::Array(items) => {
                    crate::parse_index(&segment).and_then(|index| items.get(index))
                }
                _ => None,
            }
            .ok_or_else(|| TypeError {
                kind: TypeErrorKind::MissingTarget,
                location: pointer.to_owned(),
            })?;
        }
        Ok(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/tokens/type-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = resolve_token_type(&vector["document"], vector["path"].as_str().unwrap());
            if let Some(expected) = vector.get("expected") {
                assert_eq!(
                    result.unwrap(),
                    expected.as_str().unwrap(),
                    "{}",
                    vector["name"]
                );
            } else {
                let error = result.unwrap_err();
                assert_eq!(
                    format!("{:?}", error.kind),
                    vector["error"].as_str().unwrap(),
                    "{}: {error}",
                    vector["name"]
                );
            }
        }
    }
}
