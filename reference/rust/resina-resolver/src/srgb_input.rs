use resina_color::{ColorFallbackError, SrgbFallback, resolve_srgb_fallback};
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use std::fmt;

#[derive(Debug)]
pub(crate) struct SrgbInput {
    color_space: String,
    components: [f64; 3],
    alpha: f64,
}

impl<'de> Deserialize<'de> for SrgbInput {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Members {
            color_space: String,
            components: [f64; 3],
            alpha: f64,
        }

        struct Color;

        impl<'de> Visitor<'de> for Color {
            type Value = SrgbInput;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an sRGB color object")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let members = Members::deserialize(MapAccessDeserializer::new(map))?;
                Ok(SrgbInput {
                    color_space: members.color_space,
                    components: members.components,
                    alpha: members.alpha,
                })
            }
        }

        deserializer.deserialize_map(Color)
    }
}

#[derive(Debug)]
pub(crate) enum SrgbInputError {
    InvalidColorSpace,
    Color(ColorFallbackError),
}

impl SrgbInput {
    pub(crate) fn into_fallback(self) -> Result<SrgbFallback, SrgbInputError> {
        if self.color_space != "srgb" {
            return Err(SrgbInputError::InvalidColorSpace);
        }
        let value = serde_json::json!({
            "colorSpace": self.color_space,
            "components": self.components,
            "alpha": self.alpha,
        });
        resolve_srgb_fallback(&value).map_err(SrgbInputError::Color)
    }
}
