use crate::{
    Resolver, TypeErrorKind, canonical_pointer, curly_path_to_pointer, escape_pointer_segment,
    is_token, materialize_group_extensions, parse_pointer,
    types::resolve_type_in_expanded_document, validate_resolved_value,
};
use serde_json::{Map, Value};
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedToken {
    pub token_type: String,
    pub value: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentErrorKind {
    InvalidDocument,
    MissingType,
    InvalidReference,
    ReferenceTypeMismatch,
    ExplicitNestedArray,
    InvalidValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentError {
    pub kind: DocumentErrorKind,
    pub location: String,
}

impl fmt::Display for DocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?} at {}", self.kind, self.location)
    }
}

impl std::error::Error for DocumentError {}

pub fn resolve_token_document(
    document: &Value,
) -> Result<BTreeMap<String, ResolvedToken>, Vec<DocumentError>> {
    let expanded = materialize_group_extensions(document).map_err(|error| {
        vec![DocumentError {
            kind: DocumentErrorKind::InvalidDocument,
            location: error.location,
        }]
    })?;
    let mut tokens = Vec::new();
    collect_tokens(expanded.as_object().unwrap(), &mut Vec::new(), &mut tokens);
    let mut resolved = BTreeMap::new();
    let mut errors = Vec::new();
    for (path, pointer) in tokens {
        let kind = match resolve_type_in_expanded_document(&expanded, &path) {
            Ok(kind) => kind,
            Err(error) => {
                errors.push(type_error(error));
                continue;
            }
        };
        let token = lookup(&expanded, &pointer).unwrap().as_object().unwrap();
        let raw = token
            .get("$value")
            .unwrap_or_else(|| lookup(&expanded, &pointer).unwrap());
        let raw_path = if token.contains_key("$value") {
            format!("{pointer}/$value")
        } else {
            format!("{pointer}/$ref")
        };
        if let Err(error) = check_authoring_references(&expanded, raw, &kind, &raw_path, 0) {
            errors.push(error);
            continue;
        }
        let value = match (Resolver {
            document: &expanded,
        })
        .resolve_at(&pointer, &mut Vec::new())
        {
            Ok(value) => value,
            Err(error) => {
                errors.push(DocumentError {
                    kind: DocumentErrorKind::InvalidReference,
                    location: error.location,
                });
                continue;
            }
        };
        if let Err(error) = validate_resolved_value(&kind, &value) {
            errors.push(DocumentError {
                kind: DocumentErrorKind::InvalidValue,
                location: error.location.replacen("#/$value", &raw_path, 1),
            });
            continue;
        }
        resolved.insert(
            path,
            ResolvedToken {
                token_type: kind,
                value,
            },
        );
    }
    if errors.is_empty() {
        Ok(resolved)
    } else {
        Err(errors)
    }
}

fn collect_tokens(
    group: &Map<String, Value>,
    ancestors: &mut Vec<String>,
    tokens: &mut Vec<(String, String)>,
) {
    for (name, node) in group {
        if name.starts_with('$') && name != "$root" {
            continue;
        }
        ancestors.push(name.clone());
        if is_token(node) {
            let path = ancestors.join(".");
            tokens.push((path.clone(), curly_path_to_pointer(&path).unwrap()));
        } else if let Some(nested) = node.as_object() {
            collect_tokens(nested, ancestors, tokens);
        }
        ancestors.pop();
    }
}

const MAX_AUTHORING_DEPTH: usize = 64;

fn check_authoring_references(
    document: &Value,
    raw: &Value,
    expected: &str,
    path: &str,
    depth: usize,
) -> Result<(), DocumentError> {
    if depth >= MAX_AUTHORING_DEPTH {
        return Err(DocumentError {
            kind: DocumentErrorKind::InvalidValue,
            location: path.to_owned(),
        });
    }
    if let Some(target) = reference_target(raw) {
        if let Some(target_token) = whole_token_target(document, &target) {
            let target_type =
                resolve_type_in_expanded_document(document, &target_token).map_err(type_error)?;
            if target_type != expected {
                return Err(DocumentError {
                    kind: DocumentErrorKind::ReferenceTypeMismatch,
                    location: path.to_owned(),
                });
            }
        }
        return Ok(());
    }
    match expected {
        "dimension" | "duration" => check_field(document, raw, "value", "number", path, depth),
        "color" => {
            check_array_field(document, raw, "components", "number", path, depth)?;
            check_field(document, raw, "alpha", "number", path, depth)
        }
        "cubicBezier" => check_items(document, raw, "number", path, depth),
        "fontFamily" => check_items(document, raw, "fontFamily", path, depth),
        "strokeStyle" => check_array_field(document, raw, "dashArray", "dimension", path, depth),
        "border" => check_fields(
            document,
            raw,
            &[
                ("color", "color"),
                ("width", "dimension"),
                ("style", "strokeStyle"),
            ],
            path,
            depth,
        ),
        "transition" => check_fields(
            document,
            raw,
            &[
                ("duration", "duration"),
                ("delay", "duration"),
                ("timingFunction", "cubicBezier"),
            ],
            path,
            depth,
        ),
        "typography" => check_fields(
            document,
            raw,
            &[
                ("fontFamily", "fontFamily"),
                ("fontSize", "dimension"),
                ("fontWeight", "fontWeight"),
                ("letterSpacing", "dimension"),
                ("lineHeight", "number"),
            ],
            path,
            depth,
        ),
        "shadow" => {
            if let Some(items) = raw.as_array() {
                for (index, item) in items.iter().enumerate() {
                    if item.is_array() {
                        return Err(DocumentError {
                            kind: DocumentErrorKind::ExplicitNestedArray,
                            location: format!("{path}/{index}"),
                        });
                    }
                    check_authoring_references(
                        document,
                        item,
                        "shadow",
                        &format!("{path}/{index}"),
                        depth + 1,
                    )?;
                }
                Ok(())
            } else {
                check_fields(
                    document,
                    raw,
                    &[
                        ("color", "color"),
                        ("offsetX", "dimension"),
                        ("offsetY", "dimension"),
                        ("blur", "dimension"),
                        ("spread", "dimension"),
                    ],
                    path,
                    depth,
                )
            }
        }
        "gradient" => {
            if let Some(items) = raw.as_array() {
                for (index, item) in items.iter().enumerate() {
                    let location = format!("{path}/{index}");
                    if item.is_array() {
                        return Err(DocumentError {
                            kind: DocumentErrorKind::ExplicitNestedArray,
                            location,
                        });
                    }
                    if reference_target(item).is_some() {
                        check_authoring_references(
                            document,
                            item,
                            "gradient",
                            &location,
                            depth + 1,
                        )?;
                    } else {
                        check_fields(
                            document,
                            item,
                            &[("color", "color"), ("position", "number")],
                            &location,
                            depth + 1,
                        )?;
                    }
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn type_error(error: crate::TypeError) -> DocumentError {
    let kind = match error.kind {
        TypeErrorKind::MissingType => DocumentErrorKind::MissingType,
        TypeErrorKind::InvalidDocument | TypeErrorKind::InvalidTokenPath => {
            DocumentErrorKind::InvalidDocument
        }
        _ => DocumentErrorKind::InvalidReference,
    };
    DocumentError {
        kind,
        location: error.location,
    }
}

fn check_fields(
    document: &Value,
    raw: &Value,
    fields: &[(&str, &str)],
    path: &str,
    depth: usize,
) -> Result<(), DocumentError> {
    for (name, kind) in fields {
        check_field(document, raw, name, kind, path, depth)?;
    }
    Ok(())
}

fn check_field(
    document: &Value,
    raw: &Value,
    name: &str,
    kind: &str,
    path: &str,
    depth: usize,
) -> Result<(), DocumentError> {
    if let Some(value) = raw.get(name) {
        check_authoring_references(
            document,
            value,
            kind,
            &format!("{path}/{}", escape_pointer_segment(name)),
            depth + 1,
        )?;
    }
    Ok(())
}

fn check_array_field(
    document: &Value,
    raw: &Value,
    name: &str,
    kind: &str,
    path: &str,
    depth: usize,
) -> Result<(), DocumentError> {
    if let Some(value) = raw.get(name) {
        check_items(
            document,
            value,
            kind,
            &format!("{path}/{}", escape_pointer_segment(name)),
            depth + 1,
        )?;
    }
    Ok(())
}

fn check_items(
    document: &Value,
    raw: &Value,
    kind: &str,
    path: &str,
    depth: usize,
) -> Result<(), DocumentError> {
    if let Some(items) = raw.as_array() {
        for (index, item) in items.iter().enumerate() {
            check_authoring_references(
                document,
                item,
                kind,
                &format!("{path}/{index}"),
                depth + 1,
            )?;
        }
    }
    Ok(())
}

fn reference_target(raw: &Value) -> Option<String> {
    if let Some(text) = raw.as_str() {
        return text
            .strip_prefix('{')
            .and_then(|value| value.strip_suffix('}'))
            .and_then(|path| curly_path_to_pointer(path).ok());
    }
    raw.as_object()?
        .get("$ref")?
        .as_str()
        .and_then(|pointer| canonical_pointer(pointer).ok())
}

fn whole_token_target(document: &Value, pointer: &str) -> Option<String> {
    let candidate = pointer.strip_suffix("/$value").unwrap_or(pointer);
    if lookup(document, candidate).is_some_and(is_token) {
        let segments = parse_pointer(candidate).ok()?;
        return Some(segments.join("."));
    }
    None
}

fn lookup<'a>(document: &'a Value, pointer: &str) -> Option<&'a Value> {
    let mut current = document;
    for segment in parse_pointer(pointer).ok()? {
        current = match current {
            Value::Object(object) => object.get(&segment)?,
            Value::Array(items) => items.get(crate::parse_index(&segment)?)?,
            _ => return None,
        };
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/tokens/document-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let source = vector["document"].clone();
            let result = resolve_token_document(&vector["document"]);
            if let Some(expected) = vector.get("expected") {
                let actual = Value::Object(
                    result
                        .unwrap()
                        .into_iter()
                        .map(|(path, token)| {
                            (path, serde_json::json!({"token_type": token.token_type, "value": token.value}))
                        })
                        .collect(),
                );
                assert_eq!(actual, *expected, "{}", vector["name"]);
            } else {
                let errors = result.unwrap_err();
                assert!(
                    errors
                        .iter()
                        .any(|error| format!("{:?}", error.kind)
                            == vector["error"].as_str().unwrap()),
                    "{}: {errors:?}",
                    vector["name"]
                );
                if let Some(location) = vector.get("location") {
                    assert!(
                        errors
                            .iter()
                            .any(|error| error.location == location.as_str().unwrap()),
                        "{}: {errors:?}",
                        vector["name"]
                    );
                }
            }
            assert_eq!(vector["document"], source, "{}", vector["name"]);
        }
    }
}
