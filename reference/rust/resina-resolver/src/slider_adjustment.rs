use crate::{SliderValueError, SliderValueIr, resolve_slider_value};
use resina_model::SliderValue;
use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SliderAdjustment {
    SetValue(f64),
    Increase(f64),
    Decrease(f64),
    Minimum,
    Maximum,
}
pub struct SliderAdjustmentInput<'a> {
    pub current: &'a SliderValueIr,
    pub enabled: bool,
    pub read_only: bool,
    pub adjustment: SliderAdjustment,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliderAdjustmentIr {
    schema_version: &'static str,
    value: SliderValueIr,
    accepted: bool,
    changed: bool,
}
impl SliderAdjustmentIr {
    pub fn value(&self) -> &SliderValueIr {
        &self.value
    }
    pub fn accepted(&self) -> bool {
        self.accepted
    }
    pub fn changed(&self) -> bool {
        self.changed
    }
}
#[derive(Debug)]
pub enum SliderAdjustmentError {
    InvalidValue(&'static str),
    InvalidAmount,
    NumericRange,
    Value(SliderValueError),
}
impl fmt::Display for SliderAdjustmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue(error) => write!(f, "slider adjustment: {error}"),
            Self::InvalidAmount => {
                f.write_str("slider adjustment amount must be finite and positive")
            }
            Self::NumericRange => f.write_str("slider adjustment exceeds representable arithmetic"),
            Self::Value(error) => write!(f, "slider adjustment: {error}"),
        }
    }
}
impl std::error::Error for SliderAdjustmentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Value(error) => Some(error),
            _ => None,
        }
    }
}
pub fn resolve_slider_adjustment(
    input: SliderAdjustmentInput<'_>,
) -> Result<SliderAdjustmentIr, SliderAdjustmentError> {
    let current = input.current.value();
    let minimum = current.minimum();
    let maximum = current.maximum();
    let value = current.value();
    match input.adjustment {
        SliderAdjustment::SetValue(target) => {
            SliderValue::try_new(minimum, maximum, target)
                .map_err(SliderAdjustmentError::InvalidValue)?;
        }
        SliderAdjustment::Increase(amount) | SliderAdjustment::Decrease(amount) => {
            if !amount.is_finite() || amount <= 0.0 {
                return Err(SliderAdjustmentError::InvalidAmount);
            }
        }
        SliderAdjustment::Minimum | SliderAdjustment::Maximum => {}
    }
    let accepted = input.enabled && !input.read_only;
    let next = if !accepted {
        *input.current
    } else {
        let target = match input.adjustment {
            SliderAdjustment::SetValue(target) => target,
            SliderAdjustment::Minimum => minimum,
            SliderAdjustment::Maximum => maximum,
            SliderAdjustment::Increase(amount) => {
                let target = value + amount;
                if target > maximum {
                    maximum
                } else if target == maximum {
                    if sum_roundoff(value, amount, target) < 0.0 {
                        return Err(SliderAdjustmentError::NumericRange);
                    }
                    maximum
                } else if target <= value {
                    return Err(SliderAdjustmentError::NumericRange);
                } else {
                    target
                }
            }
            SliderAdjustment::Decrease(amount) => {
                let target = value - amount;
                if target < minimum {
                    minimum
                } else if target == minimum {
                    if sum_roundoff(value, -amount, target) > 0.0 {
                        return Err(SliderAdjustmentError::NumericRange);
                    }
                    minimum
                } else if target >= value {
                    return Err(SliderAdjustmentError::NumericRange);
                } else {
                    target
                }
            }
        };
        let next = SliderValue::try_new(minimum, maximum, target)
            .map_err(SliderAdjustmentError::InvalidValue)?;
        resolve_slider_value(&next).map_err(SliderAdjustmentError::Value)?
    };
    Ok(SliderAdjustmentIr {
        schema_version: "0.1.0",
        value: next,
        accepted,
        changed: next.value().value() != value,
    })
}

pub(crate) fn sum_roundoff(a: f64, b: f64, sum: f64) -> f64 {
    // FastTwoSum requires finite sum and operands ordered by magnitude.
    let (larger, smaller) = if a.abs() >= b.abs() { (a, b) } else { (b, a) };
    smaller - (sum - larger)
}
