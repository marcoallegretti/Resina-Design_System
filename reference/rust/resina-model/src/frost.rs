use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
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

impl<'de> Deserialize<'de> for FrostPigment {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Pigment;

        impl<'de> Visitor<'de> for Pigment {
            type Value = FrostPigment;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a Frost pigment object")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                FrostPigmentInput::deserialize(MapAccessDeserializer::new(map))?
                    .try_into()
                    .map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_map(Pigment)
    }
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
            for result in [
                serde_json::from_value::<FrostPigment>(vector["document"].clone()),
                serde_json::from_str::<FrostPigment>(&vector["document"].to_string()),
            ] {
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

    #[test]
    fn duplicate_pigment_members_fail_before_publication() {
        for source in [
            r#"{"schemaVersion":"0.1.0","schemaVersion":"0.1.0","tintStrength":0.55}"#,
            r#"{"schemaVersion":"0.1.0","tintStrength":0.55,"tint\u0053trength":0.55}"#,
        ] {
            assert!(
                serde_json::from_str::<FrostPigment>(source)
                    .unwrap_err()
                    .to_string()
                    .contains("duplicate field")
            );
        }
    }
}
