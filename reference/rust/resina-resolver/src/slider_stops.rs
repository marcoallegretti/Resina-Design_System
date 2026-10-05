use crate::{
    SliderAdjustment, SliderAdjustmentError, SliderAdjustmentInput, SliderAdjustmentIr,
    SliderValueError, SliderValueIr, resolve_slider_adjustment, resolve_slider_value,
    slider_adjustment::sum_roundoff,
};
use resina_model::SliderValue;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, fmt};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliderStops {
    schema_version: &'static str,
    values: Vec<SliderValueIr>,
}

#[derive(Debug)]
pub enum SliderStopsError {
    InvalidBounds(&'static str),
    TooFewStops,
    InvalidStop {
        index: usize,
        reason: &'static str,
    },
    UnorderedStops {
        index: usize,
    },
    EndpointMismatch,
    CurrentBounds,
    CurrentNotAllowed,
    InvalidCount,
    InvalidTarget,
    TargetNotAllowed,
    NumericRange,
    Value {
        index: usize,
        error: SliderValueError,
    },
    Adjustment(SliderAdjustmentError),
}
impl fmt::Display for SliderStopsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBounds(reason) => write!(f, "slider stops: {reason}"),
            Self::TooFewStops => f.write_str("slider stops require at least two values"),
            Self::InvalidStop { index, reason } => {
                write!(f, "slider stop {index}: {reason}")
            }
            Self::UnorderedStops { index } => {
                write!(
                    f,
                    "slider stop {index} must be strictly greater than its predecessor"
                )
            }
            Self::EndpointMismatch => {
                f.write_str("slider stops must include exact range endpoints")
            }
            Self::CurrentBounds => f.write_str("slider current bounds must match its stop domain"),
            Self::CurrentNotAllowed => f.write_str("slider current value must be an allowed stop"),
            Self::InvalidCount => f.write_str("slider stop count must be positive"),
            Self::InvalidTarget => {
                f.write_str("slider stop target must be finite and within bounds")
            }
            Self::TargetNotAllowed => f.write_str("slider absolute target must be an allowed stop"),
            Self::NumericRange => {
                f.write_str("slider stop distance exceeds representable arithmetic")
            }
            Self::Value { index, error } => write!(f, "slider stop {index}: {error}"),
            Self::Adjustment(error) => write!(f, "slider stops: {error}"),
        }
    }
}
impl std::error::Error for SliderStopsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Value { error, .. } => Some(error),
            Self::Adjustment(error) => Some(error),
            _ => None,
        }
    }
}

impl SliderStops {
    pub fn try_new(minimum: f64, maximum: f64, stops: &[f64]) -> Result<Self, SliderStopsError> {
        SliderValue::try_new(minimum, maximum, minimum).map_err(SliderStopsError::InvalidBounds)?;
        if stops.len() < 2 {
            return Err(SliderStopsError::TooFewStops);
        }
        let mut values = Vec::with_capacity(stops.len());
        for (index, &value) in stops.iter().enumerate() {
            let scalar = SliderValue::try_new(minimum, maximum, value)
                .map_err(|reason| SliderStopsError::InvalidStop { index, reason })?;
            if index > 0 && value <= stops[index - 1] {
                return Err(SliderStopsError::UnorderedStops { index });
            }
            values.push(
                resolve_slider_value(&scalar)
                    .map_err(|error| SliderStopsError::Value { index, error })?,
            );
        }
        if stops[0] != minimum || stops[stops.len() - 1] != maximum {
            return Err(SliderStopsError::EndpointMismatch);
        }
        Ok(Self {
            schema_version: "0.1.0",
            values,
        })
    }
    pub fn values(&self) -> &[SliderValueIr] {
        &self.values
    }
    fn search(&self, value: f64) -> Result<usize, usize> {
        self.values
            .binary_search_by(|stop| stop.value().value().partial_cmp(&value).unwrap())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SliderTieBreak {
    Lower,
    Higher,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SliderStopAdjustment {
    SetValue(f64),
    Nearest {
        value: f64,
        tie_break: SliderTieBreak,
    },
    Increase(u64),
    Decrease(u64),
    Minimum,
    Maximum,
}
pub struct SliderStopInput<'a> {
    pub stops: &'a SliderStops,
    pub current: &'a SliderValueIr,
    pub enabled: bool,
    pub read_only: bool,
    pub adjustment: SliderStopAdjustment,
}

fn nearest(
    stops: &SliderStops,
    value: f64,
    tie_break: SliderTieBreak,
) -> Result<usize, SliderStopsError> {
    let upper = match stops.search(value) {
        Ok(index) => return Ok(index),
        Err(index) => index,
    };
    let lower = upper - 1;
    let a = stops.values[lower].value().value();
    let b = stops.values[upper].value().value();
    let low = value - a;
    let high = b - value;
    let ordering = if low.is_finite() && high.is_finite() {
        low.partial_cmp(&high).unwrap().then_with(|| {
            sum_roundoff(value, -a, low)
                .partial_cmp(&sum_roundoff(b, -value, high))
                .unwrap()
        })
    } else if low.is_finite() {
        Ordering::Less
    } else if high.is_finite() {
        Ordering::Greater
    } else {
        return Err(SliderStopsError::NumericRange);
    };
    Ok(match ordering {
        Ordering::Less => lower,
        Ordering::Greater => upper,
        Ordering::Equal => match tie_break {
            SliderTieBreak::Lower => lower,
            SliderTieBreak::Higher => upper,
        },
    })
}

pub fn resolve_slider_stop_adjustment(
    input: SliderStopInput<'_>,
) -> Result<SliderAdjustmentIr, SliderStopsError> {
    let values = &input.stops.values;
    let bounds = values[0].value();
    let current = input.current.value();
    if current.minimum() != bounds.minimum() || current.maximum() != bounds.maximum() {
        return Err(SliderStopsError::CurrentBounds);
    }
    let index = input
        .stops
        .search(current.value())
        .map_err(|_| SliderStopsError::CurrentNotAllowed)?;
    let last = values.len() - 1;
    let target = match input.adjustment {
        SliderStopAdjustment::SetValue(value) | SliderStopAdjustment::Nearest { value, .. } => {
            if !value.is_finite() || value < bounds.minimum() || value > bounds.maximum() {
                return Err(SliderStopsError::InvalidTarget);
            }
            match input.adjustment {
                SliderStopAdjustment::Nearest { tie_break, .. } => {
                    nearest(input.stops, value, tie_break)?
                }
                _ => input
                    .stops
                    .search(value)
                    .map_err(|_| SliderStopsError::TargetNotAllowed)?,
            }
        }
        SliderStopAdjustment::Increase(count) | SliderStopAdjustment::Decrease(count) => {
            if count == 0 {
                return Err(SliderStopsError::InvalidCount);
            }
            let count = usize::try_from(count).unwrap_or(usize::MAX);
            match input.adjustment {
                SliderStopAdjustment::Increase(_) => index.saturating_add(count).min(last),
                _ => index.saturating_sub(count),
            }
        }
        SliderStopAdjustment::Minimum => 0,
        SliderStopAdjustment::Maximum => last,
    };
    resolve_slider_adjustment(SliderAdjustmentInput {
        current: input.current,
        enabled: input.enabled,
        read_only: input.read_only,
        adjustment: SliderAdjustment::SetValue(values[target].value().value()),
    })
    .map_err(SliderStopsError::Adjustment)
}
