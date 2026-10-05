use crate::{
    SliderAdjustment, SliderAdjustmentError, SliderAdjustmentInput, SliderAdjustmentIr,
    SliderEditSession, SliderStopAdjustment, SliderStopInput, SliderStopsError, SliderValueIr,
    SliderValuePolicy, resolve_slider_adjustment, resolve_slider_stop_adjustment,
};
use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliderPresentation {
    schema_version: &'static str,
    revision: String,
    committed: SliderValueIr,
    visible: SliderValueIr,
    value_policy: SliderValuePolicy,
    editing: bool,
}
impl SliderPresentation {
    pub fn try_new(
        committed: &SliderValueIr,
        revision: &str,
        value_policy: &SliderValuePolicy,
        edit: Option<&SliderEditSession>,
    ) -> Result<Self, SliderPresentationError> {
        if revision.is_empty() {
            return Err(SliderPresentationError::InvalidRevision);
        }
        value_policy
            .validate_current(committed)
            .map_err(SliderPresentationError::ValuePolicy)?;
        if edit.is_some_and(|edit| {
            edit.baseline() != committed
                || edit.revision() != revision
                || edit.value_policy() != value_policy
        }) {
            return Err(SliderPresentationError::EditConflict);
        }
        let visible = edit.map_or(committed, SliderEditSession::preview);
        value_policy
            .validate_current(visible)
            .map_err(SliderPresentationError::ValuePolicy)?;
        Ok(Self {
            schema_version: "0.1.0",
            revision: revision.to_owned(),
            committed: *committed,
            visible: *visible,
            value_policy: value_policy.clone(),
            editing: edit.is_some(),
        })
    }
    pub fn committed(&self) -> &SliderValueIr {
        &self.committed
    }
    pub fn visible(&self) -> &SliderValueIr {
        &self.visible
    }
    pub fn revision(&self) -> &str {
        &self.revision
    }
    pub fn value_policy(&self) -> &SliderValuePolicy {
        &self.value_policy
    }
    pub fn editing(&self) -> bool {
        self.editing
    }
}

pub struct SliderPresentationCommitInput<'a> {
    pub presentation: &'a SliderPresentation,
    pub candidate: &'a SliderAdjustmentIr,
    pub candidate_base: &'a SliderValueIr,
    pub candidate_revision: &'a str,
    pub enabled: bool,
    pub read_only: bool,
    pub source_available: bool,
}

#[derive(Debug)]
pub enum SliderPresentationError {
    InvalidRevision,
    EditConflict,
    StaleIntent,
    IntentBounds,
    ValuePolicy(SliderStopsError),
    Adjustment(SliderAdjustmentError),
}
impl fmt::Display for SliderPresentationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRevision => f.write_str("slider presentation revision must not be empty"),
            Self::EditConflict => f.write_str(
                "slider presentation edit must match committed value, revision and policy",
            ),
            Self::StaleIntent => f.write_str(
                "slider presented intent must use the current visible value and revision",
            ),
            Self::IntentBounds => {
                f.write_str("slider presented intent bounds must match committed bounds")
            }
            Self::ValuePolicy(error) => write!(f, "slider presentation policy: {error}"),
            Self::Adjustment(error) => write!(f, "slider presentation commit: {error}"),
        }
    }
}
impl std::error::Error for SliderPresentationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ValuePolicy(error) => Some(error),
            Self::Adjustment(error) => Some(error),
            _ => None,
        }
    }
}

pub fn resolve_slider_presentation_commit(
    input: SliderPresentationCommitInput<'_>,
) -> Result<SliderAdjustmentIr, SliderPresentationError> {
    let presentation = input.presentation;
    if input.candidate_base != presentation.visible()
        || input.candidate_revision != presentation.revision()
    {
        return Err(SliderPresentationError::StaleIntent);
    }
    let candidate = input.candidate.value();
    if candidate.value().minimum() != presentation.committed.value().minimum()
        || candidate.value().maximum() != presentation.committed.value().maximum()
    {
        return Err(SliderPresentationError::IntentBounds);
    }
    presentation
        .value_policy
        .validate_current(candidate)
        .map_err(SliderPresentationError::ValuePolicy)?;
    let enabled = input.enabled && input.source_available && input.candidate.accepted();
    match &presentation.value_policy {
        SliderValuePolicy::Continuous => resolve_slider_adjustment(SliderAdjustmentInput {
            current: &presentation.committed,
            enabled,
            read_only: input.read_only,
            adjustment: SliderAdjustment::SetValue(candidate.value().value()),
        })
        .map_err(SliderPresentationError::Adjustment),
        SliderValuePolicy::Stops { stops, .. } => resolve_slider_stop_adjustment(SliderStopInput {
            stops,
            current: &presentation.committed,
            enabled,
            read_only: input.read_only,
            adjustment: SliderStopAdjustment::SetValue(candidate.value().value()),
        })
        .map_err(SliderPresentationError::ValuePolicy),
    }
}
