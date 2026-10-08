use crate::SliderLayoutIr;
use resina_model::{PhysicalBounds, SliderOrientation};
use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliderTrackSegmentsIr {
    schema_version: &'static str,
    clearance: f64,
    active: Option<PhysicalBounds>,
    inactive: Option<PhysicalBounds>,
}
impl SliderTrackSegmentsIr {
    /// The track from the minimum-value end up to the thumb's clearance.
    pub fn active(&self) -> Option<PhysicalBounds> {
        self.active
    }
    /// The track from the thumb's clearance to the maximum-value end.
    pub fn inactive(&self) -> Option<PhysicalBounds> {
        self.inactive
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderTrackSegmentsError {
    InvalidClearance,
}
impl fmt::Display for SliderTrackSegmentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidClearance => "slider thumb clearance must be finite and nonnegative",
        })
    }
}
impl std::error::Error for SliderTrackSegmentsError {}

// TwoSum keeps the exact addition error, so the rounding direction is known.
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let rounded = a + b;
    let virtual_b = rounded - a;
    let virtual_a = rounded - virtual_b;
    (rounded, (a - virtual_a) + (b - virtual_b))
}
fn sum_down(a: f64, b: f64) -> f64 {
    let (rounded, error) = two_sum(a, b);
    if error < 0.0 {
        rounded.next_down()
    } else {
        rounded
    }
}
fn sum_up(a: f64, b: f64) -> f64 {
    let (rounded, error) = two_sum(a, b);
    if error > 0.0 {
        rounded.next_up()
    } else {
        rounded
    }
}

pub fn resolve_slider_track_segments(
    layout: &SliderLayoutIr,
    clearance: f64,
) -> Result<SliderTrackSegmentsIr, SliderTrackSegmentsError> {
    if !clearance.is_finite() || clearance < 0.0 {
        return Err(SliderTrackSegmentsError::InvalidClearance);
    }
    let horizontal = layout.orientation() == SliderOrientation::Horizontal;
    let main = |b: PhysicalBounds| {
        if horizontal {
            (b.x, b.width)
        } else {
            (b.y, b.height)
        }
    };
    let track = layout.track_bounds();
    let (track_start, track_extent) = main(track);
    let track_end = sum_down(track_start, track_extent);
    let (thumb_start, thumb_extent) = main(layout.thumb_bounds());
    let thumb_end = sum_up(thumb_start, thumb_extent);
    // An overflowing high start rounds to infinity, past every track end.
    let low_end = sum_down(thumb_start, -clearance).min(track_end);
    let high_start = sum_up(thumb_end, clearance).max(track_start);
    let segment = |start: f64, end: f64| {
        (end > start).then(|| {
            let extent = sum_down(end, -start);
            if horizontal {
                PhysicalBounds {
                    x: start,
                    width: extent,
                    ..track
                }
            } else {
                PhysicalBounds {
                    y: start,
                    height: extent,
                    ..track
                }
            }
        })
    };
    let low = segment(track_start, low_end);
    let high = segment(high_start, track_end);
    let minimum_is_low =
        main(layout.minimum_thumb_bounds()).0 < main(layout.maximum_thumb_bounds()).0;
    let (active, inactive) = if minimum_is_low {
        (low, high)
    } else {
        (high, low)
    };
    Ok(SliderTrackSegmentsIr {
        schema_version: "0.1.0",
        clearance,
        active,
        inactive,
    })
}
