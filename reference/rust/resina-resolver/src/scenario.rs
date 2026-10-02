use crate::{
    BoundSurface, HeadlessResolutionError, SurfaceBindingError, bind_surface,
    headless::resolve_headless_document,
};
use resina_model::SurfaceIntent;
use resina_tokens::parse_token_document;
use serde::Deserialize;
use serde_json::Value;
use std::fmt;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SurfaceScenario {
    schema_version: String,
    resolution: Value,
    surface: SurfaceIntent,
}

#[derive(Debug)]
pub enum SurfaceScenarioError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    Resolution(HeadlessResolutionError),
    Binding(SurfaceBindingError),
}

impl fmt::Display for SurfaceScenarioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "surface scenario parse failed: {error}"),
            Self::Request(error) => write!(formatter, "invalid surface scenario: {error}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.2.0"),
            Self::Resolution(error) => write!(formatter, "surface scenario resolution: {error}"),
            Self::Binding(error) => write!(formatter, "surface scenario binding: {error}"),
        }
    }
}

impl std::error::Error for SurfaceScenarioError {}

pub fn resolve_surface_scenario_source(source: &str) -> Result<BoundSurface, SurfaceScenarioError> {
    let document = parse_token_document(source).map_err(SurfaceScenarioError::Parse)?;
    let scenario: SurfaceScenario =
        serde_json::from_value(document).map_err(SurfaceScenarioError::Request)?;
    if scenario.schema_version != "0.2.0" {
        return Err(SurfaceScenarioError::UnsupportedVersion);
    }
    let resolution =
        resolve_headless_document(scenario.resolution).map_err(SurfaceScenarioError::Resolution)?;
    bind_surface(&scenario.surface, &resolution).map_err(SurfaceScenarioError::Binding)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const RESOLUTION: &str = include_str!("../../../../conformance/headless/valid-request.json");
    const VECTORS: &str = include_str!("../../../../conformance/surfaces/binding-vectors.json");

    #[test]
    fn surface_scenario_resolves_binding_vectors() {
        let resolution: Value = serde_json::from_str(RESOLUTION).unwrap();
        let vectors: Vec<Value> = serde_json::from_str(VECTORS).unwrap();
        for vector in vectors {
            let scenario = json!({
                "schemaVersion": "0.2.0",
                "resolution": resolution,
                "surface": vector["document"]
            });
            let result = resolve_surface_scenario_source(&scenario.to_string());
            if let Some(expected) = vector.get("expected") {
                assert_eq!(
                    serde_json::to_value(result.unwrap()).unwrap(),
                    *expected,
                    "{}",
                    vector["name"]
                );
            } else {
                let error = result.unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains(vector["error"].as_str().unwrap()),
                    "{}: {error}",
                    vector["name"]
                );
            }
        }
    }

    #[test]
    fn scenario_rejects_duplicate_members_and_invalid_resolution() {
        let resolution: Value = serde_json::from_str(RESOLUTION).unwrap();
        let surface: Value = serde_json::from_str(VECTORS).unwrap();
        let valid = json!({
            "schemaVersion": "0.2.0",
            "resolution": resolution,
            "surface": surface[0]["document"]
        });
        let duplicate = valid.to_string().replacen(
            "\"schemaVersion\":\"0.2.0\"",
            "\"schemaVersion\":\"0.2.0\",\"schemaVersion\":\"0.2.0\"",
            1,
        );
        assert!(
            resolve_surface_scenario_source(&duplicate)
                .unwrap_err()
                .to_string()
                .contains("duplicate JSON member")
        );
        let mut missing = valid;
        missing["resolution"]["colorAssignments"]["roles"]["focus"] = json!("missing.color");
        assert!(
            resolve_surface_scenario_source(&missing.to_string())
                .unwrap_err()
                .to_string()
                .contains("color: MissingToken")
        );
    }
}
