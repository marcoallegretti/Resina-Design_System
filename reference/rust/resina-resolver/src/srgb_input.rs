use resina_color::{ColorFallbackError, SrgbFallback, resolve_srgb_fallback};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SrgbInput {
    color_space: String,
    components: [f64; 3],
    alpha: f64,
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
