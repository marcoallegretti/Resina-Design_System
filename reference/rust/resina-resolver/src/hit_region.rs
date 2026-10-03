use crate::resolve_minimum_hit_target;
use resina_environment::EnvironmentSnapshot;
use resina_model::{PhysicalBounds, PhysicalVector, SurfaceSize};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use std::fmt;

pub struct HitRegionInput<'a> {
    pub environment: &'a EnvironmentSnapshot,
    pub visual_bounds: PhysicalBounds,
    pub available_bounds: PhysicalBounds,
    pub component_minimum: SurfaceSize,
    pub occupied_regions: &'a [PhysicalBounds],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HitRegionIr {
    schema_version: &'static str,
    bounds: PhysicalBounds,
    minimum_size: SurfaceSize,
}

impl HitRegionIr {
    pub fn bounds(&self) -> PhysicalBounds {
        self.bounds
    }

    pub fn minimum_size(&self) -> SurfaceSize {
        self.minimum_size
    }

    pub fn contains(&self, point: PhysicalVector) -> Result<bool, HitRegionError> {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(HitRegionError::InvalidPoint);
        }
        let (right, bottom) = endpoints(self.bounds, "hit bounds")?;
        Ok(point.x >= self.bounds.x
            && point.y >= self.bounds.y
            && right.above(point.x)
            && bottom.above(point.y))
    }
}

#[derive(Debug)]
pub enum HitRegionError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    InvalidBounds(&'static str),
    NumericRange(&'static str),
    InvalidMinimum,
    InvalidPoint,
    Clipped,
    Occupied(usize),
}

impl fmt::Display for HitRegionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "hit region parse failed: {error}"),
            Self::Request(error) => write!(formatter, "invalid hit region request: {error}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidBounds(field) => write!(
                formatter,
                "{field} must have finite coordinates and positive finite dimensions"
            ),
            Self::NumericRange(field) => write!(formatter, "{field} has unrepresentable extents"),
            Self::InvalidMinimum => {
                formatter.write_str("componentMinimum dimensions must be finite and positive")
            }
            Self::InvalidPoint => formatter.write_str("hit point must be finite"),
            Self::Clipped => formatter.write_str("availableBounds clips the required hit region"),
            Self::Occupied(index) => {
                write!(formatter, "hit region overlaps occupiedRegions[{index}]")
            }
        }
    }
}

impl std::error::Error for HitRegionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) => Some(error),
            Self::Request(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
struct Endpoint {
    rounded: f64,
    residual: f64,
}

impl Endpoint {
    fn sum(origin: f64, extent: f64, field: &'static str) -> Result<Self, HitRegionError> {
        let rounded = origin + extent;
        let virtual_extent = rounded - origin;
        let virtual_origin = rounded - virtual_extent;
        // TwoSum retains the exact addition error; see Shewchuk's robust predicates.
        let residual = (origin - virtual_origin) + (extent - virtual_extent);
        if !rounded.is_finite()
            || !residual.is_finite()
            || rounded <= origin
            || (rounded == f64::MAX && residual > 0.0)
        {
            return Err(HitRegionError::NumericRange(field));
        }
        Ok(Self { rounded, residual })
    }

    fn above(self, point: f64) -> bool {
        self.rounded > point || (self.rounded == point && self.residual > 0.0)
    }

    fn exceeds(self, other: Self) -> bool {
        self.rounded > other.rounded
            || (self.rounded == other.rounded && self.residual > other.residual)
    }
}

fn endpoints(
    bounds: PhysicalBounds,
    field: &'static str,
) -> Result<(Endpoint, Endpoint), HitRegionError> {
    if ![bounds.x, bounds.y, bounds.width, bounds.height]
        .into_iter()
        .all(f64::is_finite)
        || bounds.width <= 0.0
        || bounds.height <= 0.0
    {
        return Err(HitRegionError::InvalidBounds(field));
    }
    Ok((
        Endpoint::sum(bounds.x, bounds.width, field)?,
        Endpoint::sum(bounds.y, bounds.height, field)?,
    ))
}

pub fn resolve_hit_region(input: HitRegionInput<'_>) -> Result<HitRegionIr, HitRegionError> {
    let visual = input.visual_bounds;
    let available = input.available_bounds;
    let (visual_right, visual_bottom) = endpoints(visual, "visualBounds")?;
    let (available_right, available_bottom) = endpoints(available, "availableBounds")?;
    for &bounds in input.occupied_regions {
        endpoints(bounds, "occupiedRegions")?;
    }
    let component = input.component_minimum;
    if !component.width.is_finite()
        || !component.height.is_finite()
        || component.width <= 0.0
        || component.height <= 0.0
    {
        return Err(HitRegionError::InvalidMinimum);
    }
    let floor = resolve_minimum_hit_target(input.environment);
    let minimum_size = SurfaceSize {
        width: component.width.max(f64::from(floor.minimum_width())),
        height: component.height.max(f64::from(floor.minimum_height())),
    };
    let width = visual.width.max(minimum_size.width);
    let height = visual.height.max(minimum_size.height);
    let bounds = PhysicalBounds {
        x: visual.x - (width - visual.width) / 2.0,
        y: visual.y - (height - visual.height) / 2.0,
        width,
        height,
    };
    let (right, bottom) = endpoints(bounds, "hit bounds")?;
    if bounds.x > visual.x
        || bounds.y > visual.y
        || visual_right.exceeds(right)
        || visual_bottom.exceeds(bottom)
    {
        return Err(HitRegionError::NumericRange("hit bounds"));
    }
    if bounds.x < available.x
        || bounds.y < available.y
        || right.exceeds(available_right)
        || bottom.exceeds(available_bottom)
    {
        return Err(HitRegionError::Clipped);
    }
    for (index, &occupied) in input.occupied_regions.iter().enumerate() {
        let (occupied_right, occupied_bottom) = endpoints(occupied, "occupiedRegions")?;
        if occupied_right.above(bounds.x)
            && right.above(occupied.x)
            && occupied_bottom.above(bounds.y)
            && bottom.above(occupied.y)
        {
            return Err(HitRegionError::Occupied(index));
        }
    }
    Ok(HitRegionIr {
        schema_version: "0.1.0",
        bounds,
        minimum_size,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundsInput {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl From<BoundsInput> for PhysicalBounds {
    fn from(value: BoundsInput) -> Self {
        Self {
            x: value.x,
            y: value.y,
            width: value.width,
            height: value.height,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    environment: EnvironmentSnapshot,
    visual_bounds: BoundsInput,
    available_bounds: BoundsInput,
    component_minimum: SurfaceSize,
    occupied_regions: Vec<BoundsInput>,
}

pub fn resolve_hit_region_source(source: &str) -> Result<HitRegionIr, HitRegionError> {
    let value = parse_token_document(source).map_err(HitRegionError::Parse)?;
    let request: Request = serde_json::from_value(value).map_err(HitRegionError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(HitRegionError::UnsupportedVersion);
    }
    let occupied: Vec<_> = request
        .occupied_regions
        .into_iter()
        .map(PhysicalBounds::from)
        .collect();
    resolve_hit_region(HitRegionInput {
        environment: &request.environment,
        visual_bounds: request.visual_bounds.into(),
        available_bounds: request.available_bounds.into(),
        component_minimum: request.component_minimum,
        occupied_regions: &occupied,
    })
}
