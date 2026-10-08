use resina_model::{KeyLight, PhysicalVector};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct KeyLightRequest {
    schema_version: String,
    key_light: KeyLight,
    depth: f64,
    normals: Vec<PhysicalVector>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyLightResult {
    schema_version: &'static str,
    direction: PhysicalVector,
    side_offset: PhysicalVector,
    edge_highlight_weights: EdgeHighlightWeights,
    normal_highlight_weights: Vec<f64>,
}

impl KeyLightResult {
    pub fn direction(&self) -> PhysicalVector {
        self.direction
    }
    pub fn side_offset(&self) -> PhysicalVector {
        self.side_offset
    }
    pub fn edge_highlight_weights(&self) -> &EdgeHighlightWeights {
        &self.edge_highlight_weights
    }
    pub fn normal_highlight_weights(&self) -> &[f64] {
        &self.normal_highlight_weights
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EdgeHighlightWeights {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Debug)]
pub enum KeyLightError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    InvalidShape(&'static str),
    InvalidDepth,
    InvalidNormal(usize),
}

impl fmt::Display for KeyLightError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "key light parse failed: {error}"),
            Self::Request(error) => write!(formatter, "invalid key light request: {error}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidShape(field) => {
                write!(formatter, "key light {field} must be a JSON object")
            }
            Self::InvalidDepth => formatter.write_str("depth must be finite and nonnegative"),
            Self::InvalidNormal(index) => {
                write!(formatter, "normal {index} must be finite and nonzero")
            }
        }
    }
}

impl std::error::Error for KeyLightError {}

pub fn resolve_key_light_source(source: &str) -> Result<KeyLightResult, KeyLightError> {
    let document = parse_token_document(source).map_err(KeyLightError::Parse)?;
    if !document.is_object() {
        return Err(KeyLightError::InvalidShape("request"));
    }
    for (pointer, field) in [
        ("/keyLight", "keyLight"),
        ("/keyLight/direction", "direction"),
    ] {
        if document
            .pointer(pointer)
            .is_some_and(|value| !value.is_object())
        {
            return Err(KeyLightError::InvalidShape(field));
        }
    }
    if let Some(normals) = document["normals"].as_array()
        && normals.iter().any(|normal| !normal.is_object())
    {
        return Err(KeyLightError::InvalidShape("normals[]"));
    }
    let request: KeyLightRequest =
        serde_json::from_value(document).map_err(KeyLightError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(KeyLightError::UnsupportedVersion);
    }
    resolve_key_light(&request.key_light, request.depth, &request.normals)
}

pub fn resolve_key_light(
    key_light: &KeyLight,
    depth: f64,
    normals: &[PhysicalVector],
) -> Result<KeyLightResult, KeyLightError> {
    if !depth.is_finite() || depth < 0.0 {
        return Err(KeyLightError::InvalidDepth);
    }
    let direction = unit_direction(key_light.direction());
    let normal_highlight_weights = normals
        .iter()
        .enumerate()
        .map(|(index, normal)| {
            if !normal.x.is_finite()
                || !normal.y.is_finite()
                || (normal.x == 0.0 && normal.y == 0.0)
            {
                return Err(KeyLightError::InvalidNormal(index));
            }
            let normal = unit_direction(*normal);
            Ok((normal.x * direction.x + normal.y * direction.y).clamp(0.0, 1.0))
        })
        .collect::<Result<_, _>>()?;
    Ok(KeyLightResult {
        schema_version: "0.1.0",
        direction,
        side_offset: PhysicalVector {
            x: -direction.x * depth,
            y: -direction.y * depth,
        },
        edge_highlight_weights: EdgeHighlightWeights {
            top: (-direction.y).max(0.0),
            right: direction.x.max(0.0),
            bottom: direction.y.max(0.0),
            left: (-direction.x).max(0.0),
        },
        normal_highlight_weights,
    })
}

fn unit_direction(vector: PhysicalVector) -> PhysicalVector {
    let largest = vector.x.abs().max(vector.y.abs());
    let x = vector.x / largest;
    let y = vector.y / largest;
    let length = (x * x + y * y).sqrt();
    PhysicalVector {
        x: x / length,
        y: y / length,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn public_key_light_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/lighting/key-light-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = resolve_key_light_source(&vector["request"].to_string());
            if let Some(expected) = vector.get("expected") {
                let result = result.unwrap();
                let expected_direction: PhysicalVector =
                    serde_json::from_value(expected["direction"].clone()).unwrap();
                let expected_offset: PhysicalVector =
                    serde_json::from_value(expected["sideOffset"].clone()).unwrap();
                let actual = result.direction();
                assert!((actual.x - expected_direction.x).abs() <= 1e-12);
                assert!((actual.y - expected_direction.y).abs() <= 1e-12);
                let actual = result.side_offset();
                for (actual, expected) in
                    [(actual.x, expected_offset.x), (actual.y, expected_offset.y)]
                {
                    assert!(
                        (actual - expected).abs() <= (expected.abs() * 1e-12).max(1e-12),
                        "{}",
                        vector["name"]
                    );
                }
                let weights = result.edge_highlight_weights();
                for (name, actual) in [
                    ("top", weights.top),
                    ("right", weights.right),
                    ("bottom", weights.bottom),
                    ("left", weights.left),
                ] {
                    assert!(
                        (actual - expected["edgeHighlightWeights"][name].as_f64().unwrap()).abs()
                            <= 1e-12
                    );
                }
                let samples = expected["normalHighlightWeights"].as_array().unwrap();
                assert_eq!(samples.len(), result.normal_highlight_weights().len());
                for (actual, expected) in result.normal_highlight_weights().iter().zip(samples) {
                    assert!((actual - expected.as_f64().unwrap()).abs() <= 1e-12);
                }
            } else {
                let error = result.unwrap_err().to_string();
                assert!(
                    error.contains(vector["errorContains"].as_str().unwrap()),
                    "{}: {error}",
                    vector["name"]
                );
            }
        }
    }

    #[test]
    fn typed_calls_reject_invalid_depth_and_normals() {
        let light = KeyLight::try_new(PhysicalVector { x: -1.0, y: -1.0 }).unwrap();
        for depth in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                resolve_key_light(&light, depth, &[]),
                Err(KeyLightError::InvalidDepth)
            ));
        }
        for normal in [
            PhysicalVector { x: 0.0, y: 0.0 },
            PhysicalVector {
                x: f64::NAN,
                y: 1.0,
            },
            PhysicalVector {
                x: 1.0,
                y: f64::INFINITY,
            },
        ] {
            assert!(matches!(
                resolve_key_light(&light, 1.0, &[normal]),
                Err(KeyLightError::InvalidNormal(0))
            ));
        }
    }
}
