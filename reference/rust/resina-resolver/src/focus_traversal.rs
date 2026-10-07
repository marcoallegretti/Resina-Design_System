use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FocusDirection {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy)]
pub struct FocusTarget<'a> {
    pub id: &'a str,
    pub eligible: bool,
}

pub struct FocusTraversalInput<'a> {
    pub targets: &'a [FocusTarget<'a>],
    pub current: Option<&'a str>,
    pub direction: FocusDirection,
    pub wrap: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusTraversalResult {
    schema_version: &'static str,
    target_id: Option<String>,
}

impl FocusTraversalResult {
    pub fn target_id(&self) -> Option<&str> {
        self.target_id.as_deref()
    }
}

#[derive(Debug)]
pub enum FocusTraversalError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    InvalidShape(&'static str, &'static str),
    InvalidCurrent,
    EmptyId(usize),
    DuplicateId(String),
    UnknownCurrent(String),
}

impl fmt::Display for FocusTraversalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "focus traversal parse failed: {error}"),
            Self::Request(error) => write!(formatter, "invalid focus traversal request: {error}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidShape(field, shape) => write!(
                formatter,
                "invalid focus traversal request: {field} must be a JSON {shape}"
            ),
            Self::InvalidCurrent => {
                formatter.write_str("current must be a target identifier or null")
            }
            Self::EmptyId(index) => write!(formatter, "targets[{index}].id must not be empty"),
            Self::DuplicateId(id) => write!(formatter, "duplicate focus target identifier {id:?}"),
            Self::UnknownCurrent(id) => {
                write!(formatter, "current identifier {id:?} is not in targets")
            }
        }
    }
}

impl std::error::Error for FocusTraversalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) | Self::Request(error) => Some(error),
            _ => None,
        }
    }
}

pub fn resolve_focus_traversal(
    input: FocusTraversalInput<'_>,
) -> Result<FocusTraversalResult, FocusTraversalError> {
    let mut ids = HashSet::new();
    let mut current_index = None;
    for (index, target) in input.targets.iter().enumerate() {
        if target.id.is_empty() {
            return Err(FocusTraversalError::EmptyId(index));
        }
        if !ids.insert(target.id) {
            return Err(FocusTraversalError::DuplicateId(target.id.to_owned()));
        }
        if input.current == Some(target.id) {
            current_index = Some(index);
        }
    }
    if let Some(current) = input.current
        && current_index.is_none()
    {
        return Err(FocusTraversalError::UnknownCurrent(current.to_owned()));
    }
    let eligible = |target: &&FocusTarget<'_>| target.eligible;
    let selected = match (input.direction, current_index) {
        (FocusDirection::Forward, None) => input.targets.iter().find(eligible),
        (FocusDirection::Backward, None) => input.targets.iter().rev().find(eligible),
        (FocusDirection::Forward, Some(index)) => input.targets[index + 1..]
            .iter()
            .find(eligible)
            .or_else(|| {
                if input.wrap {
                    input.targets[..=index].iter().find(eligible)
                } else {
                    None
                }
            }),
        (FocusDirection::Backward, Some(index)) => input.targets[..index]
            .iter()
            .rev()
            .find(eligible)
            .or_else(|| {
                if input.wrap {
                    input.targets[index..].iter().rev().find(eligible)
                } else {
                    None
                }
            }),
    };
    Ok(FocusTraversalResult {
        schema_version: "0.1.0",
        target_id: selected.map(|target| target.id.to_owned()),
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    id: String,
    eligible: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    targets: Vec<Target>,
    current: serde_json::Value,
    direction: FocusDirection,
    wrap: bool,
}

pub fn resolve_focus_traversal_source(
    source: &str,
) -> Result<FocusTraversalResult, FocusTraversalError> {
    let value = parse_token_document(source).map_err(FocusTraversalError::Parse)?;
    if !value.is_object() {
        return Err(FocusTraversalError::InvalidShape("root", "object"));
    }
    if let Some(targets) = value["targets"].as_array()
        && targets.iter().any(|target| !target.is_object())
    {
        return Err(FocusTraversalError::InvalidShape("targets[]", "object"));
    }
    if value
        .get("direction")
        .is_some_and(|direction| !direction.is_string())
    {
        return Err(FocusTraversalError::InvalidShape("direction", "string"));
    }
    let request: Request = serde_json::from_value(value).map_err(FocusTraversalError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(FocusTraversalError::UnsupportedVersion);
    }
    let current = match &request.current {
        serde_json::Value::Null => None,
        serde_json::Value::String(id) => Some(id.as_str()),
        _ => return Err(FocusTraversalError::InvalidCurrent),
    };
    let targets: Vec<_> = request
        .targets
        .iter()
        .map(|target| FocusTarget {
            id: &target.id,
            eligible: target.eligible,
        })
        .collect();
    resolve_focus_traversal(FocusTraversalInput {
        targets: &targets,
        current,
        direction: request.direction,
        wrap: request.wrap,
    })
}
