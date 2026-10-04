use resina_model::{SpringDynamics, SpringParameters, SpringState};
mod trajectory;
use serde::{Deserialize, Serialize};
use std::fmt;
pub use trajectory::{
    SpringTrajectorySample, resolve_spring_trajectory_source, sample_spring_trajectory,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SpringRepresentation {
    Spring,
    Immediate,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpringSample {
    schema_version: &'static str,
    representation: SpringRepresentation,
    position: f64,
    velocity: f64,
    settled: bool,
}
impl SpringSample {
    pub fn representation(&self) -> SpringRepresentation {
        self.representation
    }
    pub fn position(&self) -> f64 {
        self.position
    }
    pub fn velocity(&self) -> f64 {
        self.velocity
    }
    pub fn settled(&self) -> bool {
        self.settled
    }
}

#[derive(Debug)]
pub enum SpringError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    InvalidTime,
    InvalidTarget,
    NumericRange(&'static str),
}
impl fmt::Display for SpringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(formatter, "spring request parse failed: {e}"),
            Self::Request(e) => write!(formatter, "invalid spring request: {e}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidTime => formatter.write_str("time must be finite and nonnegative"),
            Self::InvalidTarget => formatter.write_str("target must be finite"),
            Self::NumericRange(field) => {
                write!(formatter, "spring {field} exceeds representable arithmetic")
            }
        }
    }
}
impl std::error::Error for SpringError {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    spring: SpringParameters,
    time: f64,
    reduced_motion: bool,
}

pub fn resolve_spring_source(source: &str) -> Result<SpringSample, SpringError> {
    let document = resina_tokens::parse_token_document(source).map_err(SpringError::Parse)?;
    let request: Request = serde_json::from_value(document).map_err(SpringError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(SpringError::UnsupportedVersion);
    }
    sample_spring(&request.spring, request.time, request.reduced_motion)
}

pub fn sample_spring(
    spring: &SpringParameters,
    time: f64,
    reduced_motion: bool,
) -> Result<SpringSample, SpringError> {
    sample_state(
        &spring.dynamics(),
        SpringState::try_new(0.0, spring.initial_velocity()).expect("validated initial velocity"),
        1.0,
        time,
        reduced_motion,
    )
}

fn sample_state(
    spring: &SpringDynamics,
    initial: SpringState,
    target: f64,
    time: f64,
    reduced_motion: bool,
) -> Result<SpringSample, SpringError> {
    if !target.is_finite() {
        return Err(SpringError::InvalidTarget);
    }
    if !time.is_finite() || time < 0.0 {
        return Err(SpringError::InvalidTime);
    }
    if reduced_motion {
        return Ok(sample(SpringRepresentation::Immediate, target, 0.0, true));
    }
    let mass_root = spring.mass().sqrt();
    let stiffness_root = spring.stiffness().sqrt();
    let omega = positive(stiffness_root / mass_root, "natural frequency")?;
    let scale = positive(mass_root * stiffness_root, "mass-stiffness scale")?;
    let ratio = spring.damping() / scale;
    let zeta = if ratio.is_finite() {
        ratio * 0.5
    } else {
        (spring.damping() * 0.5) / scale
    };
    finite(zeta, "damping ratio")?;
    if spring.damping() > 0.0 && zeta == 0.0 {
        return Err(SpringError::NumericRange("damping ratio"));
    }
    let q = finite(initial.velocity() / omega, "normalized velocity")?;
    if initial.velocity() != 0.0 && q == 0.0 {
        return Err(SpringError::NumericRange("normalized velocity"));
    }
    let u = finite(omega * time, "normalized time")?;
    if time > 0.0 && u == 0.0 {
        return Err(SpringError::NumericRange("normalized time"));
    }
    let initial_displacement = finite(initial.position() - target, "initial displacement")?;
    let (displacement, normalized_velocity) = propagate(zeta, u, initial_displacement, q, 1.0)?;
    // Separate scaling prevents normalized underflow from erasing the physical velocity bound.
    let (scaled_displacement, physical_velocity) =
        propagate(zeta, u, initial_displacement, initial.velocity(), omega)?;
    let radius = finite(displacement.hypot(normalized_velocity), "energy radius")?;
    let physical_radius = finite(
        scaled_displacement.hypot(physical_velocity),
        "velocity energy radius",
    )?;
    let settled =
        radius <= spring.position_threshold() && physical_radius <= spring.velocity_threshold();
    let position = if settled {
        target
    } else if time == 0.0 {
        initial.position()
    } else {
        finite(target + displacement, "position")?
    };
    let velocity = if settled { 0.0 } else { physical_velocity };
    Ok(sample(
        SpringRepresentation::Spring,
        position,
        velocity,
        settled,
    ))
}

fn propagate(
    zeta: f64,
    u: f64,
    position: f64,
    velocity: f64,
    position_scale: f64,
) -> Result<(f64, f64), SpringError> {
    if u == 0.0 {
        return Ok((
            finite(position * position_scale, "scaled initial displacement")?,
            velocity,
        ));
    }
    let (position, velocity) = if zeta <= 1.0 {
        let beta = ((1.0 - zeta) * (1.0 + zeta)).sqrt();
        let phase = finite(beta * u, "oscillation phase")?;
        let kernel = if phase == 0.0 { u } else { phase.sin() / beta };
        let decay = -zeta * u;
        let cosine = phase.cos();
        (
            product_exp(&[position, position_scale, cosine + zeta * kernel], decay)?
                + product_exp(&[velocity, kernel], decay)?,
            product_exp(&[-position, position_scale, kernel], decay)?
                + product_exp(&[velocity, cosine - zeta * kernel], decay)?,
        )
    } else {
        let beta = positive((zeta - 1.0).sqrt() * (zeta + 1.0).sqrt(), "overdamped root")?;
        let slow = -(1.0 / zeta) / (1.0 + beta / zeta);
        if slow == 0.0 {
            return Err(SpringError::NumericRange("slow pole"));
        }
        let decay = slow * u;
        let gap_time = -2.0 * (beta * u);
        let difference = -gap_time.exp_m1();
        let kernel = if gap_time == 0.0 {
            u
        } else {
            difference * (0.5 / beta)
        };
        (
            product_exp(&[position, position_scale], decay)?
                + product_exp(&[-position, position_scale, slow, kernel], decay)?
                + product_exp(&[velocity, kernel], decay)?,
            product_exp(&[-position, position_scale, kernel], decay)?
                + product_exp(&[velocity], decay + gap_time)?
                + product_exp(&[velocity, slow, kernel], decay)?,
        )
    };
    Ok((
        finite(position, "displacement")?,
        finite(velocity, "velocity")?,
    ))
}

fn sample(
    representation: SpringRepresentation,
    position: f64,
    velocity: f64,
    settled: bool,
) -> SpringSample {
    SpringSample {
        schema_version: "0.1.0",
        representation,
        position,
        velocity,
        settled,
    }
}
fn finite(value: f64, name: &'static str) -> Result<f64, SpringError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(SpringError::NumericRange(name))
    }
}
fn positive(value: f64, name: &'static str) -> Result<f64, SpringError> {
    if value > 0.0 {
        finite(value, name)
    } else {
        Err(SpringError::NumericRange(name))
    }
}
fn scaled_exp(coefficient: f64, exponent: f64) -> Result<f64, SpringError> {
    finite(coefficient, "solution coefficient")?;
    let value = if exponent >= f64::MIN_POSITIVE.ln() {
        coefficient * exponent.exp()
    } else if coefficient == 0.0 {
        0.0
    } else {
        coefficient.signum() * (coefficient.abs().ln() + exponent).exp()
    };
    finite(value, "solution")
}

fn product_exp(factors: &[f64], exponent: f64) -> Result<f64, SpringError> {
    for factor in factors {
        finite(*factor, "solution factor")?;
    }
    if factors.contains(&0.0) {
        return Ok(0.0);
    }
    let product = factors.iter().product::<f64>();
    if product.is_finite() && product != 0.0 {
        scaled_exp(product, exponent)
    } else {
        let sign = factors
            .iter()
            .map(|factor| factor.signum())
            .product::<f64>();
        let logarithm = factors.iter().map(|factor| factor.abs().ln()).sum::<f64>();
        finite(sign * (logarithm + exponent).exp(), "solution")
    }
}
