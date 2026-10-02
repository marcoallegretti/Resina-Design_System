use crate::ElevationRole;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElevationDepthAssignments {
    #[serde(deserialize_with = "deserialize_version")]
    schema_version: String,
    roles: ElevationDepthRoles,
}

impl ElevationDepthAssignments {
    pub fn token_path_for(&self, role: ElevationRole) -> &str {
        match role {
            ElevationRole::Embedded => self.roles.embedded.as_str(),
            ElevationRole::Base => self.roles.base.as_str(),
            ElevationRole::Raised => self.roles.raised.as_str(),
            ElevationRole::Floating => self.roles.floating.as_str(),
            ElevationRole::Overlay => self.roles.overlay.as_str(),
            ElevationRole::Modal => self.roles.modal.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ElevationDepthRoles {
    embedded: TokenPath,
    base: TokenPath,
    raised: TokenPath,
    floating: TokenPath,
    overlay: TokenPath,
    modal: TokenPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
struct TokenPath(String);

impl TokenPath {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TokenPath {
    type Error = &'static str;

    fn try_from(path: String) -> Result<Self, Self::Error> {
        if path.is_empty() {
            Err("elevation depth token path must not be empty")
        } else {
            Ok(Self(path))
        }
    }
}

fn deserialize_version<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let version = String::deserialize(deserializer)?;
    if version == "0.1.0" {
        Ok(version)
    } else {
        Err(D::Error::custom("schemaVersion must be 0.1.0"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn assignment_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/elevation/depth-assignment-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result =
                serde_json::from_value::<ElevationDepthAssignments>(vector["document"].clone());
            if let Some(expected) = vector.get("expected") {
                let assignments = result.unwrap();
                for role in ElevationRole::ALL {
                    let name = serde_json::to_value(role).unwrap();
                    assert_eq!(
                        assignments.token_path_for(role),
                        expected[name.as_str().unwrap()].as_str().unwrap(),
                        "{}: {name}",
                        vector["name"]
                    );
                }
                assert_eq!(
                    serde_json::to_value(assignments).unwrap(),
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
