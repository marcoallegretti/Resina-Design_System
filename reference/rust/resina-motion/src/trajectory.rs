use crate::{SpringError, SpringRepresentation, sample_state};
use resina_model::{SpringDynamics, SpringState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpringTrajectorySample {
    schema_version: &'static str,
    representation: SpringRepresentation,
    target: f64,
    state: SpringState,
    settled: bool,
}
impl SpringTrajectorySample {
    pub fn representation(&self) -> SpringRepresentation {
        self.representation
    }
    pub fn target(&self) -> f64 {
        self.target
    }
    pub fn state(&self) -> SpringState {
        self.state
    }
    pub fn settled(&self) -> bool {
        self.settled
    }
}

pub fn sample_spring_trajectory(
    dynamics: &SpringDynamics,
    initial: SpringState,
    target: f64,
    time: f64,
    reduced_motion: bool,
) -> Result<SpringTrajectorySample, SpringError> {
    let sample = sample_state(dynamics, initial, target, time, reduced_motion)?;
    Ok(SpringTrajectorySample {
        schema_version: "0.1.0",
        representation: sample.representation(),
        target,
        state: SpringState::try_new(sample.position(), sample.velocity())
            .expect("sampler publishes only finite state"),
        settled: sample.settled(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    dynamics: SpringDynamics,
    initial: SpringState,
    target: f64,
    time: f64,
    reduced_motion: bool,
}
pub fn resolve_spring_trajectory_source(
    source: &str,
) -> Result<SpringTrajectorySample, SpringError> {
    let document = resina_tokens::parse_token_document(source).map_err(SpringError::Parse)?;
    if !document.is_object() {
        return Err(SpringError::InvalidRequestShape);
    }
    let request: Request = serde_json::from_value(document).map_err(SpringError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(SpringError::UnsupportedVersion);
    }
    sample_spring_trajectory(
        &request.dynamics,
        request.initial,
        request.target,
        request.time,
        request.reduced_motion,
    )
}
