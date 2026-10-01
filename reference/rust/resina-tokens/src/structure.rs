use crate::{curly_path_to_pointer, escape_pointer_segment, parse_pointer};
use serde_json::{Map, Value};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureErrorKind {
    InvalidRoot,
    InvalidName,
    InvalidNode,
    InvalidRootToken,
    UnknownProperty,
    InvalidMetadata,
    InvalidReference,
    ConflictingValueAndReference,
    TokenWithChildren,
    DepthExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructureError {
    pub kind: StructureErrorKind,
    pub location: String,
}

impl fmt::Display for StructureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?} at {}", self.kind, self.location)
    }
}

impl std::error::Error for StructureError {}

pub fn validate_document_structure(document: &Value) -> Result<(), Vec<StructureError>> {
    let mut errors = Vec::new();
    match document.as_object() {
        Some(group) => validate_group(group, "#", true, 0, &mut errors),
        None => errors.push(StructureError {
            kind: StructureErrorKind::InvalidRoot,
            location: "#".to_owned(),
        }),
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

const MAX_GROUP_DEPTH: usize = 256;

fn validate_group(
    group: &Map<String, Value>,
    path: &str,
    root: bool,
    depth: usize,
    errors: &mut Vec<StructureError>,
) {
    if depth >= MAX_GROUP_DEPTH {
        push(errors, StructureErrorKind::DepthExceeded, path);
        return;
    }
    for (name, value) in group {
        let child_path = pointer_child(path, name);
        match name.as_str() {
            "$schema" if root => {
                if !value.is_string() {
                    push(errors, StructureErrorKind::InvalidMetadata, &child_path);
                }
            }
            "$type" => validate_type(value, &child_path, errors),
            "$description" => validate_description(value, &child_path, errors),
            "$extensions" => validate_extensions(value, &child_path, errors),
            "$deprecated" => validate_deprecated(value, &child_path, errors),
            "$extends" => validate_extends(value, &child_path, errors),
            "$root" => match value.as_object() {
                Some(token) if is_token_shape(token) => validate_token(token, &child_path, errors),
                _ => push(errors, StructureErrorKind::InvalidRootToken, &child_path),
            },
            _ if name.starts_with('$') => {
                push(errors, StructureErrorKind::UnknownProperty, &child_path);
            }
            _ if !valid_name(name) => {
                push(errors, StructureErrorKind::InvalidName, &child_path);
            }
            _ => match value.as_object() {
                Some(object) if is_token_shape(object) => {
                    validate_token(object, &child_path, errors)
                }
                Some(object) => validate_group(object, &child_path, false, depth + 1, errors),
                None => push(errors, StructureErrorKind::InvalidNode, &child_path),
            },
        }
    }
}

fn validate_token(token: &Map<String, Value>, path: &str, errors: &mut Vec<StructureError>) {
    match (token.contains_key("$value"), token.contains_key("$ref")) {
        (true, true) => push(
            errors,
            StructureErrorKind::ConflictingValueAndReference,
            path,
        ),
        (false, false) => unreachable!(),
        _ => {}
    }
    for (name, value) in token {
        let property_path = pointer_child(path, name);
        match name.as_str() {
            "$value" => {}
            "$ref" => {
                if value
                    .as_str()
                    .is_none_or(|reference| parse_pointer(reference).is_err())
                {
                    push(errors, StructureErrorKind::InvalidReference, &property_path);
                }
            }
            "$type" => validate_type(value, &property_path, errors),
            "$description" => validate_description(value, &property_path, errors),
            "$extensions" => validate_extensions(value, &property_path, errors),
            "$deprecated" => validate_deprecated(value, &property_path, errors),
            _ if name.starts_with('$') => {
                push(errors, StructureErrorKind::UnknownProperty, &property_path);
            }
            _ => push(
                errors,
                StructureErrorKind::TokenWithChildren,
                &property_path,
            ),
        }
    }
}

fn is_token_shape(node: &Map<String, Value>) -> bool {
    node.contains_key("$value") || node.contains_key("$ref")
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('$') && !name.contains(['{', '}', '.'])
}

fn validate_type(value: &Value, path: &str, errors: &mut Vec<StructureError>) {
    const TYPES: &[&str] = &[
        "color",
        "dimension",
        "fontFamily",
        "fontWeight",
        "duration",
        "cubicBezier",
        "number",
        "strokeStyle",
        "border",
        "transition",
        "shadow",
        "gradient",
        "typography",
    ];
    if value.as_str().is_none_or(|kind| !TYPES.contains(&kind)) {
        push(errors, StructureErrorKind::InvalidMetadata, path);
    }
}

fn validate_description(value: &Value, path: &str, errors: &mut Vec<StructureError>) {
    if !value.is_string() {
        push(errors, StructureErrorKind::InvalidMetadata, path);
    }
}

fn validate_extensions(value: &Value, path: &str, errors: &mut Vec<StructureError>) {
    if !value.is_object() {
        push(errors, StructureErrorKind::InvalidMetadata, path);
    }
}

fn validate_deprecated(value: &Value, path: &str, errors: &mut Vec<StructureError>) {
    if !value.is_boolean() && !value.is_string() {
        push(errors, StructureErrorKind::InvalidMetadata, path);
    }
}

fn validate_extends(value: &Value, path: &str, errors: &mut Vec<StructureError>) {
    let valid = value.as_str().is_some_and(|reference| {
        if let Some(inner) = reference
            .strip_prefix('{')
            .and_then(|value| value.strip_suffix('}'))
        {
            curly_path_to_pointer(inner).is_ok()
        } else {
            parse_pointer(reference).is_ok()
        }
    });
    if !valid {
        push(errors, StructureErrorKind::InvalidReference, path);
    }
}

fn pointer_child(path: &str, name: &str) -> String {
    format!("{path}/{}", escape_pointer_segment(name))
}

fn push(errors: &mut Vec<StructureError>, kind: StructureErrorKind, location: &str) {
    errors.push(StructureError {
        kind,
        location: location.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/tokens/structure-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = validate_document_structure(&vector["document"]);
            match vector.get("error") {
                None => assert!(result.is_ok(), "{}: {result:?}", vector["name"]),
                Some(expected) => {
                    let errors = result.unwrap_err();
                    let expected_kind = expected.as_str().unwrap();
                    assert!(
                        errors
                            .iter()
                            .any(|error| format!("{:?}", error.kind) == expected_kind),
                        "{}: {errors:?}",
                        vector["name"]
                    );
                }
            }
        }
    }
}
