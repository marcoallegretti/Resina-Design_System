use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "SliderValueInput")]
pub struct SliderValue {
    #[serde(skip_deserializing)]
    schema_version: &'static str,
    minimum: f64,
    maximum: f64,
    value: f64,
}

struct SliderValueInput {
    schema_version: String,
    minimum: f64,
    maximum: f64,
    value: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SliderValueMembers {
    #[serde(deserialize_with = "crate::deserialize_version")]
    schema_version: String,
    #[serde(deserialize_with = "deserialize_number")]
    minimum: f64,
    #[serde(deserialize_with = "deserialize_number")]
    maximum: f64,
    #[serde(deserialize_with = "deserialize_number")]
    value: f64,
}

impl<'de> Deserialize<'de> for SliderValueInput {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ValueVisitor;
        impl<'de> Visitor<'de> for ValueVisitor {
            type Value = SliderValueInput;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a slider value object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let input = SliderValueMembers::deserialize(MapAccessDeserializer::new(map))?;
                Ok(SliderValueInput {
                    schema_version: input.schema_version,
                    minimum: input.minimum,
                    maximum: input.maximum,
                    value: input.value,
                })
            }
        }
        deserializer.deserialize_map(ValueVisitor)
    }
}

fn deserialize_number<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    struct NumberVisitor;
    impl serde::de::Visitor<'_> for NumberVisitor {
        type Value = f64;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a finite number representable as binary64")
        }
        fn visit_i64<E: serde::de::Error>(self, integer: i64) -> Result<f64, E> {
            let value = integer as f64;
            if value as i128 != i128::from(integer) {
                return Err(E::custom(
                    "slider integer exceeds exact binary64 representation",
                ));
            }
            Ok(value)
        }
        fn visit_u64<E: serde::de::Error>(self, integer: u64) -> Result<f64, E> {
            let value = integer as f64;
            if value as u128 != u128::from(integer) {
                return Err(E::custom(
                    "slider integer exceeds exact binary64 representation",
                ));
            }
            Ok(value)
        }
        fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<f64, E> {
            if !value.is_finite() {
                return Err(E::custom("slider number must be finite"));
            }
            Ok(value)
        }
    }
    deserializer.deserialize_any(NumberVisitor)
}

impl TryFrom<SliderValueInput> for SliderValue {
    type Error = &'static str;
    fn try_from(input: SliderValueInput) -> Result<Self, Self::Error> {
        debug_assert_eq!(input.schema_version, "0.1.0");
        Self::try_new(input.minimum, input.maximum, input.value)
    }
}
impl SliderValue {
    pub fn try_new(minimum: f64, maximum: f64, value: f64) -> Result<Self, &'static str> {
        if !minimum.is_finite() || !maximum.is_finite() || !value.is_finite() {
            return Err("slider minimum, maximum and value must be finite");
        }
        if minimum >= maximum {
            return Err("slider minimum must be less than maximum");
        }
        if value < minimum || value > maximum {
            return Err("slider value must be within minimum and maximum");
        }
        Ok(Self {
            schema_version: "0.1.0",
            minimum,
            maximum,
            value,
        })
    }
    pub fn minimum(&self) -> f64 {
        self.minimum
    }
    pub fn maximum(&self) -> f64 {
        self.maximum
    }
    pub fn value(&self) -> f64 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_boundaries_reject_nonfinite_collapsed_and_outside_values() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(SliderValue::try_new(bad, 1.0, 0.5).is_err());
            assert!(SliderValue::try_new(0.0, bad, 0.5).is_err());
            assert!(SliderValue::try_new(0.0, 1.0, bad).is_err());
        }
        for (min, max, value) in [
            (1.0, 1.0, 1.0),
            (2.0, 1.0, 1.5),
            (0.0, 1.0, -0.1),
            (0.0, 1.0, 1.1),
        ] {
            assert!(SliderValue::try_new(min, max, value).is_err());
        }
        for value in [0.0, 0.5, 1.0] {
            let original = SliderValue::try_new(0.0, 1.0, value).unwrap();
            assert_eq!(
                serde_json::from_value::<SliderValue>(serde_json::to_value(original).unwrap())
                    .unwrap(),
                original
            );
        }
    }
}
