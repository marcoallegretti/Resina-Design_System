use crate::{
    DocumentError, ResolvedToken, parse_pointer, parse_token_document, resolve_token_document,
    validate_document_structure,
};
use serde_json::{Map, Value};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    fmt,
};

const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
const MAX_COMPOSED_NODES: usize = 100_000;
const MAX_COMPOSED_TEXT_BYTES: usize = 16 * 1024 * 1024;
const MAX_REFERENCE_DEPTH: usize = 128;

#[derive(Debug)]
pub enum ResolverModuleError {
    Parse(String),
    Invalid(String),
    Tokens(Vec<DocumentError>),
}

impl fmt::Display for ResolverModuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(message) | Self::Invalid(message) => formatter.write_str(message),
            Self::Tokens(errors) => {
                formatter.write_str("composed token document is invalid")?;
                for error in errors {
                    write!(formatter, "\n  {error}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ResolverModuleError {}

pub fn resolve_resolver_module_source(
    resolver_source: &str,
    input_source: &str,
    external_sources: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, ResolvedToken>, ResolverModuleError> {
    let bytes = resolver_source
        .len()
        .checked_add(input_source.len())
        .and_then(|size| {
            external_sources
                .values()
                .try_fold(size, |size, source| size.checked_add(source.len()))
        })
        .ok_or_else(|| {
            ResolverModuleError::Invalid("resolver source size limit exceeded".into())
        })?;
    if bytes > MAX_SOURCE_BYTES {
        return Err(ResolverModuleError::Invalid(
            "resolver source size limit exceeded".into(),
        ));
    }
    let resolver = parse_token_document(resolver_source)
        .map_err(|error| ResolverModuleError::Parse(format!("resolver: {error}")))?;
    let input = parse_token_document(input_source)
        .map_err(|error| ResolverModuleError::Parse(format!("input: {error}")))?;
    let composed = Composer {
        resolver: &resolver,
        sources: external_sources,
        remaining_nodes: Cell::new(MAX_COMPOSED_NODES),
        remaining_text_bytes: Cell::new(MAX_COMPOSED_TEXT_BYTES),
    }
    .compose(&input)?;
    resolve_token_document(&composed).map_err(ResolverModuleError::Tokens)
}

struct Composer<'a> {
    resolver: &'a Value,
    sources: &'a BTreeMap<String, String>,
    remaining_nodes: Cell<usize>,
    remaining_text_bytes: Cell<usize>,
}

impl Composer<'_> {
    fn compose(&self, input: &Value) -> Result<Value, ResolverModuleError> {
        let root = object(self.resolver, "resolver")?;
        if root.get("version").and_then(Value::as_str) != Some("2025.10") {
            return invalid("resolver version must be 2025.10");
        }
        for key in root.keys() {
            if !matches!(
                key.as_str(),
                "version"
                    | "name"
                    | "description"
                    | "sets"
                    | "modifiers"
                    | "resolutionOrder"
                    | "$schema"
                    | "$defs"
            ) {
                return invalid(format!("unknown resolver property {key}"));
            }
        }
        for key in ["name", "description", "$schema"] {
            if root.get(key).is_some_and(|value| !value.is_string()) {
                return invalid(format!("resolver {key} must be a string"));
            }
        }
        let empty = Map::new();
        let sets = match root.get("sets") {
            Some(value) => object(value, "sets")?,
            None => &empty,
        };
        let modifiers = match root.get("modifiers") {
            Some(value) => object(value, "modifiers")?,
            None => &empty,
        };
        let order = root
            .get("resolutionOrder")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ResolverModuleError::Invalid("resolutionOrder must be an array".into())
            })?;
        if order.is_empty() {
            return invalid("resolutionOrder must contain at least one item");
        }
        for (name, definition) in sets {
            if name.is_empty() {
                return invalid("set name must not be empty");
            }
            validate_definition(definition, "set", name, false)?;
        }
        for (name, definition) in modifiers {
            if name.is_empty() {
                return invalid("modifier name must not be empty");
            }
            validate_definition(definition, "modifier", name, false)?;
        }
        let inputs = object(input, "input")?;
        for (name, value) in inputs {
            if !value.is_string() {
                return invalid(format!("input {name} must be a string"));
            }
        }
        let mut selected = BTreeMap::new();
        let mut names = BTreeSet::new();
        let mut entries = Vec::new();
        for (index, item) in order.iter().enumerate() {
            let path = format!("#/resolutionOrder/{index}");
            let (name, kind, definition) = self.order_entry(item, &path)?;
            validate_definition(&definition, kind, &name, item.get("$ref").is_none())?;
            if !names.insert(name.clone()) {
                return invalid(format!("duplicate resolutionOrder name {name}"));
            }
            entries.push((name, kind, definition));
        }
        let effective_modifier_names = entries
            .iter()
            .filter(|(_, kind, _)| *kind == "modifier")
            .map(|(name, _, _)| name.as_str())
            .collect::<BTreeSet<_>>();
        for (name, definition) in modifiers {
            if !effective_modifier_names.contains(name.as_str()) {
                self.select_modifier(name, definition, inputs, &mut selected)?;
            }
        }
        for (name, kind, definition) in &entries {
            if *kind == "modifier" {
                self.select_modifier(name, definition, inputs, &mut selected)?;
            }
        }
        for name in inputs.keys() {
            if !selected.contains_key(name) {
                return invalid(format!("unknown modifier input {name}"));
            }
        }
        let mut result = Value::Object(Map::new());
        let mut stack = Vec::new();
        for (name, kind, definition) in entries {
            let sources = if kind == "set" {
                set_sources(&definition, &name)?
            } else {
                let context = selected.get(&name).ok_or_else(|| {
                    ResolverModuleError::Invalid(format!("missing modifier input {name}"))
                })?;
                modifier_context(&definition, &name, context)?
            };
            self.compose_sources(&mut result, sources, &mut stack)?;
        }
        check_size(&result)?;
        Ok(result)
    }

    fn order_entry(
        &self,
        item: &Value,
        path: &str,
    ) -> Result<(String, &'static str, Value), ResolverModuleError> {
        let fields = object(item, path)?;
        if let Some(reference) = fields.get("$ref") {
            let reference = reference.as_str().ok_or_else(|| {
                ResolverModuleError::Invalid(format!("{path}/$ref must be a string"))
            })?;
            let segments = parse_pointer(reference)
                .map_err(|_| ResolverModuleError::Invalid(format!("{path}/$ref is invalid")))?;
            if segments.len() != 2 || !matches!(segments[0].as_str(), "sets" | "modifiers") {
                return invalid(format!("{path}/$ref must name a set or modifier"));
            }
            let mut definition = self.local_reference(reference, &mut Vec::new())?;
            let target = definition.as_object_mut().ok_or_else(|| {
                ResolverModuleError::Invalid(format!("{path}/$ref does not name an object"))
            })?;
            for (key, value) in fields {
                if key != "$ref" {
                    target.insert(key.clone(), value.clone());
                }
            }
            let kind = if segments[0] == "sets" {
                "set"
            } else {
                "modifier"
            };
            return Ok((segments[1].clone(), kind, definition));
        }
        let name = fields
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                ResolverModuleError::Invalid(format!("{path}/name must be a nonempty string"))
            })?;
        let kind = fields
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| ResolverModuleError::Invalid(format!("{path}/type is required")))?;
        let kind = match kind {
            "set" => "set",
            "modifier" => "modifier",
            _ => return invalid(format!("{path}/type must be set or modifier")),
        };
        if fields.contains_key("$ref") {
            return invalid(format!("{path} cannot be both inline and referenced"));
        }
        Ok((name.into(), kind, item.clone()))
    }

    fn select_modifier(
        &self,
        name: &str,
        definition: &Value,
        inputs: &Map<String, Value>,
        selected: &mut BTreeMap<String, String>,
    ) -> Result<(), ResolverModuleError> {
        let fields = object(definition, name)?;
        let contexts = fields
            .get("contexts")
            .and_then(Value::as_object)
            .filter(|contexts| !contexts.is_empty())
            .ok_or_else(|| {
                ResolverModuleError::Invalid(format!("modifier {name} needs contexts"))
            })?;
        let choice = inputs
            .get(name)
            .and_then(Value::as_str)
            .or_else(|| fields.get("default").and_then(Value::as_str))
            .ok_or_else(|| {
                ResolverModuleError::Invalid(format!("missing modifier input {name}"))
            })?;
        if !contexts.contains_key(choice) {
            return invalid(format!("invalid context {choice} for modifier {name}"));
        }
        selected.insert(name.into(), choice.into());
        Ok(())
    }

    fn compose_sources(
        &self,
        output: &mut Value,
        sources: &[Value],
        stack: &mut Vec<String>,
    ) -> Result<(), ResolverModuleError> {
        for source in sources {
            let fields = object(source, "token source")?;
            if let Some(reference) = fields.get("$ref") {
                let reference = reference.as_str().ok_or_else(|| {
                    ResolverModuleError::Invalid("token source $ref must be a string".into())
                })?;
                let set_reference = parse_pointer(reference)
                    .ok()
                    .is_some_and(|segments| segments.len() == 2 && segments[0] == "sets");
                if set_reference {
                    if stack.contains(&reference.to_owned()) || stack.len() >= MAX_REFERENCE_DEPTH {
                        return invalid(format!("circular or deep set reference {reference}"));
                    }
                    stack.push(reference.into());
                    let mut set = self.local_reference(reference, stack)?;
                    apply_overrides(&mut set, fields, reference)?;
                    self.compose_sources(output, set_sources(&set, reference)?, stack)?;
                    stack.pop();
                } else {
                    if reference.starts_with("#/modifiers/")
                        || reference.starts_with("#/resolutionOrder/")
                    {
                        return invalid(format!("invalid token source reference {reference}"));
                    }
                    let mut document = self.source_reference(reference, stack)?;
                    apply_overrides(&mut document, fields, reference)?;
                    self.merge_source(output, document)?;
                }
            } else {
                self.merge_source(output, source.clone())?;
            }
        }
        Ok(())
    }

    fn merge_source(&self, output: &mut Value, source: Value) -> Result<(), ResolverModuleError> {
        let mut remaining = self.remaining_nodes.get();
        let mut remaining_text = self.remaining_text_bytes.get();
        if !charge_value(&source, &mut remaining, &mut remaining_text) {
            return invalid("token composition size limit exceeded");
        }
        self.remaining_nodes.set(remaining);
        self.remaining_text_bytes.set(remaining_text);
        merge_groups(output, source)
    }

    fn source_reference(
        &self,
        reference: &str,
        stack: &mut Vec<String>,
    ) -> Result<Value, ResolverModuleError> {
        if reference.starts_with('#') {
            return self.local_reference(reference, stack);
        }
        let (uri, fragment) = reference.split_once('#').unwrap_or((reference, ""));
        if uri.is_empty() {
            return invalid(format!("invalid source reference {reference}"));
        }
        let source = self
            .sources
            .get(uri)
            .ok_or_else(|| ResolverModuleError::Invalid(format!("missing source {uri}")))?;
        let document = parse_token_document(source)
            .map_err(|error| ResolverModuleError::Parse(format!("{uri}: {error}")))?;
        validate_document_structure(&document).map_err(|errors| {
            ResolverModuleError::Invalid(format!(
                "invalid external token source {uri}: {}",
                errors[0]
            ))
        })?;
        if fragment.is_empty() {
            Ok(document)
        } else {
            pointer_value(&document, &format!("#{fragment}"))
        }
    }

    fn local_reference(
        &self,
        reference: &str,
        stack: &mut Vec<String>,
    ) -> Result<Value, ResolverModuleError> {
        if stack.len() >= MAX_REFERENCE_DEPTH {
            return invalid(format!("reference depth exceeded at {reference}"));
        }
        let segments = parse_pointer(reference)
            .map_err(|_| ResolverModuleError::Invalid(format!("invalid reference {reference}")))?;
        if segments
            .first()
            .is_some_and(|segment| segment == "resolutionOrder")
        {
            return invalid(format!("invalid reference target {reference}"));
        }
        let value = pointer_value(self.resolver, reference)?;
        if let Some(fields) = value.as_object()
            && let Some(next) = fields.get("$ref").and_then(Value::as_str)
        {
            if stack.contains(&reference.to_owned()) {
                return invalid(format!("circular reference {reference}"));
            }
            stack.push(reference.into());
            let mut target = self.source_reference(next, stack)?;
            stack.pop();
            let target_fields = target.as_object_mut().ok_or_else(|| {
                ResolverModuleError::Invalid(format!("{reference} does not reference an object"))
            })?;
            for (name, value) in fields {
                if name != "$ref" {
                    target_fields.insert(name.clone(), value.clone());
                }
            }
            return Ok(target);
        }
        Ok(value)
    }
}

fn object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, ResolverModuleError> {
    value
        .as_object()
        .ok_or_else(|| ResolverModuleError::Invalid(format!("{path} must be an object")))
}

fn apply_overrides(
    target: &mut Value,
    fields: &Map<String, Value>,
    reference: &str,
) -> Result<(), ResolverModuleError> {
    let target = target.as_object_mut().ok_or_else(|| {
        ResolverModuleError::Invalid(format!("{reference} does not reference an object"))
    })?;
    for (name, value) in fields {
        if name != "$ref" {
            target.insert(name.clone(), value.clone());
        }
    }
    Ok(())
}

fn set_sources<'a>(definition: &'a Value, name: &str) -> Result<&'a [Value], ResolverModuleError> {
    definition
        .get("sources")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| ResolverModuleError::Invalid(format!("set {name} needs a sources array")))
}

fn modifier_context<'a>(
    definition: &'a Value,
    name: &str,
    choice: &str,
) -> Result<&'a [Value], ResolverModuleError> {
    definition
        .get("contexts")
        .and_then(Value::as_object)
        .and_then(|contexts| contexts.get(choice))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| {
            ResolverModuleError::Invalid(format!("modifier {name} lacks context {choice}"))
        })
}

fn validate_definition(
    definition: &Value,
    kind: &str,
    name: &str,
    inline: bool,
) -> Result<(), ResolverModuleError> {
    let fields = object(definition, name)?;
    for key in fields.keys() {
        let allowed = matches!(key.as_str(), "description" | "$extensions")
            || (inline && matches!(key.as_str(), "name" | "type"))
            || (kind == "set" && key == "sources")
            || (kind == "modifier" && matches!(key.as_str(), "contexts" | "default"));
        if !allowed {
            return invalid(format!("unknown {kind} {name} property {key}"));
        }
    }
    if fields
        .get("description")
        .is_some_and(|value| !value.is_string())
    {
        return invalid(format!("{kind} {name} description must be a string"));
    }
    if fields
        .get("$extensions")
        .is_some_and(|value| !value.is_object())
    {
        return invalid(format!("{kind} {name} $extensions must be an object"));
    }
    if kind == "set" {
        let sources = set_sources(definition, name)?;
        for source in sources {
            validate_source_shape(source)?;
        }
    } else {
        let contexts = fields
            .get("contexts")
            .and_then(Value::as_object)
            .filter(|contexts| !contexts.is_empty())
            .ok_or_else(|| {
                ResolverModuleError::Invalid(format!("modifier {name} needs contexts"))
            })?;
        for (context, sources) in contexts {
            if context.is_empty() {
                return invalid(format!("modifier {name} context name must not be empty"));
            }
            let sources = sources.as_array().ok_or_else(|| {
                ResolverModuleError::Invalid(format!(
                    "modifier {name} context {context} must be an array"
                ))
            })?;
            for source in sources {
                validate_source_shape(source)?;
            }
        }
        if let Some(default) = fields.get("default") {
            let default = default.as_str().ok_or_else(|| {
                ResolverModuleError::Invalid(format!("modifier {name} default must be a string"))
            })?;
            if !contexts.contains_key(default) {
                return invalid(format!("modifier {name} default {default} is unknown"));
            }
        }
    }
    Ok(())
}

fn validate_source_shape(source: &Value) -> Result<(), ResolverModuleError> {
    let fields = object(source, "token source")?;
    if let Some(reference) = fields.get("$ref") {
        let reference = reference.as_str().ok_or_else(|| {
            ResolverModuleError::Invalid("token source $ref must be a string".into())
        })?;
        let (uri, fragment) = reference.split_once('#').unwrap_or((reference, ""));
        if uri.is_empty() && !reference.starts_with('#') {
            return invalid(format!("invalid source reference {reference}"));
        }
        if reference.starts_with('#') || !fragment.is_empty() {
            let pointer = if reference.starts_with('#') {
                reference.to_owned()
            } else {
                format!("#{fragment}")
            };
            let segments = parse_pointer(&pointer).map_err(|_| {
                ResolverModuleError::Invalid(format!("invalid source reference {reference}"))
            })?;
            if reference.starts_with('#')
                && segments
                    .first()
                    .is_some_and(|segment| segment == "modifiers" || segment == "resolutionOrder")
            {
                return invalid(format!("invalid token source reference {reference}"));
            }
        }
    } else {
        validate_document_structure(source).map_err(|errors| {
            ResolverModuleError::Invalid(format!("invalid inline token source: {}", errors[0]))
        })?;
    }
    Ok(())
}

fn pointer_value(document: &Value, pointer: &str) -> Result<Value, ResolverModuleError> {
    let segments = parse_pointer(pointer)
        .map_err(|_| ResolverModuleError::Invalid(format!("invalid reference {pointer}")))?;
    let mut value = document;
    for segment in segments {
        value = match value {
            Value::Object(fields) => fields.get(&segment),
            Value::Array(items) => segment
                .parse::<usize>()
                .ok()
                .filter(|_| segment == "0" || !segment.starts_with('0'))
                .and_then(|index| items.get(index)),
            _ => None,
        }
        .ok_or_else(|| ResolverModuleError::Invalid(format!("missing reference {pointer}")))?;
    }
    Ok(value.clone())
}

fn merge_groups(output: &mut Value, source: Value) -> Result<(), ResolverModuleError> {
    let source = source
        .as_object()
        .ok_or_else(|| ResolverModuleError::Invalid("token source must be an object".into()))?;
    let output = output.as_object_mut().ok_or_else(|| {
        ResolverModuleError::Invalid("merged token source must be an object".into())
    })?;
    for (name, value) in source {
        match output.get_mut(name) {
            Some(previous) if !name.starts_with('$') && is_group(previous) && is_group(value) => {
                merge_groups(previous, value.clone())?
            }
            _ => {
                output.insert(name.clone(), value.clone());
            }
        }
    }
    Ok(())
}

fn is_group(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|fields| !fields.contains_key("$value") && !fields.contains_key("$ref"))
}

fn check_size(value: &Value) -> Result<(), ResolverModuleError> {
    let mut remaining = MAX_COMPOSED_NODES;
    let mut remaining_text = MAX_COMPOSED_TEXT_BYTES;
    if charge_value(value, &mut remaining, &mut remaining_text) {
        Ok(())
    } else {
        invalid("composed token document size limit exceeded")
    }
}

fn charge_value(value: &Value, remaining_nodes: &mut usize, remaining_text: &mut usize) -> bool {
    if *remaining_nodes == 0 {
        return false;
    }
    *remaining_nodes -= 1;
    match value {
        Value::Array(items) => items
            .iter()
            .all(|value| charge_value(value, remaining_nodes, remaining_text)),
        Value::Object(fields) => fields.iter().all(|(name, value)| {
            if let Some(bytes) = remaining_text.checked_sub(name.len()) {
                *remaining_text = bytes;
                charge_value(value, remaining_nodes, remaining_text)
            } else {
                false
            }
        }),
        Value::String(text) => {
            if let Some(bytes) = remaining_text.checked_sub(text.len()) {
                *remaining_text = bytes;
                true
            } else {
                false
            }
        }
        _ => true,
    }
}

fn invalid<T>(message: impl Into<String>) -> Result<T, ResolverModuleError> {
    Err(ResolverModuleError::Invalid(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolver_module_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/tokens/resolver-module-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let sources: BTreeMap<String, String> =
                serde_json::from_value(vector["externalSources"].clone()).unwrap();
            let result = resolve_resolver_module_source(
                vector["resolver"].as_str().unwrap(),
                vector["input"].as_str().unwrap(),
                &sources,
            );
            if let Some(expected) = vector.get("expected") {
                let tokens = result.unwrap_or_else(|error| panic!("{}: {error}", vector["name"]));
                let actual = tokens
                    .into_iter()
                    .map(|(path, token)| {
                        (
                            path,
                            json!({"token_type": token.token_type, "value": token.value}),
                        )
                    })
                    .collect::<Map<_, _>>();
                assert_eq!(Value::Object(actual), *expected, "{}", vector["name"]);
            } else {
                let error = result
                    .err()
                    .unwrap_or_else(|| panic!("{} unexpectedly succeeded", vector["name"]));
                assert!(
                    error
                        .to_string()
                        .contains(vector["errorContains"].as_str().unwrap()),
                    "{}: {error}",
                    vector["name"]
                );
            }
        }
    }

    #[test]
    fn authored_foundation_is_composable_without_copying_it() {
        let foundation = include_str!("../../../../tokens/foundation.json");
        let sources = BTreeMap::from([("foundation.json".into(), foundation.into())]);
        let resolver = r##"{
            "version": "2025.10",
            "sets": {"foundation": {"sources": [{"$ref": "foundation.json"}]}},
            "resolutionOrder": [{"$ref": "#/sets/foundation"}]
        }"##;
        let composed = resolve_resolver_module_source(resolver, "{}", &sources).unwrap();
        let direct = crate::resolve_token_source(foundation).unwrap();
        assert_eq!(composed, direct);
        assert_eq!(composed.len(), 24);
    }

    #[test]
    fn repeated_large_external_source_has_a_text_work_limit() {
        let source =
            json!({"font": {"$type": "fontFamily", "$value": "x".repeat(1_000_000)}}).to_string();
        let sources = BTreeMap::from([("large.json".into(), source)]);
        let resolver = json!({
            "version": "2025.10",
            "resolutionOrder": [{
                "type": "set",
                "name": "repeated",
                "sources": vec![json!({"$ref": "large.json"}); 20]
            }]
        })
        .to_string();
        let error = resolve_resolver_module_source(&resolver, "{}", &sources).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("token composition size limit exceeded")
        );
    }
}
