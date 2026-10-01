use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SpatialRole {
    #[serde(rename = "space.control.inline")]
    ControlInline,
    #[serde(rename = "space.control.block")]
    ControlBlock,
    #[serde(rename = "space.container.inner")]
    ContainerInner,
    #[serde(rename = "space.container.outer")]
    ContainerOuter,
    #[serde(rename = "space.group")]
    Group,
    #[serde(rename = "space.section")]
    Section,
    #[serde(rename = "space.page")]
    Page,
}

impl SpatialRole {
    pub const ALL: [Self; 7] = [
        Self::ControlInline,
        Self::ControlBlock,
        Self::ContainerInner,
        Self::ContainerOuter,
        Self::Group,
        Self::Section,
        Self::Page,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "SpatialAssignmentsInput")]
pub struct SpatialAssignments {
    schema_version: String,
    roles: BTreeMap<SpatialRole, TokenPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SpatialAssignmentsInput {
    schema_version: String,
    roles: BTreeMap<SpatialRole, TokenPath>,
}

impl TryFrom<SpatialAssignmentsInput> for SpatialAssignments {
    type Error = &'static str;

    fn try_from(input: SpatialAssignmentsInput) -> Result<Self, Self::Error> {
        if input.schema_version != "0.1.0" {
            return Err("schemaVersion must be 0.1.0");
        }
        if SpatialRole::ALL
            .iter()
            .any(|role| !input.roles.contains_key(role))
        {
            return Err("missing spatial role");
        }
        Ok(Self {
            schema_version: input.schema_version,
            roles: input.roles,
        })
    }
}

impl SpatialAssignments {
    pub fn token_path_for(&self, role: SpatialRole) -> &str {
        self.roles[&role].0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
struct TokenPath(String);

impl TryFrom<String> for TokenPath {
    type Error = &'static str;

    fn try_from(path: String) -> Result<Self, Self::Error> {
        if path.is_empty() {
            Err("spatial role token path must not be empty")
        } else {
            Ok(Self(path))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn assignment_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/spatial/assignment-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = serde_json::from_value::<SpatialAssignments>(vector["document"].clone());
            if let Some(expected) = vector.get("expected") {
                let assignments = result.unwrap();
                for role in SpatialRole::ALL {
                    let name = serde_json::to_value(role).unwrap();
                    assert_eq!(
                        assignments.token_path_for(role),
                        expected[name.as_str().unwrap()].as_str().unwrap(),
                        "{}: {name}",
                        vector["name"]
                    );
                }
                assert_eq!(
                    serde_json::to_value(&assignments).unwrap(),
                    vector["document"]
                );
            } else {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains(vector["error"].as_str().unwrap()),
                    "{}",
                    vector["name"]
                );
            }
        }
    }
}
