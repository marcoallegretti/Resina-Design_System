use crate::{
    SrgbFallback,
    srgb_input::{SrgbInput, SrgbInputError},
};
use resina_color::{ColorFallbackError, composite_srgb_over_opaque, resolve_srgb_fallback};
use resina_model::{MaterialFamily, OpaquePigmentProfile, OpaquePigmentProfiles};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpaquePigmentRequest {
    #[serde(deserialize_with = "deserialize_version")]
    schema_version: String,
    material_family: MaterialFamily,
    body: SrgbInput,
    profiles: OpaquePigmentProfiles,
}

fn deserialize_version<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    let version = String::deserialize(deserializer)?;
    if version != "0.1.0" {
        return Err(serde::de::Error::custom("schemaVersion must be 0.1.0"));
    }
    Ok(version)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpaquePigmentResult {
    schema_version: &'static str,
    material_family: MaterialFamily,
    profile: OpaquePigmentProfile,
    body: SrgbFallback,
    side: SrgbFallback,
    highlight: SrgbFallback,
}

impl OpaquePigmentResult {
    pub fn material_family(&self) -> MaterialFamily {
        self.material_family
    }
    pub fn profile(&self) -> &OpaquePigmentProfile {
        &self.profile
    }
    pub fn body(&self) -> &SrgbFallback {
        &self.body
    }
    pub fn side(&self) -> &SrgbFallback {
        &self.side
    }
    pub fn highlight(&self) -> &SrgbFallback {
        &self.highlight
    }
}

#[derive(Debug)]
pub enum OpaquePigmentError {
    Parse(serde_json::Error),
    InvalidRequestShape,
    Request(serde_json::Error),
    InvalidColorSpace,
    Body(ColorFallbackError),
    TranslucentBody,
}

impl fmt::Display for OpaquePigmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "opaque pigment parse failed: {error}"),
            Self::InvalidRequestShape => {
                formatter.write_str("opaque pigment request must be a JSON object")
            }
            Self::Request(error) => write!(formatter, "invalid opaque pigment request: {error}"),
            Self::InvalidColorSpace => formatter.write_str("opaque pigment body must use sRGB"),
            Self::Body(error) => write!(formatter, "invalid opaque pigment body: {error}"),
            Self::TranslucentBody => formatter.write_str("opaque pigment body must be opaque"),
        }
    }
}

impl std::error::Error for OpaquePigmentError {}

pub fn resolve_opaque_pigment_source(
    source: &str,
) -> Result<OpaquePigmentResult, OpaquePigmentError> {
    let document = parse_token_document(source).map_err(OpaquePigmentError::Parse)?;
    if !document.is_object() {
        return Err(OpaquePigmentError::InvalidRequestShape);
    }
    let request: OpaquePigmentRequest =
        serde_json::from_value(document).map_err(OpaquePigmentError::Request)?;
    debug_assert_eq!(request.schema_version, "0.1.0");
    let body = request.body.into_fallback().map_err(|error| match error {
        SrgbInputError::InvalidColorSpace => OpaquePigmentError::InvalidColorSpace,
        SrgbInputError::Color(error) => OpaquePigmentError::Body(error),
    })?;
    resolve_opaque_pigment(request.material_family, &body, &request.profiles)
}

pub fn resolve_opaque_pigment(
    material_family: MaterialFamily,
    body: &SrgbFallback,
    profiles: &OpaquePigmentProfiles,
) -> Result<OpaquePigmentResult, OpaquePigmentError> {
    if body.alpha() != 1.0 {
        return Err(OpaquePigmentError::TranslucentBody);
    }
    let profile = *profiles.profile_for(material_family);
    let overlay = |components, strength| {
        let source = resolve_srgb_fallback(&json!({
            "colorSpace": "srgb", "components": components, "alpha": strength
        }))
        .expect("validated pigment strength and constant sRGB channels");
        composite_srgb_over_opaque(&source, body).expect("opaque pigment body")
    };
    Ok(OpaquePigmentResult {
        schema_version: "0.1.0",
        material_family,
        profile,
        body: body.clone(),
        side: overlay([0.0; 3], profile.side_shade()),
        highlight: overlay([1.0; 3], profile.highlight_lift()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn opaque_pigment_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/materials/opaque-pigment-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let source = serde_json::to_string(&vector["request"]).unwrap();
            let result = resolve_opaque_pigment_source(&source);
            if let Some(expected) = vector.get("expected") {
                let result = result.unwrap();
                assert_eq!(
                    result.material_family(),
                    serde_json::from_value(expected["materialFamily"].clone()).unwrap()
                );
                assert_eq!(
                    *result.profile(),
                    serde_json::from_value::<OpaquePigmentProfile>(expected["profile"].clone())
                        .unwrap()
                );
                for (name, color) in [
                    ("body", result.body()),
                    ("side", result.side()),
                    ("highlight", result.highlight()),
                ] {
                    assert_eq!(color.alpha(), 1.0);
                    for (got, want) in color
                        .components()
                        .into_iter()
                        .zip(expected[name]["components"].as_array().unwrap())
                    {
                        assert!(
                            (got - want.as_f64().unwrap()).abs() <= 1e-12,
                            "{}: {name}",
                            vector["name"]
                        );
                    }
                }
            } else {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains(vector["errorContains"].as_str().unwrap()),
                    "{}",
                    vector["name"]
                );
            }
        }
    }
}
