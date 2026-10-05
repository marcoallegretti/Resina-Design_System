use crate::{
    SliderAdjustment, SliderAdjustmentError, SliderAdjustmentInput, SliderAdjustmentIr,
    SliderLayoutIr, SliderPositionError, SliderPositionInput, SliderValueIr,
    resolve_slider_adjustment, resolve_slider_position,
};
use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliderEditSession {
    schema_version: &'static str,
    revision: String,
    baseline: SliderValueIr,
    preview: SliderValueIr,
}
impl SliderEditSession {
    pub fn begin(value: &SliderValueIr, revision: &str) -> Result<Self, SliderEditError> {
        if revision.is_empty() {
            return Err(SliderEditError::InvalidRevision);
        }
        Ok(Self {
            schema_version: "0.1.0",
            revision: revision.to_owned(),
            baseline: *value,
            preview: *value,
        })
    }
    pub fn revision(&self) -> &str {
        &self.revision
    }
    pub fn baseline(&self) -> &SliderValueIr {
        &self.baseline
    }
    pub fn preview(&self) -> &SliderValueIr {
        &self.preview
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SliderEditAction<'a> {
    Preview {
        layout: &'a SliderLayoutIr,
        desired_origin: f64,
    },
    Commit,
    Cancel,
}
pub struct SliderEditInput<'a> {
    pub session: &'a SliderEditSession,
    pub current: &'a SliderValueIr,
    pub revision: &'a str,
    pub enabled: bool,
    pub read_only: bool,
    pub action: SliderEditAction<'a>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SliderEditOutcome {
    Previewed,
    Committed,
    Cancelled,
    Unavailable,
    Conflict,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliderEditResult {
    schema_version: &'static str,
    session: Option<SliderEditSession>,
    preview: SliderValueIr,
    commit: Option<SliderAdjustmentIr>,
    outcome: SliderEditOutcome,
}
impl SliderEditResult {
    pub fn session(&self) -> Option<&SliderEditSession> {
        self.session.as_ref()
    }
    pub fn preview(&self) -> &SliderValueIr {
        &self.preview
    }
    pub fn commit(&self) -> Option<&SliderAdjustmentIr> {
        self.commit.as_ref()
    }
    pub fn outcome(&self) -> SliderEditOutcome {
        self.outcome
    }
}
#[derive(Debug)]
pub enum SliderEditError {
    InvalidRevision,
    InvalidPosition,
    IncoherentLayout,
    Position(SliderPositionError),
    Adjustment(SliderAdjustmentError),
}
impl fmt::Display for SliderEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRevision => f.write_str("slider edit revision must not be empty"),
            Self::InvalidPosition => f.write_str("slider edit desired origin must be finite"),
            Self::IncoherentLayout => {
                f.write_str("slider edit layout must contain its current preview")
            }
            Self::Position(error) => write!(f, "slider edit: {error}"),
            Self::Adjustment(error) => write!(f, "slider edit: {error}"),
        }
    }
}
impl std::error::Error for SliderEditError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Position(error) => Some(error),
            Self::Adjustment(error) => Some(error),
            _ => None,
        }
    }
}

pub fn resolve_slider_edit(
    input: SliderEditInput<'_>,
) -> Result<SliderEditResult, SliderEditError> {
    if input.revision.is_empty() {
        return Err(SliderEditError::InvalidRevision);
    }
    if let SliderEditAction::Preview { desired_origin, .. } = input.action
        && !desired_origin.is_finite()
    {
        return Err(SliderEditError::InvalidPosition);
    }
    let mut result = SliderEditResult {
        schema_version: "0.1.0",
        session: None,
        preview: *input.current,
        commit: None,
        outcome: SliderEditOutcome::Cancelled,
    };
    if matches!(input.action, SliderEditAction::Cancel) {
        return Ok(result);
    }
    if input.revision != input.session.revision() || input.current != input.session.baseline() {
        result.outcome = SliderEditOutcome::Conflict;
        return Ok(result);
    }
    let adjustment = if let SliderEditAction::Preview {
        layout,
        desired_origin,
    } = input.action
    {
        if layout.value() != input.session.preview() {
            return Err(SliderEditError::IncoherentLayout);
        }
        resolve_slider_position(SliderPositionInput {
            layout,
            desired_origin,
            enabled: input.enabled,
            read_only: input.read_only,
        })
        .map_err(SliderEditError::Position)?
    } else {
        resolve_slider_adjustment(SliderAdjustmentInput {
            current: input.current,
            enabled: input.enabled,
            read_only: input.read_only,
            adjustment: SliderAdjustment::SetValue(input.session.preview().value().value()),
        })
        .map_err(SliderEditError::Adjustment)?
    };
    if !adjustment.accepted() {
        result.outcome = SliderEditOutcome::Unavailable;
        return Ok(result);
    }
    result.preview = *adjustment.value();
    if matches!(input.action, SliderEditAction::Preview { .. }) {
        let mut session = input.session.clone();
        session.preview = result.preview;
        result.session = Some(session);
        result.outcome = SliderEditOutcome::Previewed;
    } else {
        result.commit = Some(adjustment);
        result.outcome = SliderEditOutcome::Committed;
    }
    Ok(result)
}
