use crate::PhysicalVector;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "KeyLightInput")]
pub struct KeyLight {
    schema_version: String,
    direction: PhysicalVector,
}

struct KeyLightInput {
    schema_version: String,
    direction: PhysicalVector,
}

impl<'de> Deserialize<'de> for KeyLightInput {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Members {
            #[serde(deserialize_with = "crate::deserialize_version")]
            schema_version: String,
            direction: PhysicalVector,
        }

        struct LightVisitor;
        impl<'de> Visitor<'de> for LightVisitor {
            type Value = KeyLightInput;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a key light object")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let members = Members::deserialize(MapAccessDeserializer::new(map))?;
                Ok(KeyLightInput {
                    schema_version: members.schema_version,
                    direction: members.direction,
                })
            }
        }
        deserializer.deserialize_map(LightVisitor)
    }
}

impl TryFrom<KeyLightInput> for KeyLight {
    type Error = &'static str;

    fn try_from(input: KeyLightInput) -> Result<Self, Self::Error> {
        debug_assert_eq!(input.schema_version, "0.1.0");
        Self::try_new(input.direction)
    }
}

impl KeyLight {
    pub fn try_new(direction: PhysicalVector) -> Result<Self, &'static str> {
        if !direction.x.is_finite() || !direction.y.is_finite() {
            return Err("key light direction must be finite");
        }
        if direction.x == 0.0 && direction.y == 0.0 {
            return Err("key light direction must be nonzero");
        }
        Ok(Self {
            schema_version: "0.1.0".to_owned(),
            direction,
        })
    }

    pub fn direction(&self) -> PhysicalVector {
        self.direction
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_light_rejects_invalid_directions() {
        assert!(KeyLight::try_new(PhysicalVector { x: 0.0, y: -0.0 }).is_err());
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(KeyLight::try_new(PhysicalVector { x: invalid, y: 1.0 }).is_err());
            assert!(KeyLight::try_new(PhysicalVector { x: 1.0, y: invalid }).is_err());
        }
        for magnitude in [f64::from_bits(1), 1.0, f64::MAX] {
            assert!(
                KeyLight::try_new(PhysicalVector {
                    x: magnitude,
                    y: -magnitude
                })
                .is_ok()
            );
        }
    }
}
