use resina_model::SpringParameters;
use serde::{Deserialize, Serialize};
use std::fmt;

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
    NumericRange(&'static str),
}
impl fmt::Display for SpringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(formatter, "spring request parse failed: {e}"),
            Self::Request(e) => write!(formatter, "invalid spring request: {e}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidTime => formatter.write_str("time must be finite and nonnegative"),
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
    if !time.is_finite() || time < 0.0 {
        return Err(SpringError::InvalidTime);
    }
    if reduced_motion {
        return Ok(sample(SpringRepresentation::Immediate, 1.0, 0.0, true));
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
    let q = finite(spring.initial_velocity() / omega, "normalized velocity")?;
    if spring.initial_velocity() != 0.0 && q == 0.0 {
        return Err(SpringError::NumericRange("normalized velocity"));
    }
    let u = finite(omega * time, "normalized time")?;
    if time > 0.0 && u == 0.0 {
        return Err(SpringError::NumericRange("normalized time"));
    }
    let (displacement, normalized_velocity) = propagate(zeta, u, -1.0, q)?;
    // Separate scaling prevents normalized underflow from erasing the physical velocity bound.
    let (scaled_displacement, physical_velocity) =
        propagate(zeta, u, -omega, spring.initial_velocity())?;
    let radius = finite(displacement.hypot(normalized_velocity), "energy radius")?;
    let physical_radius = finite(
        scaled_displacement.hypot(physical_velocity),
        "velocity energy radius",
    )?;
    let settled =
        radius <= spring.position_threshold() && physical_radius <= spring.velocity_threshold();
    let position = if settled {
        1.0
    } else if time == 0.0 {
        0.0
    } else {
        finite(1.0 + displacement, "position")?
    };
    let velocity = if settled { 0.0 } else { physical_velocity };
    Ok(sample(
        SpringRepresentation::Spring,
        position,
        velocity,
        settled,
    ))
}

fn propagate(zeta: f64, u: f64, position: f64, velocity: f64) -> Result<(f64, f64), SpringError> {
    if u == 0.0 {
        return Ok((position, velocity));
    }
    let (position, velocity) = if zeta <= 1.0 {
        let beta = ((1.0 - zeta) * (1.0 + zeta)).sqrt();
        let phase = finite(beta * u, "oscillation phase")?;
        let kernel = if phase == 0.0 { u } else { phase.sin() / beta };
        let decay = -zeta * u;
        let cosine = phase.cos();
        (
            product_exp(position, cosine + zeta * kernel, decay)?
                + product_exp(velocity, kernel, decay)?,
            product_exp(-position, kernel, decay)?
                + product_exp(velocity, cosine - zeta * kernel, decay)?,
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
            product_exp(position, 1.0, decay)?
                + product_exp(-position * slow, kernel, decay)?
                + product_exp(velocity, kernel, decay)?,
            product_exp(-position, kernel, decay)?
                + product_exp(velocity, 1.0, decay + gap_time)?
                + product_exp(velocity * slow, kernel, decay)?,
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

fn product_exp(left: f64, right: f64, exponent: f64) -> Result<f64, SpringError> {
    finite(left, "solution factor")?;
    finite(right, "solution factor")?;
    if left == 0.0 || right == 0.0 {
        return Ok(0.0);
    }
    let product = left * right;
    if product.is_finite() && product != 0.0 {
        scaled_exp(product, exponent)
    } else {
        finite(
            left.signum() * right.signum() * (left.abs().ln() + right.abs().ln() + exponent).exp(),
            "solution",
        )
    }
}
