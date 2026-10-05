use crate::{SliderLayoutIr, SliderMinimumPosition, SliderOrientation};
use resina_environment::LayoutDirection;
use resina_model::PhysicalVector;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliderPointerAnchor {
    orientation: SliderOrientation,
    layout_direction: LayoutDirection,
    minimum_position: SliderMinimumPosition,
    extent: f64,
    pointer: f64,
    origin: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderAnchorError {
    InvalidPoint,
    IncompatibleLayout,
    NumericRange,
}

impl fmt::Display for SliderAnchorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPoint => "slider pointer point must be finite",
            Self::IncompatibleLayout => {
                "slider pointer anchor requires unchanged axis mapping and thumb extent"
            }
            Self::NumericRange => "slider pointer anchor exceeds representable arithmetic",
        })
    }
}
impl std::error::Error for SliderAnchorError {}

fn coordinate(
    orientation: SliderOrientation,
    point: PhysicalVector,
) -> Result<f64, SliderAnchorError> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return Err(SliderAnchorError::InvalidPoint);
    }
    Ok(match orientation {
        SliderOrientation::Horizontal => point.x,
        SliderOrientation::Vertical => point.y,
    })
}

impl SliderPointerAnchor {
    pub fn grab(layout: &SliderLayoutIr, point: PhysicalVector) -> Result<Self, SliderAnchorError> {
        let bounds = layout.thumb_bounds();
        let (origin, extent) = match layout.orientation() {
            SliderOrientation::Horizontal => (bounds.x, bounds.width),
            SliderOrientation::Vertical => (bounds.y, bounds.height),
        };
        Ok(Self {
            orientation: layout.orientation(),
            layout_direction: layout.layout_direction(),
            minimum_position: layout.minimum_position(),
            extent,
            pointer: coordinate(layout.orientation(), point)?,
            origin,
        })
    }

    pub fn center(layout: &SliderLayoutIr) -> Result<Self, SliderAnchorError> {
        let bounds = layout.thumb_bounds();
        let mut anchor = Self::grab(
            layout,
            PhysicalVector {
                x: bounds.x,
                y: bounds.y,
            },
        )?;
        let half = anchor.extent * 0.5;
        let center = anchor.origin + half;
        if half <= 0.0
            || half * 2.0 != anchor.extent
            || !center.is_finite()
            || center <= anchor.origin
        {
            return Err(SliderAnchorError::NumericRange);
        }
        anchor.pointer = center;
        Ok(anchor)
    }

    pub fn desired_origin(
        &self,
        layout: &SliderLayoutIr,
        point: PhysicalVector,
    ) -> Result<f64, SliderAnchorError> {
        let pointer = coordinate(self.orientation, point)?;
        let bounds = layout.thumb_bounds();
        let extent = match layout.orientation() {
            SliderOrientation::Horizontal => bounds.width,
            SliderOrientation::Vertical => bounds.height,
        };
        if layout.orientation() != self.orientation
            || layout.layout_direction() != self.layout_direction
            || layout.minimum_position() != self.minimum_position
            || extent != self.extent
        {
            return Err(SliderAnchorError::IncompatibleLayout);
        }
        if pointer == self.pointer {
            return Ok(self.origin);
        }
        let displacement = pointer - self.pointer;
        let desired = self.origin + displacement;
        if !displacement.is_finite()
            || !desired.is_finite()
            || (pointer > self.pointer && desired <= self.origin)
            || (pointer < self.pointer && desired >= self.origin)
        {
            return Err(SliderAnchorError::NumericRange);
        }
        Ok(desired)
    }
}
