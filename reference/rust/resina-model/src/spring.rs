use serde::{
    Deserialize, Deserializer, Serialize,
    de::{Error, MapAccess, Visitor, value::MapAccessDeserializer},
};
use std::{fmt, marker::PhantomData};

fn deserialize_spring_object<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Object<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for Object<T> {
        type Value = T;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a spring object")
        }
        fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
            T::deserialize(MapAccessDeserializer::new(map))
        }
    }
    deserializer.deserialize_map(Object(PhantomData))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
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

impl<'de> Deserialize<'de> for SpringParameters {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize_spring_object::<D, SpringInput>(deserializer)?
            .try_into()
            .map_err(D::Error::custom)
    }
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
        SpringDynamics::try_new(
            mass,
            stiffness,
            damping,
            position_threshold,
            velocity_threshold,
        )?;
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
    pub fn dynamics(&self) -> SpringDynamics {
        SpringDynamics {
            schema_version: "0.1.0",
            mass: self.mass,
            stiffness: self.stiffness,
            damping: self.damping,
            position_threshold: self.position_threshold,
            velocity_threshold: self.velocity_threshold,
        }
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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpringDynamics {
    schema_version: &'static str,
    mass: f64,
    stiffness: f64,
    damping: f64,
    position_threshold: f64,
    velocity_threshold: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DynamicsInput {
    #[serde(deserialize_with = "crate::deserialize_version")]
    schema_version: String,
    mass: f64,
    stiffness: f64,
    damping: f64,
    position_threshold: f64,
    velocity_threshold: f64,
}
impl<'de> Deserialize<'de> for SpringDynamics {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize_spring_object::<D, DynamicsInput>(deserializer)?
            .try_into()
            .map_err(D::Error::custom)
    }
}
impl TryFrom<DynamicsInput> for SpringDynamics {
    type Error = &'static str;
    fn try_from(input: DynamicsInput) -> Result<Self, Self::Error> {
        debug_assert_eq!(input.schema_version, "0.1.0");
        Self::try_new(
            input.mass,
            input.stiffness,
            input.damping,
            input.position_threshold,
            input.velocity_threshold,
        )
    }
}
impl SpringDynamics {
    pub fn try_new(
        mass: f64,
        stiffness: f64,
        damping: f64,
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

        Ok(Self {
            schema_version: "0.1.0",
            mass,
            stiffness,
            damping,
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
    pub fn position_threshold(&self) -> f64 {
        self.position_threshold
    }
    pub fn velocity_threshold(&self) -> f64 {
        self.velocity_threshold
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpringState {
    position: f64,
    velocity: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StateInput {
    position: f64,
    velocity: f64,
}
impl<'de> Deserialize<'de> for SpringState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize_spring_object::<D, StateInput>(deserializer)?
            .try_into()
            .map_err(D::Error::custom)
    }
}
impl TryFrom<StateInput> for SpringState {
    type Error = &'static str;
    fn try_from(input: StateInput) -> Result<Self, Self::Error> {
        Self::try_new(input.position, input.velocity)
    }
}
impl SpringState {
    pub fn try_new(position: f64, velocity: f64) -> Result<Self, &'static str> {
        if !position.is_finite() {
            return Err("position must be finite");
        }
        if !velocity.is_finite() {
            return Err("velocity must be finite");
        }
        Ok(Self { position, velocity })
    }
    pub fn position(&self) -> f64 {
        self.position
    }
    pub fn velocity(&self) -> f64 {
        self.velocity
    }
}
