use crate::{SliderValueIr, hit_region::bounds_contain_bounds};
use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{PhysicalBounds, SliderOrientation, SurfaceSize};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SliderMinimumPosition {
    Start,
    End,
}
pub struct SliderLayoutInput<'a> {
    pub allocation_size: SurfaceSize,
    pub thumb_size: SurfaceSize,
    pub track_thickness: f64,
    pub insets: &'a SafeArea,
    pub layout_direction: LayoutDirection,
    pub orientation: SliderOrientation,
    pub minimum_position: SliderMinimumPosition,
    pub value: &'a SliderValueIr,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliderLayoutIr {
    schema_version: &'static str,
    layout_direction: LayoutDirection,
    orientation: SliderOrientation,
    minimum_position: SliderMinimumPosition,
    value: SliderValueIr,
    allocation_bounds: PhysicalBounds,
    track_bounds: PhysicalBounds,
    minimum_thumb_bounds: PhysicalBounds,
    maximum_thumb_bounds: PhysicalBounds,
    thumb_bounds: PhysicalBounds,
}
impl SliderLayoutIr {
    pub fn allocation_bounds(&self) -> PhysicalBounds {
        self.allocation_bounds
    }
    pub fn track_bounds(&self) -> PhysicalBounds {
        self.track_bounds
    }
    pub fn minimum_thumb_bounds(&self) -> PhysicalBounds {
        self.minimum_thumb_bounds
    }
    pub fn maximum_thumb_bounds(&self) -> PhysicalBounds {
        self.maximum_thumb_bounds
    }
    pub fn thumb_bounds(&self) -> PhysicalBounds {
        self.thumb_bounds
    }
    pub fn value(&self) -> &SliderValueIr {
        &self.value
    }
    pub fn orientation(&self) -> SliderOrientation {
        self.orientation
    }
    pub fn layout_direction(&self) -> LayoutDirection {
        self.layout_direction
    }
    pub fn minimum_position(&self) -> SliderMinimumPosition {
        self.minimum_position
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderLayoutError {
    InvalidSize,
    InvalidInsets,
    DoesNotFit,
    NoTravel,
    NumericRange,
}
impl fmt::Display for SliderLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidSize => {
                "slider allocation, thumb size and track thickness must be finite and positive"
            }
            Self::InvalidInsets => "slider insets must be finite and nonnegative",
            Self::DoesNotFit => "slider parts must fit within the inset allocation",
            Self::NoTravel => "slider requires distinct travel endpoints",
            Self::NumericRange => "slider layout exceeds representable arithmetic",
        })
    }
}
impl std::error::Error for SliderLayoutError {}

fn subtract(left: f64, right: f64) -> Result<f64, SliderLayoutError> {
    if right > left {
        return Err(SliderLayoutError::DoesNotFit);
    }
    let result = left - right;
    if right > 0.0 && result == left {
        return Err(SliderLayoutError::NumericRange);
    }
    Ok(result)
}
fn add(origin: f64, extent: f64) -> Result<f64, SliderLayoutError> {
    let result = origin + extent;
    if !result.is_finite() || (extent > 0.0 && result == origin) {
        return Err(SliderLayoutError::NumericRange);
    }
    Ok(result)
}
fn half(extent: f64) -> Result<f64, SliderLayoutError> {
    let result = extent * 0.5;
    if extent > 0.0 && result == 0.0 {
        return Err(SliderLayoutError::NumericRange);
    }
    Ok(result)
}
fn centered(origin: f64, space: f64, extent: f64) -> Result<f64, SliderLayoutError> {
    add(origin, half(subtract(space, extent)?)?)
}
fn interpolate(minimum: f64, maximum: f64, progress: f64) -> Result<f64, SliderLayoutError> {
    if progress == 0.0 {
        return Ok(minimum);
    }
    if progress == 1.0 {
        return Ok(maximum);
    }
    let span = maximum - minimum;
    let result = if progress <= 0.5 {
        minimum + span * progress
    } else {
        maximum - span * (1.0 - progress)
    };
    if !result.is_finite() || result <= minimum.min(maximum) || result >= minimum.max(maximum) {
        return Err(SliderLayoutError::NumericRange);
    }
    Ok(result)
}
pub fn resolve_slider_layout(
    input: SliderLayoutInput<'_>,
) -> Result<SliderLayoutIr, SliderLayoutError> {
    let allocation = input.allocation_size;
    let thumb = input.thumb_size;
    if [
        allocation.width,
        allocation.height,
        thumb.width,
        thumb.height,
        input.track_thickness,
    ]
    .iter()
    .any(|v| !v.is_finite() || *v <= 0.0)
    {
        return Err(SliderLayoutError::InvalidSize);
    }
    let p = input.insets;
    if [p.start, p.end, p.top, p.bottom]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.0)
    {
        return Err(SliderLayoutError::InvalidInsets);
    }
    let width = subtract(subtract(allocation.width, p.start)?, p.end)?;
    let height = subtract(subtract(allocation.height, p.top)?, p.bottom)?;
    let (left, right) = match input.layout_direction {
        LayoutDirection::Ltr => (p.start, p.end),
        LayoutDirection::Rtl => (p.end, p.start),
    };
    let (
        main_size,
        main_low_inset,
        main_high_inset,
        thumb_main_extent,
        cross_origin,
        cross_extent,
        thumb_cross_extent,
    ) = match input.orientation {
        SliderOrientation::Horizontal => (
            allocation.width,
            left,
            right,
            thumb.width,
            p.top,
            height,
            thumb.height,
        ),
        SliderOrientation::Vertical => (
            allocation.height,
            p.top,
            p.bottom,
            thumb.height,
            left,
            width,
            thumb.width,
        ),
    };
    let high_origin = subtract(subtract(main_size, main_high_inset)?, thumb_main_extent)?;
    let travel = subtract(high_origin, main_low_inset)?;
    if travel <= 0.0 {
        return Err(SliderLayoutError::NoTravel);
    }
    let thumb_cross = centered(cross_origin, cross_extent, thumb_cross_extent)?;
    let track_cross = centered(cross_origin, cross_extent, input.track_thickness)?;
    let half_thumb = half(thumb_main_extent)?;
    let low_center = add(main_low_inset, half_thumb)?;
    let high_center = add(high_origin, half_thumb)?;
    if low_center >= high_center {
        return Err(SliderLayoutError::NumericRange);
    }
    let axis_start_is_low = input.orientation == SliderOrientation::Vertical
        || input.layout_direction == LayoutDirection::Ltr;
    let (start, end) = if axis_start_is_low {
        (main_low_inset, high_origin)
    } else {
        (high_origin, main_low_inset)
    };
    let (minimum, maximum) = match input.minimum_position {
        SliderMinimumPosition::Start => (start, end),
        SliderMinimumPosition::End => (end, start),
    };
    let current = interpolate(minimum, maximum, input.value.progress())?;
    let part = |main, cross, main_extent, cross_extent| match input.orientation {
        SliderOrientation::Horizontal => PhysicalBounds {
            x: main,
            y: cross,
            width: main_extent,
            height: cross_extent,
        },
        SliderOrientation::Vertical => PhysicalBounds {
            x: cross,
            y: main,
            width: cross_extent,
            height: main_extent,
        },
    };
    let track_bounds = part(
        low_center,
        track_cross,
        high_center - low_center,
        input.track_thickness,
    );
    let minimum_thumb_bounds = part(minimum, thumb_cross, thumb_main_extent, thumb_cross_extent);
    let maximum_thumb_bounds = part(maximum, thumb_cross, thumb_main_extent, thumb_cross_extent);
    let thumb_bounds = part(current, thumb_cross, thumb_main_extent, thumb_cross_extent);
    let allocation_bounds = PhysicalBounds {
        x: 0.0,
        y: 0.0,
        width: allocation.width,
        height: allocation.height,
    };
    let interior = PhysicalBounds {
        x: left,
        y: p.top,
        width,
        height,
    };
    for bounds in [
        track_bounds,
        minimum_thumb_bounds,
        maximum_thumb_bounds,
        thumb_bounds,
    ] {
        if !bounds_contain_bounds(interior, bounds).map_err(|_| SliderLayoutError::NumericRange)?
            || !bounds_contain_bounds(allocation_bounds, bounds)
                .map_err(|_| SliderLayoutError::NumericRange)?
        {
            return Err(SliderLayoutError::DoesNotFit);
        }
    }
    Ok(SliderLayoutIr {
        schema_version: "0.1.0",
        layout_direction: input.layout_direction,
        orientation: input.orientation,
        minimum_position: input.minimum_position,
        value: *input.value,
        allocation_bounds,
        track_bounds,
        minimum_thumb_bounds,
        maximum_thumb_bounds,
        thumb_bounds,
    })
}
