use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "FrostPigmentInput")]
pub struct FrostPigment {
    schema_version: String,
    tint_strength: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrostPigmentInput {
    schema_version: String,
    tint_strength: f64,
}

impl TryFrom<FrostPigmentInput> for FrostPigment {
    type Error = &'static str;

    fn try_from(input: FrostPigmentInput) -> Result<Self, Self::Error> {
        if input.schema_version != "0.1.0" {
            return Err("schemaVersion must be 0.1.0");
        }
        if !input.tint_strength.is_finite()
            || input.tint_strength <= 0.0
            || input.tint_strength >= 1.0
        {
            return Err("tintStrength must be finite and between 0 and 1");
        }
        Ok(Self {
            schema_version: input.schema_version,
            tint_strength: input.tint_strength,
        })
    }
}

impl FrostPigment {
    pub fn tint_strength(&self) -> f64 {
        self.tint_strength
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn frost_pigment_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/materials/frost-pigment-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = serde_json::from_value::<FrostPigment>(vector["document"].clone());
            if let Some(expected) = vector.get("expected") {
                let pigment = result.unwrap();
                assert_eq!(pigment.tint_strength(), expected.as_f64().unwrap());
                assert_eq!(serde_json::to_value(pigment).unwrap(), vector["document"]);
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
