use crate::{
    SliderAdjustment, SliderAdjustmentError, SliderAdjustmentInput, SliderAdjustmentIr,
    SliderStopAdjustment, SliderStopInput, SliderStopsError, SliderValueIr, SliderValuePolicy,
    resolve_slider_adjustment, resolve_slider_stop_adjustment,
};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SliderKey {
    ArrowRight,
    ArrowLeft,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    PageUp,
    PageDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SliderKeySteps {
    Continuous { step: f64, page: Option<f64> },
    Stops { step: u64, page: Option<u64> },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliderKeyPolicy {
    schema_version: &'static str,
    steps: SliderKeySteps,
    increase_on_right: bool,
    increase_on_up: bool,
}
impl SliderKeyPolicy {
    pub fn try_new(
        steps: SliderKeySteps,
        increase_on_right: bool,
        increase_on_up: bool,
    ) -> Result<Self, SliderKeyError> {
        match steps {
            SliderKeySteps::Continuous { step, page } => {
                if !step.is_finite() || step <= 0.0 {
                    return Err(SliderKeyError::InvalidStep(
                        "slider key step must be finite and positive",
                    ));
                }
                if page.is_some_and(|page| !page.is_finite() || page <= step) {
                    return Err(SliderKeyError::InvalidPage);
                }
            }
            SliderKeySteps::Stops { step, page } => {
                if step != 1 {
                    return Err(SliderKeyError::InvalidStep(
                        "slider stopped key step must be exactly one index",
                    ));
                }
                if page.is_some_and(|page| page <= step) {
                    return Err(SliderKeyError::InvalidPage);
                }
            }
        }
        Ok(Self {
            schema_version: "0.1.0",
            steps,
            increase_on_right,
            increase_on_up,
        })
    }
}

pub struct SliderKeyInput<'a> {
    pub current: &'a SliderValueIr,
    pub value_policy: &'a SliderValuePolicy,
    pub key_policy: &'a SliderKeyPolicy,
    pub key: SliderKey,
    pub enabled: bool,
    pub read_only: bool,
    pub focused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SliderKeyOutcome {
    Adjusted,
    Unavailable,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliderKeyIr {
    schema_version: &'static str,
    value: SliderValueIr,
    commit: Option<SliderAdjustmentIr>,
    outcome: SliderKeyOutcome,
}
impl SliderKeyIr {
    pub fn value(&self) -> &SliderValueIr {
        &self.value
    }
    pub fn commit(&self) -> Option<&SliderAdjustmentIr> {
        self.commit.as_ref()
    }
    pub fn outcome(&self) -> SliderKeyOutcome {
        self.outcome
    }
}

#[derive(Debug)]
pub enum SliderKeyError {
    InvalidStep(&'static str),
    InvalidPage,
    PolicyMismatch,
    ValuePolicy(SliderStopsError),
    Adjustment(SliderAdjustmentError),
}
impl fmt::Display for SliderKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStep(reason) => f.write_str(reason),
            Self::InvalidPage => {
                f.write_str("slider key page must be finite and greater than step")
            }
            Self::PolicyMismatch => {
                f.write_str("slider key steps must match the value policy kind")
            }
            Self::ValuePolicy(error) => write!(f, "slider key policy: {error}"),
            Self::Adjustment(error) => write!(f, "slider key adjustment: {error}"),
        }
    }
}
impl std::error::Error for SliderKeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ValuePolicy(error) => Some(error),
            Self::Adjustment(error) => Some(error),
            _ => None,
        }
    }
}

pub fn resolve_slider_key(input: SliderKeyInput<'_>) -> Result<SliderKeyIr, SliderKeyError> {
    if !matches!(
        (input.value_policy, input.key_policy.steps),
        (
            SliderValuePolicy::Continuous,
            SliderKeySteps::Continuous { .. }
        ) | (
            SliderValuePolicy::Stops { .. },
            SliderKeySteps::Stops { .. }
        )
    ) {
        return Err(SliderKeyError::PolicyMismatch);
    }
    input
        .value_policy
        .validate_current(input.current)
        .map_err(SliderKeyError::ValuePolicy)?;
    let mut result = SliderKeyIr {
        schema_version: "0.1.0",
        value: *input.current,
        commit: None,
        outcome: SliderKeyOutcome::Unsupported,
    };
    let page_key = matches!(input.key, SliderKey::PageUp | SliderKey::PageDown);
    let increase = match input.key {
        SliderKey::ArrowRight => input.key_policy.increase_on_right,
        SliderKey::ArrowLeft => !input.key_policy.increase_on_right,
        SliderKey::ArrowUp => input.key_policy.increase_on_up,
        SliderKey::ArrowDown => !input.key_policy.increase_on_up,
        SliderKey::PageUp => true,
        SliderKey::PageDown => false,
        SliderKey::Home | SliderKey::End => false,
    };
    let enabled = input.enabled && input.focused;
    let adjustment = match (input.value_policy, input.key_policy.steps) {
        (SliderValuePolicy::Continuous, SliderKeySteps::Continuous { step, page }) => {
            let amount = if page_key {
                let Some(page) = page else { return Ok(result) };
                page
            } else {
                step
            };
            let adjustment = match input.key {
                SliderKey::Home => SliderAdjustment::Minimum,
                SliderKey::End => SliderAdjustment::Maximum,
                _ if increase => SliderAdjustment::Increase(amount),
                _ => SliderAdjustment::Decrease(amount),
            };
            resolve_slider_adjustment(SliderAdjustmentInput {
                current: input.current,
                enabled,
                read_only: input.read_only,
                adjustment,
            })
            .map_err(SliderKeyError::Adjustment)?
        }
        (SliderValuePolicy::Stops { stops, .. }, SliderKeySteps::Stops { step, page }) => {
            let count = if page_key {
                let Some(page) = page else { return Ok(result) };
                page
            } else {
                step
            };
            let adjustment = match input.key {
                SliderKey::Home => SliderStopAdjustment::Minimum,
                SliderKey::End => SliderStopAdjustment::Maximum,
                _ if increase => SliderStopAdjustment::Increase(count),
                _ => SliderStopAdjustment::Decrease(count),
            };
            resolve_slider_stop_adjustment(SliderStopInput {
                stops,
                current: input.current,
                enabled,
                read_only: input.read_only,
                adjustment,
            })
            .map_err(SliderKeyError::ValuePolicy)?
        }
        _ => unreachable!("policy kinds were checked before resolution"),
    };
    if adjustment.accepted() {
        result.value = *adjustment.value();
        result.commit = Some(adjustment);
        result.outcome = SliderKeyOutcome::Adjusted;
    } else {
        result.outcome = SliderKeyOutcome::Unavailable;
    }
    Ok(result)
}
