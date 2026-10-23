use crate::{
    CommandMotionPolicy, SurfaceOutline, SurfaceOutlineError,
    surface_outline::{OUTLINE_OBJECTS, OutlineRequest},
};
use resina_model::{CornerRadius, MaterialFamily, PhysicalBounds, PhysicalVector};
use resina_motion::SpringError;
use resina_tokens::parse_token_document;
use serde::{Deserialize, de::DeserializeOwned};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceMotionError(String);

impl SurfaceMotionError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}
impl fmt::Display for SurfaceMotionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "surface motion: {}", self.0)
    }
}
impl std::error::Error for SurfaceMotionError {}
impl From<SurfaceOutlineError> for SurfaceMotionError {
    fn from(error: SurfaceOutlineError) -> Self {
        Self(error.to_string())
    }
}
impl From<SpringError> for SurfaceMotionError {
    fn from(error: SpringError) -> Self {
        Self(error.to_string())
    }
}

pub(crate) fn parse_request<T: DeserializeOwned>(
    source: &str,
    objects: &[&str],
    outlines: &[&str],
) -> Result<T, SurfaceMotionError> {
    let document =
        parse_token_document(source).map_err(|e| SurfaceMotionError::new(e.to_string()))?;
    let outline_objects = outlines.iter().flat_map(|prefix| {
        OUTLINE_OBJECTS
            .iter()
            .map(move |pointer| format!("{prefix}{pointer}"))
    });
    for pointer in objects
        .iter()
        .map(|pointer| pointer.to_string())
        .chain(outline_objects)
    {
        if !document
            .pointer(&pointer)
            .is_some_and(serde_json::Value::is_object)
        {
            let name = if pointer.is_empty() {
                "request"
            } else {
                &pointer
            };
            return Err(SurfaceMotionError::new(format!("{name} must be an object")));
        }
    }
    serde_json::from_str(source).map_err(|e| SurfaceMotionError::new(e.to_string()))
}

pub(crate) fn require_outline_version(outline: &OutlineRequest) -> Result<(), SurfaceMotionError> {
    if outline.schema_version != "0.1.0" {
        return Err(SurfaceMotionError::new(
            "outline schemaVersion must be 0.1.0",
        ));
    }
    Ok(())
}

pub(crate) fn motion_policy(
    family: MaterialFamily,
    reduced_motion: bool,
) -> Result<CommandMotionPolicy, SurfaceMotionError> {
    match (family, reduced_motion) {
        (MaterialFamily::Gel, _) => Err(SurfaceMotionError::new(
            "Gel has no calibrated dynamics and is not supported",
        )),
        (_, true) => Ok(CommandMotionPolicy::ReducedMotion),
        (MaterialFamily::Cast, false) => Ok(CommandMotionPolicy::CastImmediate),
        _ => Ok(CommandMotionPolicy::Spring),
    }
}

pub(crate) fn require_time(time: f64) -> Result<(), SurfaceMotionError> {
    if time.is_finite() && time >= 0.0 {
        Ok(())
    } else {
        Err(SurfaceMotionError::new(
            "time must be finite and nonnegative",
        ))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContentBox {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl ContentBox {
    pub(crate) fn validated(&self) -> Result<PhysicalBounds, SurfaceMotionError> {
        let valid = [self.x, self.y].iter().all(|v| v.is_finite())
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0;
        if !valid {
            return Err(SurfaceMotionError::new(
                "content must be finite and enclose positive area",
            ));
        }
        Ok(PhysicalBounds {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        })
    }
}

pub(crate) fn contained_in_face(
    content: PhysicalBounds,
    origin: PhysicalVector,
    outline: &SurfaceOutline,
) -> bool {
    let face = outline.content_bounds();
    let left = origin.x + face.x;
    let top = origin.y + face.y;
    content.x >= left
        && content.y >= top
        && content.x + content.width <= left + face.width
        && content.y + content.height <= top + face.height
}

pub(crate) fn cover(
    rectangles: &[PhysicalBounds],
    sweep: PhysicalVector,
) -> Result<PhysicalBounds, SurfaceMotionError> {
    let mut left = f64::INFINITY;
    let mut top = f64::INFINITY;
    let mut right = f64::NEG_INFINITY;
    let mut bottom = f64::NEG_INFINITY;
    for rectangle in rectangles {
        for (dx, dy) in [(0.0, 0.0), (sweep.x, sweep.y)] {
            left = left.min(rectangle.x + dx);
            top = top.min(rectangle.y + dy);
            right = right.max(rectangle.x + rectangle.width + dx);
            bottom = bottom.max(rectangle.y + rectangle.height + dy);
        }
    }
    let reservation = PhysicalBounds {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    };
    let representable = [
        reservation.x,
        reservation.y,
        reservation.width,
        reservation.height,
    ]
    .iter()
    .all(|v| v.is_finite())
        && reservation.width > 0.0
        && reservation.height > 0.0;
    if !representable {
        return Err(SurfaceMotionError::new("reservation is not representable"));
    }
    Ok(reservation)
}

pub(crate) fn interpolate(first: f64, second: f64, weight: f64) -> f64 {
    first * (1.0 - weight) + second * weight
}

pub(crate) fn interpolate_corner(
    first: CornerRadius,
    second: CornerRadius,
    weight: f64,
) -> CornerRadius {
    CornerRadius {
        x: interpolate(first.x, second.x, weight),
        y: interpolate(first.y, second.y, weight),
    }
}
