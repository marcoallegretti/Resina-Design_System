use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "SpringInput")]
pub struct SpringParameters {
    schema_version: String,
    mass: f64,
    stiffness: f64,
    damping: f64,
    initial_velocity: f64,
    position_threshold: f64,
    velocity_threshold: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SpringInput {
    #[serde(deserialize_with = "crate::deserialize_version")]
    schema_version: String,
    mass: f64,
    stiffness: f64,
    damping: f64,
    initial_velocity: f64,
    position_threshold: f64,
    velocity_threshold: f64,
}

impl TryFrom<SpringInput> for SpringParameters {
    type Error = &'static str;
    fn try_from(input: SpringInput) -> Result<Self, Self::Error> {
        debug_assert_eq!(input.schema_version, "0.1.0");
        Self::try_new(
            input.mass,
            input.stiffness,
            input.damping,
            input.initial_velocity,
            input.position_threshold,
            input.velocity_threshold,
        )
    }
}

impl SpringParameters {
    pub fn try_new(
        mass: f64,
        stiffness: f64,
        damping: f64,
        initial_velocity: f64,
        position_threshold: f64,
        velocity_threshold: f64,
    ) -> Result<Self, &'static str> {
        for (message, value) in [
            ("mass must be finite and positive", mass),
            ("stiffness must be finite and positive", stiffness),
            (
                "positionThreshold must be finite and positive",
                position_threshold,
            ),
            (
                "velocityThreshold must be finite and positive",
                velocity_threshold,
            ),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(message);
            }
        }
        if !damping.is_finite() || damping < 0.0 {
            return Err("damping must be finite and nonnegative");
        }
        if !initial_velocity.is_finite() {
            return Err("initialVelocity must be finite");
        }
        Ok(Self {
            schema_version: "0.1.0".to_owned(),
            mass,
            stiffness,
            damping,
            initial_velocity,
            position_threshold,
            velocity_threshold,
        })
    }
    pub fn mass(&self) -> f64 {
        self.mass
    }
    pub fn stiffness(&self) -> f64 {
        self.stiffness
    }
    pub fn damping(&self) -> f64 {
        self.damping
    }
    pub fn initial_velocity(&self) -> f64 {
        self.initial_velocity
    }
    pub fn position_threshold(&self) -> f64 {
        self.position_threshold
    }
    pub fn velocity_threshold(&self) -> f64 {
        self.velocity_threshold
    }
}
