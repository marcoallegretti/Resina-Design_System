use crate::{
    SliderAdjustment, SliderAdjustmentError, SliderAdjustmentInput, SliderAdjustmentIr,
    SliderLayoutIr, SliderOrientation, resolve_slider_adjustment, resolve_slider_value,
    slider_layout::interpolate,
};
use resina_model::SliderValue;
use std::fmt;

pub struct SliderPositionInput<'a> {
    pub layout: &'a SliderLayoutIr,
    pub desired_origin: f64,
    pub enabled: bool,
    pub read_only: bool,
}

#[derive(Debug)]
pub enum SliderPositionError {
    InvalidPosition,
    NumericRange,
    Adjustment(SliderAdjustmentError),
}
impl fmt::Display for SliderPositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPosition => f.write_str("slider desired origin must be finite"),
            Self::NumericRange => f.write_str("slider position exceeds representable arithmetic"),
            Self::Adjustment(error) => write!(f, "slider position: {error}"),
        }
    }
}
impl std::error::Error for SliderPositionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Adjustment(error) => Some(error),
            _ => None,
        }
    }
}

pub fn resolve_slider_position(
    input: SliderPositionInput<'_>,
) -> Result<SliderAdjustmentIr, SliderPositionError> {
    let desired = input.desired_origin;
    if !desired.is_finite() {
        return Err(SliderPositionError::InvalidPosition);
    }
    let layout = input.layout;
    let coordinate = |bounds: resina_model::PhysicalBounds| match layout.orientation() {
        SliderOrientation::Horizontal => bounds.x,
        SliderOrientation::Vertical => bounds.y,
    };
    let minimum_origin = coordinate(layout.minimum_thumb_bounds());
    let maximum_origin = coordinate(layout.maximum_thumb_bounds());
    let current_origin = coordinate(layout.thumb_bounds());
    let current = layout.value().value();
    let increasing_axis = minimum_origin < maximum_origin;
    let adjustment = if desired == current_origin {
        SliderAdjustment::SetValue(current.value())
    } else if (increasing_axis && desired <= minimum_origin)
        || (!increasing_axis && desired >= minimum_origin)
    {
        SliderAdjustment::Minimum
    } else if (increasing_axis && desired >= maximum_origin)
        || (!increasing_axis && desired <= maximum_origin)
    {
        SliderAdjustment::Maximum
    } else {
        let from_minimum = (desired - minimum_origin).abs();
        let from_maximum = (maximum_origin - desired).abs();
        let travel = (maximum_origin - minimum_origin).abs();
        let progress = if from_minimum <= from_maximum {
            from_minimum / travel
        } else {
            1.0 - from_maximum / travel
        };
        if !progress.is_finite() || progress <= 0.0 || progress >= 1.0 {
            return Err(SliderPositionError::NumericRange);
        }
        let minimum = current.minimum();
        let maximum = current.maximum();
        let span = maximum - minimum;
        let target = if !span.is_finite() {
            minimum * (1.0 - progress) + maximum * progress
        } else if progress <= 0.5 {
            minimum + span * progress
        } else {
            maximum - span * (1.0 - progress)
        };
        if !target.is_finite() || target <= minimum || target >= maximum {
            return Err(SliderPositionError::NumericRange);
        }
        let increasing_value = (desired > current_origin) == increasing_axis;
        if (increasing_value && target <= current.value())
            || (!increasing_value && target >= current.value())
        {
            return Err(SliderPositionError::NumericRange);
        }
        let value = SliderValue::try_new(minimum, maximum, target)
            .map_err(|_| SliderPositionError::NumericRange)?;
        let value = resolve_slider_value(&value).map_err(|_| SliderPositionError::NumericRange)?;
        let projected = interpolate(minimum_origin, maximum_origin, value.progress())
            .map_err(|_| SliderPositionError::NumericRange)?;
        if (desired > current_origin && projected <= current_origin)
            || (desired < current_origin && projected >= current_origin)
        {
            return Err(SliderPositionError::NumericRange);
        }
        SliderAdjustment::SetValue(target)
    };
    resolve_slider_adjustment(SliderAdjustmentInput {
        current: layout.value(),
        enabled: input.enabled,
        read_only: input.read_only,
        adjustment,
    })
    .map_err(SliderPositionError::Adjustment)
}
