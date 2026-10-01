use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MaterialFamily {
    Cast,
    Frost,
    Elastomer,
    Gel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaterialRole {
    #[serde(rename = "surface.base")]
    SurfaceBase,
    #[serde(rename = "surface.content")]
    SurfaceContent,
    #[serde(rename = "surface.chrome")]
    SurfaceChrome,
    #[serde(rename = "surface.raised")]
    SurfaceRaised,
    #[serde(rename = "surface.transient")]
    SurfaceTransient,
    #[serde(rename = "control.passive")]
    ControlPassive,
    #[serde(rename = "control.interactive")]
    ControlInteractive,
    #[serde(rename = "control.primary")]
    ControlPrimary,
    #[serde(rename = "feedback.focus")]
    FeedbackFocus,
    #[serde(rename = "feedback.selection")]
    FeedbackSelection,
    #[serde(rename = "feedback.drag")]
    FeedbackDrag,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterialAssignments {
    #[serde(deserialize_with = "deserialize_version")]
    schema_version: String,
    surface: SurfaceAssignments,
    control: ControlAssignments,
    feedback: FeedbackAssignments,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SurfaceAssignments {
    #[serde(deserialize_with = "deserialize_structural_material")]
    base: MaterialFamily,
    #[serde(deserialize_with = "deserialize_structural_material")]
    content: MaterialFamily,
    #[serde(deserialize_with = "deserialize_structural_material")]
    chrome: MaterialFamily,
    #[serde(deserialize_with = "deserialize_structural_material")]
    raised: MaterialFamily,
    transient: MaterialFamily,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlAssignments {
    passive: MaterialFamily,
    interactive: MaterialFamily,
    primary: MaterialFamily,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FeedbackAssignments {
    focus: MaterialFamily,
    selection: MaterialFamily,
    drag: MaterialFamily,
}

impl MaterialAssignments {
    pub fn material_for(&self, role: MaterialRole) -> MaterialFamily {
        match role {
            MaterialRole::SurfaceBase => self.surface.base,
            MaterialRole::SurfaceContent => self.surface.content,
            MaterialRole::SurfaceChrome => self.surface.chrome,
            MaterialRole::SurfaceRaised => self.surface.raised,
            MaterialRole::SurfaceTransient => self.surface.transient,
            MaterialRole::ControlPassive => self.control.passive,
            MaterialRole::ControlInteractive => self.control.interactive,
            MaterialRole::ControlPrimary => self.control.primary,
            MaterialRole::FeedbackFocus => self.feedback.focus,
            MaterialRole::FeedbackSelection => self.feedback.selection,
            MaterialRole::FeedbackDrag => self.feedback.drag,
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

fn deserialize_structural_material<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<MaterialFamily, D::Error> {
    let material = MaterialFamily::deserialize(deserializer)?;
    if material == MaterialFamily::Gel {
        Err(D::Error::custom(
            "gel cannot be a default structural material",
        ))
    } else {
        Ok(material)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn material_assignment_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/materials/role-assignment-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = serde_json::from_value::<MaterialAssignments>(vector["document"].clone());
            if let Some(expected) = vector.get("expected") {
                let assignments = result.unwrap();
                for (role, family) in expected.as_object().unwrap() {
                    let role: MaterialRole = serde_json::from_value(json!(role)).unwrap();
                    assert_eq!(
                        serde_json::to_value(assignments.material_for(role)).unwrap(),
                        *family,
                        "{}: {role:?}",
                        vector["name"]
                    );
                }
                assert_eq!(
                    serde_json::to_value(&assignments).unwrap(),
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
