use crate::{
    canonical_pointer, curly_path_to_pointer, parse_pointer, structure::validate_document_structure,
};
use serde_json::{Map, Value};
use std::{cell::Cell, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionErrorKind {
    InvalidStructure,
    InvalidReference,
    MissingTarget,
    TokenTarget,
    CircularExtension,
    ExtensionDepthExceeded,
    ExpansionLimitExceeded,
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
    let materializer = Materializer {
        document,
        remaining_nodes: Cell::new(MAX_EXPANSION_NODES),
        remaining_text_bytes: Cell::new(MAX_EXPANSION_TEXT_BYTES),
    };
    materializer.expand_group("#", &mut Vec::new())
}

const MAX_EXTENSION_DEPTH: usize = 256;
const MAX_EXPANSION_NODES: usize = 100_000;
const MAX_EXPANSION_TEXT_BYTES: usize = 8 * 1024 * 1024;

struct Materializer<'a> {
    document: &'a Value,
    remaining_nodes: Cell<usize>,
    remaining_text_bytes: Cell<usize>,
}

struct ExpansionCost {
    nodes: usize,
    text_bytes: usize,
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
                self.expand_group(&target, stack).map_err(|mut error| {
                    if error.kind == ExtensionErrorKind::ExpansionLimitExceeded {
                        error.location = format!("{path}/$extends");
                    }
                    error
                })?
            }
            Some(_) => unreachable!(),
            None => {
                self.charge(
                    ExpansionCost {
                        nodes: 1,
                        text_bytes: 0,
                    },
                    path,
                )?;
                Value::Object(Map::new())
            }
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
                self.charge(value_cost(value), &child_path)?;
                value.clone()
            };
            match result_object.get_mut(name) {
                Some(inherited)
                    if !name.starts_with('$') && is_group(inherited) && is_group(&local) =>
                {
                    deep_merge_groups(inherited, local);
                }
                _ => {
                    self.charge(
                        ExpansionCost {
                            nodes: 0,
                            text_bytes: name.len(),
                        },
                        &child_path,
                    )?;
                    result_object.insert(name.clone(), local);
                }
            }
        }
        Ok(result)
    }

    fn charge(&self, cost: ExpansionCost, path: &str) -> Result<(), ExtensionError> {
        let remaining_nodes = self
            .remaining_nodes
            .get()
            .checked_sub(cost.nodes)
            .ok_or_else(|| ExtensionError {
                kind: ExtensionErrorKind::ExpansionLimitExceeded,
                location: path.to_owned(),
            })?;
        let remaining_text_bytes = self
            .remaining_text_bytes
            .get()
            .checked_sub(cost.text_bytes)
            .ok_or_else(|| ExtensionError {
                kind: ExtensionErrorKind::ExpansionLimitExceeded,
                location: path.to_owned(),
            })?;
        self.remaining_nodes.set(remaining_nodes);
        self.remaining_text_bytes.set(remaining_text_bytes);
        Ok(())
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

fn value_cost(value: &Value) -> ExpansionCost {
    match value {
        Value::Array(items) => items.iter().fold(
            ExpansionCost {
                nodes: 1,
                text_bytes: 0,
            },
            |cost, item| cost.combine(value_cost(item)),
        ),
        Value::Object(fields) => fields.iter().fold(
            ExpansionCost {
                nodes: 1,
                text_bytes: 0,
            },
            |cost, (name, item)| {
                cost.combine(ExpansionCost {
                    nodes: 0,
                    text_bytes: name.len(),
                })
                .combine(value_cost(item))
            },
        ),
        Value::String(text) => ExpansionCost {
            nodes: 1,
            text_bytes: text.len(),
        },
        _ => ExpansionCost {
            nodes: 1,
            text_bytes: 0,
        },
    }
}

impl ExpansionCost {
    fn combine(self, other: Self) -> Self {
        Self {
            nodes: self.nodes.saturating_add(other.nodes),
            text_bytes: self.text_bytes.saturating_add(other.text_bytes),
        }
    }
}

fn deep_merge_groups(inherited: &mut Value, local: Value) {
    let inherited = inherited.as_object_mut().unwrap();
    let Value::Object(local) = local else {
        unreachable!()
    };
    for (name, value) in local {
        match inherited.get_mut(&name) {
            Some(previous) if !name.starts_with('$') && is_group(previous) && is_group(&value) => {
                deep_merge_groups(previous, value);
            }
            _ => {
                inherited.insert(name, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

    #[test]
    fn compact_branching_extensions_have_a_diagnostic_work_limit() {
        let mut document = json!({"g0": {"leaf": {"$type": "number", "$value": 1}}});
        for level in 1..=10 {
            let previous = format!("{{g{}}}", level - 1);
            document[format!("g{level}")] = json!({
                "left": {"$extends": previous},
                "right": {"$extends": previous},
            });
        }
        assert!(materialize_group_extensions(&document).is_ok());

        for level in 11..=16 {
            let previous = format!("{{g{}}}", level - 1);
            document[format!("g{level}")] = json!({
                "left": {"$extends": previous},
                "right": {"$extends": previous},
            });
        }
        let original = document.clone();
        let error = materialize_group_extensions(&document).unwrap_err();
        assert_eq!(error.kind, ExtensionErrorKind::ExpansionLimitExceeded);
        assert!(error.location.starts_with("#/g"));
        assert!(error.location.ends_with("/$extends"));
        assert_eq!(document, original);
    }

    #[test]
    fn repeated_metadata_text_is_bounded_independently_of_node_count() {
        let mut document = json!({
            "g0": {
                "leaf": {
                    "$type": "number",
                    "$value": 1,
                    "$description": "x".repeat(65_536)
                }
            }
        });
        for level in 1..=8 {
            let previous = format!("{{g{}}}", level - 1);
            document[format!("g{level}")] = json!({
                "left": {"$extends": previous},
                "right": {"$extends": previous},
            });
        }
        let error = materialize_group_extensions(&document).unwrap_err();
        assert_eq!(error.kind, ExtensionErrorKind::ExpansionLimitExceeded);
        assert!(error.location.ends_with("/$extends"));
    }
}
