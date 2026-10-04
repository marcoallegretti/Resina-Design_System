use crate::ShapeIntent;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "ShapeFallbackAssignmentsInput")]
pub struct ShapeFallbackAssignments {
    schema_version: String,
    profiles: ShapeFallbackProfiles,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ShapeFallbackAssignmentsInput {
    schema_version: String,
    profiles: ShapeFallbackProfiles,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShapeFallbackProfiles {
    structural: ShapeFallbackProfile,
    soft: ShapeFallbackProfile,
    rounded: ShapeFallbackProfile,
    capsule: ShapeFallbackProfile,
    organic: ShapeFallbackProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    from = "ShapeFallbackProfileInput"
)]
pub enum ShapeFallbackProfile {
    Uniform { radius: String },
    Corners { radii: CornerTokenPaths },
    Capsule,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum ShapeFallbackProfileInput {
    Uniform {
        #[serde(deserialize_with = "deserialize_token_path")]
        radius: String,
    },
    Corners {
        radii: CornerTokenPaths,
    },
    // Internally tagged unit variants accept unknown members despite deny_unknown_fields.
    Capsule {},
}

impl From<ShapeFallbackProfileInput> for ShapeFallbackProfile {
    fn from(input: ShapeFallbackProfileInput) -> Self {
        match input {
            ShapeFallbackProfileInput::Uniform { radius } => Self::Uniform { radius },
            ShapeFallbackProfileInput::Corners { radii } => Self::Corners { radii },
            ShapeFallbackProfileInput::Capsule {} => Self::Capsule,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CornerTokenPaths {
    #[serde(deserialize_with = "deserialize_token_path")]
    pub top_start: String,
    #[serde(deserialize_with = "deserialize_token_path")]
    pub top_end: String,
    #[serde(deserialize_with = "deserialize_token_path")]
    pub bottom_end: String,
    #[serde(deserialize_with = "deserialize_token_path")]
    pub bottom_start: String,
}

impl ShapeFallbackAssignments {
    pub fn profile_for(&self, shape: ShapeIntent) -> &ShapeFallbackProfile {
        match shape {
            ShapeIntent::Structural => &self.profiles.structural,
            ShapeIntent::Soft => &self.profiles.soft,
            ShapeIntent::Rounded => &self.profiles.rounded,
            ShapeIntent::Capsule => &self.profiles.capsule,
            ShapeIntent::Organic => &self.profiles.organic,
        }
    }
}

impl TryFrom<ShapeFallbackAssignmentsInput> for ShapeFallbackAssignments {
    type Error = &'static str;

    fn try_from(input: ShapeFallbackAssignmentsInput) -> Result<Self, Self::Error> {
        if input.schema_version != "0.1.0" {
            return Err("schemaVersion must be 0.1.0");
        }
        if [
            &input.profiles.structural,
            &input.profiles.soft,
            &input.profiles.rounded,
        ]
        .into_iter()
        .any(|profile| matches!(profile, ShapeFallbackProfile::Capsule))
        {
            return Err("only capsule may use the capsule profile");
        }
        if !matches!(input.profiles.capsule, ShapeFallbackProfile::Capsule) {
            return Err("capsule must use the capsule profile");
        }
        if !matches!(input.profiles.organic, ShapeFallbackProfile::Corners { .. }) {
            return Err("organic must use explicit logical corners");
        }
        Ok(Self {
            schema_version: input.schema_version,
            profiles: input.profiles,
        })
    }
}

fn deserialize_token_path<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let path = String::deserialize(deserializer)?;
    if path.is_empty() {
        Err(D::Error::custom("shape token path must not be empty"))
    } else {
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn authored_profiles_are_complete_and_strict() {
        let source = include_str!("../../../../definitions/tier0-shapes.json");
        let assignments: ShapeFallbackAssignments = serde_json::from_str(source).unwrap();
        assert!(matches!(
            assignments.profile_for(ShapeIntent::Capsule),
            ShapeFallbackProfile::Capsule
        ));
        assert_eq!(
            serde_json::to_value(&assignments).unwrap(),
            serde_json::from_str::<Value>(source).unwrap()
        );

        let mut missing: Value = serde_json::from_str(source).unwrap();
        missing["profiles"]
            .as_object_mut()
            .unwrap()
            .remove("organic");
        assert!(serde_json::from_value::<ShapeFallbackAssignments>(missing).is_err());

        let mut unknown: Value = serde_json::from_str(source).unwrap();
        unknown["profiles"]["organic"]["radii"]["extra"] = json!("radius.1");
        assert!(serde_json::from_value::<ShapeFallbackAssignments>(unknown).is_err());

        let mut wrong_structural: Value = serde_json::from_str(source).unwrap();
        wrong_structural["profiles"]["structural"] = json!({"kind": "capsule"});
        assert!(
            serde_json::from_value::<ShapeFallbackAssignments>(wrong_structural)
                .unwrap_err()
                .to_string()
                .contains("only capsule may use the capsule profile")
        );
    }

    #[test]
    fn assignment_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/geometry/shape-fallback-assignment-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result =
                serde_json::from_value::<ShapeFallbackAssignments>(vector["document"].clone());
            if vector.get("expected").is_some() {
                let assignments = result.unwrap();
                assert_eq!(
                    serde_json::to_value(assignments).unwrap(),
                    vector["document"],
                    "{}",
                    vector["name"]
                );
            } else {
                let error = result.unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains(vector["error"].as_str().unwrap()),
                    "{}: {error}",
                    vector["name"]
                );
            }
        }
    }
}
