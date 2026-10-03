use crate::PhysicalVector;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "KeyLightInput")]
pub struct KeyLight {
    schema_version: String,
    direction: PhysicalVector,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct KeyLightInput {
    #[serde(deserialize_with = "crate::deserialize_version")]
    schema_version: String,
    direction: PhysicalVector,
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
