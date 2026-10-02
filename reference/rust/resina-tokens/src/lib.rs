use serde_json::{Map, Value};
use std::fmt;

mod document;
mod extensions;
mod parse;
mod source;
mod structure;
mod types;
mod values;

pub use document::{DocumentError, DocumentErrorKind, ResolvedToken, resolve_token_document};
pub use extensions::{ExtensionError, ExtensionErrorKind, materialize_group_extensions};
pub use parse::parse_token_document;
pub use source::{TokenSourceError, resolve_token_source};
pub use structure::{StructureError, StructureErrorKind, validate_document_structure};
pub use types::{TypeError, TypeErrorKind, resolve_token_type};
pub use values::{ValueError, ValueErrorKind, validate_resolved_value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveErrorKind {
    InvalidTokenPath,
    InvalidReference,
    MissingTarget,
    NonTokenTarget,
    ConflictingValueAndReference,
    CircularReference,
    ReferenceDepthExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveError {
    pub kind: ResolveErrorKind,
    pub location: String,
}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?} at {}", self.kind, self.location)
    }
}

impl std::error::Error for ResolveError {}

pub fn resolve_token_value(document: &Value, path: &str) -> Result<Value, ResolveError> {
    let pointer = curly_path_to_pointer(path).map_err(|kind| ResolveError {
        kind,
        location: path.to_owned(),
    })?;
    let resolver = Resolver { document };
    let target = resolver.lookup(&pointer)?;
    if !is_token(target) {
        return Err(ResolveError {
            kind: ResolveErrorKind::NonTokenTarget,
            location: pointer,
        });
    }
    resolver.resolve_at(&pointer, &mut Vec::new())
}

struct Resolver<'a> {
    document: &'a Value,
}

const MAX_REFERENCE_DEPTH: usize = 256;

impl Resolver<'_> {
    fn resolve_at(&self, pointer: &str, stack: &mut Vec<String>) -> Result<Value, ResolveError> {
        self.resolve_at_with_mode(pointer, stack, true)
    }

    fn resolve_pointer_at(
        &self,
        pointer: &str,
        stack: &mut Vec<String>,
    ) -> Result<Value, ResolveError> {
        self.resolve_at_with_mode(pointer, stack, false)
    }

    fn resolve_at_with_mode(
        &self,
        pointer: &str,
        stack: &mut Vec<String>,
        token_value: bool,
    ) -> Result<Value, ResolveError> {
        let canonical = canonical_pointer(pointer).map_err(|kind| ResolveError {
            kind,
            location: pointer.to_owned(),
        })?;
        if stack.contains(&canonical) {
            return Err(ResolveError {
                kind: ResolveErrorKind::CircularReference,
                location: canonical,
            });
        }
        if stack.len() >= MAX_REFERENCE_DEPTH {
            return Err(ResolveError {
                kind: ResolveErrorKind::ReferenceDepthExceeded,
                location: canonical,
            });
        }
        let target = self.lookup(&canonical)?;
        stack.push(canonical.clone());
        let result = match target {
            value if !token_value && !self.is_token_value_location(&canonical) => Ok(value.clone()),
            Value::Object(object) if token_value && is_token(target) => {
                let value = object.get("$value");
                let reference = object.get("$ref");
                match (value, reference) {
                    (Some(_), Some(_)) => Err(ResolveError {
                        kind: ResolveErrorKind::ConflictingValueAndReference,
                        location: canonical,
                    }),
                    (Some(value), None) => {
                        self.resolve_inline(value, stack, &format!("{canonical}/$value"))
                    }
                    (None, Some(Value::String(reference))) => {
                        self.follow_pointer(reference, stack, &canonical)
                    }
                    (None, Some(_)) => Err(ResolveError {
                        kind: ResolveErrorKind::InvalidReference,
                        location: canonical,
                    }),
                    (None, None) => unreachable!(),
                }
            }
            value => self.resolve_inline(value, stack, pointer),
        };
        stack.pop();
        result
    }

    fn resolve_inline(
        &self,
        value: &Value,
        stack: &mut Vec<String>,
        location: &str,
    ) -> Result<Value, ResolveError> {
        match value {
            Value::String(text) if text.starts_with('{') => {
                let inner = text
                    .strip_prefix('{')
                    .and_then(|text| text.strip_suffix('}'))
                    .ok_or_else(|| ResolveError {
                        kind: ResolveErrorKind::InvalidReference,
                        location: location.to_owned(),
                    })?;
                let pointer = curly_path_to_pointer(inner).map_err(|kind| ResolveError {
                    kind,
                    location: location.to_owned(),
                })?;
                let target = self.lookup(&pointer)?;
                if !is_token(target) {
                    return Err(ResolveError {
                        kind: ResolveErrorKind::NonTokenTarget,
                        location: pointer,
                    });
                }
                if target.get("$value").is_none() {
                    return Err(ResolveError {
                        kind: ResolveErrorKind::MissingTarget,
                        location: format!("{pointer}/$value"),
                    });
                }
                self.resolve_at(&pointer, stack)
            }
            Value::Object(object) if object.contains_key("$ref") => {
                match (object.len(), object.get("$ref")) {
                    (1, Some(Value::String(reference))) => {
                        self.follow_pointer(reference, stack, location)
                    }
                    _ => Err(ResolveError {
                        kind: ResolveErrorKind::InvalidReference,
                        location: location.to_owned(),
                    }),
                }
            }
            Value::Object(object) => {
                let mut resolved = Map::new();
                for (name, child) in object {
                    let child_location = format!("{location}/{}", escape_pointer_segment(name));
                    resolved.insert(
                        name.clone(),
                        self.resolve_inline(child, stack, &child_location)?,
                    );
                }
                Ok(Value::Object(resolved))
            }
            Value::Array(items) => items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    self.resolve_inline(item, stack, &format!("{location}/{index}"))
                })
                .collect(),
            _ => Ok(value.clone()),
        }
    }

    fn lookup(&self, pointer: &str) -> Result<&Value, ResolveError> {
        let segments = parse_pointer(pointer).map_err(|kind| ResolveError {
            kind,
            location: pointer.to_owned(),
        })?;
        let mut current = self.document;
        for segment in segments {
            current = match current {
                Value::Object(object) => object.get(&segment),
                Value::Array(array) => parse_index(&segment).and_then(|index| array.get(index)),
                _ => None,
            }
            .ok_or_else(|| ResolveError {
                kind: ResolveErrorKind::MissingTarget,
                location: pointer.to_owned(),
            })?;
        }
        Ok(current)
    }

    fn is_token_value_location(&self, pointer: &str) -> bool {
        let Ok(segments) = parse_pointer(pointer) else {
            return false;
        };
        for (index, segment) in segments.iter().enumerate() {
            if segment == "$value" && token_at_segments(self.document, &segments[..index]).is_some()
            {
                return true;
            }
        }
        false
    }

    fn follow_pointer(
        &self,
        reference: &str,
        stack: &mut Vec<String>,
        source_location: &str,
    ) -> Result<Value, ResolveError> {
        canonical_pointer(reference).map_err(|kind| ResolveError {
            kind,
            location: source_location.to_owned(),
        })?;
        self.resolve_pointer_at(reference, stack)
    }
}

fn is_token(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.contains_key("$value") || object.contains_key("$ref"))
}

fn token_at_pointer<'a>(document: &'a Value, pointer: &str) -> Option<&'a Value> {
    let segments = parse_pointer(pointer).ok()?;
    token_at_segments(document, &segments)
}

fn token_at_segments<'a>(document: &'a Value, segments: &[String]) -> Option<&'a Value> {
    let (name, parents) = segments.split_last()?;
    let mut group = document;
    for parent in parents {
        if parent.starts_with('$') || is_token(group) {
            return None;
        }
        group = group.as_object()?.get(parent)?;
    }
    if is_token(group) || (name.starts_with('$') && name != "$root") {
        return None;
    }
    let token = group.as_object()?.get(name)?;
    is_token(token).then_some(token)
}

fn curly_path_to_pointer(path: &str) -> Result<String, ResolveErrorKind> {
    if path.is_empty() {
        return Err(ResolveErrorKind::InvalidTokenPath);
    }
    let mut pointer = String::from("#");
    for segment in path.split('.') {
        if segment.is_empty()
            || segment.contains(['{', '}', '.'])
            || (segment.starts_with('$') && segment != "$root")
        {
            return Err(ResolveErrorKind::InvalidTokenPath);
        }
        pointer.push('/');
        pointer.push_str(&escape_pointer_segment(segment));
    }
    Ok(pointer)
}

fn canonical_pointer(pointer: &str) -> Result<String, ResolveErrorKind> {
    let segments = parse_pointer(pointer)?;
    let mut canonical = String::from("#");
    for segment in segments {
        canonical.push('/');
        canonical.push_str(&escape_pointer_segment(&segment));
    }
    Ok(canonical)
}

fn escape_pointer_segment(segment: &str) -> String {
    segment
        .replace('~', "~0")
        .replace('/', "~1")
        .replace('%', "%25")
        .replace('#', "%23")
}

fn parse_pointer(pointer: &str) -> Result<Vec<String>, ResolveErrorKind> {
    let fragment = pointer
        .strip_prefix('#')
        .ok_or(ResolveErrorKind::InvalidReference)?;
    let decoded = percent_decode(fragment)?;
    if decoded.is_empty() {
        return Ok(Vec::new());
    }
    let path = decoded
        .strip_prefix('/')
        .ok_or(ResolveErrorKind::InvalidReference)?;
    path.split('/').map(unescape_segment).collect()
}

fn percent_decode(fragment: &str) -> Result<String, ResolveErrorKind> {
    let bytes = fragment.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = bytes
                .get(index + 1..index + 3)
                .ok_or(ResolveErrorKind::InvalidReference)?;
            let digit = |byte: u8| -> Option<u8> {
                match byte {
                    b'0'..=b'9' => Some(byte - b'0'),
                    b'a'..=b'f' => Some(byte - b'a' + 10),
                    b'A'..=b'F' => Some(byte - b'A' + 10),
                    _ => None,
                }
            };
            let high = digit(hex[0]).ok_or(ResolveErrorKind::InvalidReference)?;
            let low = digit(hex[1]).ok_or(ResolveErrorKind::InvalidReference)?;
            decoded.push(high * 16 + low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| ResolveErrorKind::InvalidReference)
}

fn unescape_segment(segment: &str) -> Result<String, ResolveErrorKind> {
    let mut unescaped = String::new();
    let mut chars = segment.chars();
    while let Some(character) = chars.next() {
        if character == '~' {
            match chars.next() {
                Some('0') => unescaped.push('~'),
                Some('1') => unescaped.push('/'),
                _ => return Err(ResolveErrorKind::InvalidReference),
            }
        } else {
            unescaped.push(character);
        }
    }
    Ok(unescaped)
}

fn parse_index(segment: &str) -> Option<usize> {
    if segment == "0"
        || (!segment.starts_with('0') && segment.bytes().all(|byte| byte.is_ascii_digit()))
    {
        segment.parse().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/tokens/reference-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let document = &vector["document"];
            let path = vector["path"].as_str().unwrap();
            let result = resolve_token_value(document, path);
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
        }
    }

    #[test]
    fn source_document_is_preserved() {
        let document: Value = serde_json::json!({
            "base": { "$type": "number", "$value": 1 },
            "alias": { "$type": "number", "$value": "{base}" }
        });
        let original = document.clone();
        assert_eq!(
            resolve_token_value(&document, "alias").unwrap(),
            Value::from(1)
        );
        assert_eq!(document, original);
    }

    #[test]
    fn nested_reference_error_identifies_the_property() {
        let document = serde_json::json!({
            "value": {
                "$type": "dimension",
                "$value": {
                    "value": { "$ref": "broken" },
                    "unit": "px"
                }
            }
        });
        let error = resolve_token_value(&document, "value").unwrap_err();
        assert_eq!(error.kind, ResolveErrorKind::InvalidReference);
        assert_eq!(error.location, "#/value/$value/value");
    }

    #[test]
    fn deep_reference_chain_fails_diagnostically() {
        let mut document = Map::new();
        for index in 0..MAX_REFERENCE_DEPTH {
            document.insert(
                format!("n{index}"),
                serde_json::json!({ "$type": "number", "$value": format!("{{n{}}}", index + 1) }),
            );
        }
        document.insert(
            format!("n{MAX_REFERENCE_DEPTH}"),
            serde_json::json!({ "$type": "number", "$value": 1 }),
        );
        let error = resolve_token_value(&Value::Object(document), "n0").unwrap_err();
        assert_eq!(error.kind, ResolveErrorKind::ReferenceDepthExceeded);
    }
}
