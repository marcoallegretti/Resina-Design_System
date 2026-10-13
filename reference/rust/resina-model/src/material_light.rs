use crate::{KeyLight, PhysicalVector};
use serde::{Deserialize, Deserializer, Serialize, de::Error};

fn required_nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionLight {
    direction: PhysicalVector,
    strength: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InteractionInput {
    direction: PhysicalVector,
    strength: f64,
}

impl<'de> Deserialize<'de> for InteractionLight {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = crate::deserialize_assignment_object::<D, InteractionInput>(deserializer)?;
        KeyLight::try_new(input.direction).map_err(D::Error::custom)?;
        if !input.strength.is_finite() || !(0.0..1.0).contains(&input.strength) {
            return Err(D::Error::custom(
                "interaction strength must be finite and in [0,1)",
            ));
        }
        Ok(Self {
            direction: input.direction,
            strength: input.strength,
        })
    }
}

impl InteractionLight {
    pub fn direction(&self) -> PhysicalVector {
        self.direction
    }
    pub fn strength(&self) -> f64 {
        self.strength
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialLight {
    schema_version: String,
    key_light: KeyLight,
    ambient: Option<f64>,
    interaction: Option<InteractionLight>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LightInput {
    #[serde(deserialize_with = "crate::deserialize_version")]
    schema_version: String,
    key_light: KeyLight,
    #[serde(deserialize_with = "required_nullable")]
    ambient: Option<f64>,
    #[serde(deserialize_with = "required_nullable")]
    interaction: Option<InteractionLight>,
}

impl<'de> Deserialize<'de> for MaterialLight {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = crate::deserialize_assignment_object::<D, LightInput>(deserializer)?;
        if input
            .ambient
            .is_some_and(|a| !a.is_finite() || !(0.0..1.0).contains(&a))
        {
            return Err(D::Error::custom("ambient must be finite and in [0,1)"));
        }
        Ok(Self {
            schema_version: input.schema_version,
            key_light: input.key_light,
            ambient: input.ambient,
            interaction: input.interaction,
        })
    }
}

impl MaterialLight {
    pub fn key_light(&self) -> &KeyLight {
        &self.key_light
    }
    pub fn ambient(&self) -> Option<f64> {
        self.ambient
    }
    pub fn interaction(&self) -> Option<&InteractionLight> {
        self.interaction.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn valid() -> Value {
        json!({"schemaVersion":"0.1.0","keyLight":{"schemaVersion":"0.1.0","direction":{"x":1,"y":0}},"ambient":null,"interaction":null})
    }

    #[test]
    fn absence_is_explicit_and_objects_are_named() {
        let baseline = valid();
        assert!(serde_json::from_value::<MaterialLight>(baseline.clone()).is_ok());
        for name in ["schemaVersion", "keyLight", "ambient", "interaction"] {
            let mut invalid = baseline.clone();
            invalid.as_object_mut().unwrap().remove(name);
            assert!(
                serde_json::from_value::<MaterialLight>(invalid).is_err(),
                "{name}"
            );
        }
        assert!(
            serde_json::from_value::<MaterialLight>(json!([
                "0.1.0",
                baseline["keyLight"],
                null,
                null
            ]))
            .is_err()
        );
        let mut invalid = baseline;
        invalid["interaction"] = json!([{"x":1,"y":0},0.25]);
        assert!(serde_json::from_value::<MaterialLight>(invalid).is_err());
    }

    #[test]
    fn optional_channels_validate_before_policy() {
        for value in [-1.0, 1.0, 2.0] {
            let mut invalid = valid();
            invalid["ambient"] = json!(value);
            assert!(serde_json::from_value::<MaterialLight>(invalid).is_err());
        }
        for direction in [json!({"x":0,"y":0}), json!([1, 0])] {
            let mut invalid = valid();
            invalid["interaction"] = json!({"direction":direction,"strength":0});
            assert!(serde_json::from_value::<MaterialLight>(invalid).is_err());
        }
        let mut document = valid();
        document["interaction"] = json!({"direction":{"x":1,"y":0},"strength":0.25});
        assert_eq!(
            serde_json::from_value::<MaterialLight>(document)
                .unwrap()
                .interaction()
                .unwrap()
                .strength(),
            0.25
        );
    }
}
