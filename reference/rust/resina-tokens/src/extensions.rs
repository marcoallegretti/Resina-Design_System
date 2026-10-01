use crate::{
    canonical_pointer, curly_path_to_pointer, parse_pointer, structure::validate_document_structure,
};
use serde_json::{Map, Value};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionErrorKind {
    InvalidStructure,
    InvalidReference,
    MissingTarget,
    TokenTarget,
    CircularExtension,
    ExtensionDepthExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionError {
    pub kind: ExtensionErrorKind,
    pub location: String,
}

impl fmt::Display for ExtensionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?} at {}", self.kind, self.location)
    }
}

impl std::error::Error for ExtensionError {}

pub fn materialize_group_extensions(document: &Value) -> Result<Value, ExtensionError> {
    if let Err(errors) = validate_document_structure(document) {
        return Err(ExtensionError {
            kind: ExtensionErrorKind::InvalidStructure,
            location: errors[0].location.clone(),
        });
    }
    let materializer = Materializer { document };
    materializer.expand_group("#", &mut Vec::new())
}

const MAX_EXTENSION_DEPTH: usize = 256;

struct Materializer<'a> {
    document: &'a Value,
}

impl Materializer<'_> {
    fn expand_group(
        &self,
        pointer: &str,
        stack: &mut Vec<String>,
    ) -> Result<Value, ExtensionError> {
        let canonical = canonical_pointer(pointer).map_err(|_| ExtensionError {
            kind: ExtensionErrorKind::InvalidReference,
            location: pointer.to_owned(),
        })?;
        if stack.contains(&canonical) {
            return Err(ExtensionError {
                kind: ExtensionErrorKind::CircularExtension,
                location: canonical,
            });
        }
        if stack.len() >= MAX_EXTENSION_DEPTH {
            return Err(ExtensionError {
                kind: ExtensionErrorKind::ExtensionDepthExceeded,
                location: canonical,
            });
        }
        let source = self.lookup_group(&canonical)?;
        stack.push(canonical.clone());
        let result = self.expand_group_contents(source, &canonical, stack);
        stack.pop();
        result
    }

    fn expand_group_contents(
        &self,
        source: &Map<String, Value>,
        path: &str,
        stack: &mut Vec<String>,
    ) -> Result<Value, ExtensionError> {
        let mut result = match source.get("$extends") {
            Some(Value::String(reference)) => {
                let target = extension_pointer(reference).map_err(|_| ExtensionError {
                    kind: ExtensionErrorKind::InvalidReference,
                    location: format!("{path}/$extends"),
                })?;
                self.expand_group(&target, stack)?
            }
            Some(_) => unreachable!(),
            None => Value::Object(Map::new()),
        };
        let result_object = result.as_object_mut().unwrap();
        for (name, value) in source {
            if name == "$extends" {
                continue;
            }
            let child_path = format!("{path}/{}", crate::escape_pointer_segment(name));
            let local = if !name.starts_with('$') && is_group(value) {
                self.expand_group(&child_path, stack)?
            } else {
                value.clone()
            };
            match result_object.get_mut(name) {
                Some(inherited)
                    if !name.starts_with('$') && is_group(inherited) && is_group(&local) =>
                {
                    deep_merge_groups(inherited, local);
                }
                _ => {
                    result_object.insert(name.clone(), local);
                }
            }
        }
        Ok(result)
    }

    fn lookup_group(&self, pointer: &str) -> Result<&Map<String, Value>, ExtensionError> {
        let segments = parse_pointer(pointer).map_err(|_| ExtensionError {
            kind: ExtensionErrorKind::InvalidReference,
            location: pointer.to_owned(),
        })?;
        let mut current = self.document;
        for segment in segments {
            if segment.starts_with('$') || !is_group(current) {
                return Err(ExtensionError {
                    kind: ExtensionErrorKind::TokenTarget,
                    location: pointer.to_owned(),
                });
            }
            current = current
                .as_object()
                .and_then(|object| object.get(&segment))
                .ok_or_else(|| ExtensionError {
                    kind: ExtensionErrorKind::MissingTarget,
                    location: pointer.to_owned(),
                })?;
        }
        let object = current.as_object().ok_or_else(|| ExtensionError {
            kind: ExtensionErrorKind::TokenTarget,
            location: pointer.to_owned(),
        })?;
        if object.contains_key("$value") || object.contains_key("$ref") {
            return Err(ExtensionError {
                kind: ExtensionErrorKind::TokenTarget,
                location: pointer.to_owned(),
            });
        }
        Ok(object)
    }
}

fn extension_pointer(reference: &str) -> Result<String, ()> {
    if let Some(inner) = reference
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    {
        curly_path_to_pointer(inner).map_err(|_| ())
    } else {
        canonical_pointer(reference).map_err(|_| ())
    }
}

fn is_group(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| !object.contains_key("$value") && !object.contains_key("$ref"))
}

fn deep_merge_groups(inherited: &mut Value, local: Value) {
    let inherited = inherited.as_object_mut().unwrap();
    for (name, value) in local.as_object().unwrap() {
        match inherited.get_mut(name) {
            Some(previous) if !name.starts_with('$') && is_group(previous) && is_group(value) => {
                deep_merge_groups(previous, value.clone());
            }
            _ => {
                inherited.insert(name.clone(), value.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/tokens/extension-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let source = vector["document"].clone();
            let result = materialize_group_extensions(&vector["document"]);
            if let Some(expected) = vector.get("expected") {
                assert_eq!(result.unwrap(), *expected, "{}", vector["name"]);
            } else {
                let error = result.unwrap_err();
                assert_eq!(
                    format!("{:?}", error.kind),
                    vector["error"].as_str().unwrap(),
                    "{}: {error}",
                    vector["name"]
                );
            }
            assert_eq!(vector["document"], source, "{}", vector["name"]);
        }
    }
}
