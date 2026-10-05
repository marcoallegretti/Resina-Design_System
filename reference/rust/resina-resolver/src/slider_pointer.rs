use crate::{
    CaptureChange, HitRegionError, HitRegionIr, SliderAdjustmentIr, SliderAnchorError,
    SliderEditAction, SliderEditError, SliderEditInput, SliderEditResult, SliderEditSession,
    SliderLayoutIr, SliderPointerAnchor, SliderStopsError, SliderValueIr, SliderValuePolicy,
    resolve_slider_edit,
};
use resina_model::PhysicalVector;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SliderPointerTarget {
    Thumb,
    Track,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SliderPointerRouting {
    Continuous,
    TerminationOnly,
    Unavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SliderPointerPhase {
    Pending,
    Acquired,
    TerminationOnly,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PointerHold {
    id: String,
    target: SliderPointerTarget,
    phase: SliderPointerPhase,
    control_region: HitRegionIr,
    target_region: HitRegionIr,
    anchor: SliderPointerAnchor,
    edit: SliderEditSession,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliderPointerState {
    schema_version: &'static str,
    hold: Option<PointerHold>,
}
impl SliderPointerState {
    pub fn idle() -> Self {
        Self {
            schema_version: "0.1.0",
            hold: None,
        }
    }
    pub fn held_id(&self) -> Option<&str> {
        self.hold.as_ref().map(|hold| hold.id.as_str())
    }
    pub fn phase(&self) -> Option<SliderPointerPhase> {
        self.hold.as_ref().map(|hold| hold.phase)
    }
    pub fn target(&self) -> Option<SliderPointerTarget> {
        self.hold.as_ref().map(|hold| hold.target)
    }
    pub fn edit(&self) -> Option<&SliderEditSession> {
        self.hold.as_ref().map(|hold| &hold.edit)
    }
}
#[derive(Debug, Clone, Copy)]
pub enum SliderPointerEvent<'a> {
    Down {
        id: &'a str,
        point: PhysicalVector,
        target: SliderPointerTarget,
        region: &'a HitRegionIr,
    },
    Move {
        id: &'a str,
        point: PhysicalVector,
    },
    Up {
        id: &'a str,
        point: PhysicalVector,
    },
    RoutingAcquired {
        id: &'a str,
    },
    RoutingLost {
        id: &'a str,
    },
    Cancel {
        id: &'a str,
    },
    Abort,
    Refresh,
}
impl<'a> SliderPointerEvent<'a> {
    fn id(self) -> Option<&'a str> {
        match self {
            Self::Down { id, .. }
            | Self::Move { id, .. }
            | Self::Up { id, .. }
            | Self::RoutingAcquired { id }
            | Self::RoutingLost { id }
            | Self::Cancel { id } => Some(id),
            Self::Abort | Self::Refresh => None,
        }
    }
    fn point(self) -> Option<PhysicalVector> {
        match self {
            Self::Down { point, .. } | Self::Move { point, .. } | Self::Up { point, .. } => {
                Some(point)
            }
            _ => None,
        }
    }
}
pub struct SliderPointerInput<'a> {
    pub state: &'a SliderPointerState,
    pub current: &'a SliderValueIr,
    pub revision: &'a str,
    pub value_policy: &'a SliderValuePolicy,
    pub layout: &'a SliderLayoutIr,
    pub control_region: &'a HitRegionIr,
    pub enabled: bool,
    pub read_only: bool,
    pub routing: SliderPointerRouting,
    pub event: SliderPointerEvent<'a>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SliderPointerOutcome {
    Ignored,
    Armed,
    RoutingAcquired,
    Previewed,
    Committed,
    Cancelled,
    Unavailable,
    Conflict,
    DeliveryUnavailable,
    LayoutChanged,
    TargetChanged,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliderPointerResult {
    schema_version: &'static str,
    state: SliderPointerState,
    preview: SliderValueIr,
    commit: Option<SliderAdjustmentIr>,
    routing: Option<CaptureChange>,
    outcome: SliderPointerOutcome,
}
impl SliderPointerResult {
    pub fn state(&self) -> &SliderPointerState {
        &self.state
    }
    pub fn preview(&self) -> &SliderValueIr {
        &self.preview
    }
    pub fn commit(&self) -> Option<&SliderAdjustmentIr> {
        self.commit.as_ref()
    }
    pub fn routing(&self) -> Option<&CaptureChange> {
        self.routing.as_ref()
    }
    pub fn outcome(&self) -> SliderPointerOutcome {
        self.outcome
    }
}
#[derive(Debug)]
pub enum SliderPointerError {
    InvalidRevision,
    InvalidIdentity,
    InvalidPoint,
    InvalidControlRegion,
    InvalidTargetRegion,
    IncoherentLayout,
    Hit(HitRegionError),
    Anchor(SliderAnchorError),
    Edit(SliderEditError),
    ValuePolicy(SliderStopsError),
}
impl fmt::Display for SliderPointerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRevision => f.write_str("slider pointer revision must not be empty"),
            Self::InvalidIdentity => f.write_str("slider pointer identity must not be empty"),
            Self::InvalidPoint => f.write_str("slider pointer point must be finite"),
            Self::InvalidControlRegion => {
                f.write_str("slider control hit region must cover its allocation")
            }
            Self::InvalidTargetRegion => f.write_str(
                "slider target region must cover its part and fit within the control region",
            ),
            Self::IncoherentLayout => {
                f.write_str("slider pointer layout must contain its current visible value")
            }
            Self::Hit(error) => write!(f, "slider pointer: {error}"),
            Self::Anchor(error) => write!(f, "slider pointer: {error}"),
            Self::Edit(error) => write!(f, "slider pointer: {error}"),
            Self::ValuePolicy(error) => write!(f, "slider pointer policy: {error}"),
        }
    }
}
impl std::error::Error for SliderPointerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hit(error) => Some(error),
            Self::Anchor(error) => Some(error),
            Self::Edit(error) => Some(error),
            Self::ValuePolicy(error) => Some(error),
            _ => None,
        }
    }
}
fn close(input: &SliderPointerInput<'_>, outcome: SliderPointerOutcome) -> SliderPointerResult {
    SliderPointerResult {
        schema_version: "0.1.0",
        state: SliderPointerState::idle(),
        preview: *input.current,
        commit: None,
        routing: input.state.hold.as_ref().and_then(|hold| {
            (hold.phase != SliderPointerPhase::TerminationOnly).then(|| CaptureChange::Release {
                id: hold.id.clone(),
            })
        }),
        outcome,
    }
}
fn preview(
    input: &SliderPointerInput<'_>,
    hold: &PointerHold,
    point: PhysicalVector,
) -> Result<SliderEditResult, SliderPointerError> {
    let desired_origin = hold
        .anchor
        .desired_origin(input.layout, point)
        .map_err(SliderPointerError::Anchor)?;
    resolve_slider_edit(SliderEditInput {
        session: &hold.edit,
        current: input.current,
        revision: input.revision,
        value_policy: input.value_policy,
        enabled: input.enabled,
        read_only: input.read_only,
        action: SliderEditAction::Preview {
            layout: input.layout,
            desired_origin,
        },
    })
    .map_err(SliderPointerError::Edit)
}
pub fn resolve_slider_pointer(
    input: SliderPointerInput<'_>,
) -> Result<SliderPointerResult, SliderPointerError> {
    if input.revision.is_empty() {
        return Err(SliderPointerError::InvalidRevision);
    }
    if input.event.id().is_some_and(str::is_empty) {
        return Err(SliderPointerError::InvalidIdentity);
    }
    if input
        .event
        .point()
        .is_some_and(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return Err(SliderPointerError::InvalidPoint);
    }
    let mut inside = false;
    if let SliderPointerEvent::Down {
        point,
        target,
        region,
        ..
    } = input.event
    {
        if !input
            .control_region
            .contains_bounds(input.layout.allocation_bounds())
            .map_err(SliderPointerError::Hit)?
        {
            return Err(SliderPointerError::InvalidControlRegion);
        }
        let part = match target {
            SliderPointerTarget::Thumb => input.layout.thumb_bounds(),
            SliderPointerTarget::Track => input.layout.track_bounds(),
        };
        if !region
            .contains_bounds(part)
            .map_err(SliderPointerError::Hit)?
            || !input
                .control_region
                .contains_bounds(region.bounds())
                .map_err(SliderPointerError::Hit)?
        {
            return Err(SliderPointerError::InvalidTargetRegion);
        }
        inside = region.contains(point).map_err(SliderPointerError::Hit)?;
    }
    if input.state.hold.is_none() {
        input
            .value_policy
            .validate_current(input.current)
            .map_err(SliderPointerError::ValuePolicy)?;
    }
    let mut result = SliderPointerResult {
        schema_version: "0.1.0",
        state: input.state.clone(),
        preview: input
            .state
            .hold
            .as_ref()
            .map_or(*input.current, |hold| *hold.edit.preview()),
        commit: None,
        routing: None,
        outcome: SliderPointerOutcome::Ignored,
    };
    if let Some(hold) = &input.state.hold {
        let own = input.event.id() == Some(hold.id.as_str());
        if matches!(input.event, SliderPointerEvent::Abort)
            || (own
                && matches!(
                    input.event,
                    SliderPointerEvent::Cancel { .. } | SliderPointerEvent::RoutingLost { .. }
                ))
        {
            return Ok(close(&input, SliderPointerOutcome::Cancelled));
        }
        if input.revision != hold.edit.revision()
            || input.current != hold.edit.baseline()
            || input.value_policy != hold.edit.value_policy()
        {
            return Ok(close(&input, SliderPointerOutcome::Conflict));
        }
        if !input.enabled || input.read_only {
            return Ok(close(&input, SliderPointerOutcome::Unavailable));
        }
        if input.routing == SliderPointerRouting::Unavailable
            || (hold.phase != SliderPointerPhase::TerminationOnly
                && input.routing != SliderPointerRouting::Continuous)
        {
            return Ok(close(&input, SliderPointerOutcome::DeliveryUnavailable));
        }
        if input.layout.value() != hold.edit.preview() {
            return Err(SliderPointerError::IncoherentLayout);
        }
        if input.control_region != &hold.control_region {
            return Ok(close(&input, SliderPointerOutcome::TargetChanged));
        }
        if !input
            .control_region
            .contains_bounds(input.layout.allocation_bounds())
            .map_err(SliderPointerError::Hit)?
        {
            return Ok(close(&input, SliderPointerOutcome::LayoutChanged));
        }
        match hold.anchor.validate_layout(input.layout) {
            Ok(()) => {}
            Err(SliderAnchorError::IncompatibleLayout) => {
                return Ok(close(&input, SliderPointerOutcome::LayoutChanged));
            }
            Err(error) => return Err(SliderPointerError::Anchor(error)),
        }
        match input.event {
            SliderPointerEvent::RoutingAcquired { .. }
                if own && hold.phase == SliderPointerPhase::Pending =>
            {
                result
                    .state
                    .hold
                    .as_mut()
                    .expect("active route must retain its hold")
                    .phase = SliderPointerPhase::Acquired;
                result.outcome = SliderPointerOutcome::RoutingAcquired;
            }
            SliderPointerEvent::Move { point, .. }
                if own && hold.phase == SliderPointerPhase::Acquired =>
            {
                let edit = preview(&input, hold, point)?;
                result.preview = *edit.preview();
                result
                    .state
                    .hold
                    .as_mut()
                    .expect("active route must retain its hold")
                    .edit = edit
                    .session()
                    .expect("live preview must retain its session")
                    .clone();
                result.outcome = SliderPointerOutcome::Previewed;
            }
            SliderPointerEvent::Up { point, .. } if own => {
                if hold.phase == SliderPointerPhase::Pending
                    || (hold.phase == SliderPointerPhase::TerminationOnly
                        && !hold
                            .target_region
                            .contains(point)
                            .map_err(SliderPointerError::Hit)?)
                {
                    return Ok(close(&input, SliderPointerOutcome::Cancelled));
                }
                let next = preview(&input, hold, point)?;
                let edit = resolve_slider_edit(SliderEditInput {
                    session: next
                        .session()
                        .expect("live preview must retain its session"),
                    current: input.current,
                    revision: input.revision,
                    value_policy: input.value_policy,
                    enabled: input.enabled,
                    read_only: input.read_only,
                    action: SliderEditAction::Commit,
                })
                .map_err(SliderPointerError::Edit)?;
                result = close(&input, SliderPointerOutcome::Committed);
                result.preview = *edit.preview();
                result.commit = Some(*edit.commit().expect("live commit must return its intent"));
            }
            _ => {}
        }
    } else if let SliderPointerEvent::Down {
        id,
        point,
        target,
        region,
    } = input.event
        && inside
    {
        if input.layout.value() != input.current {
            return Err(SliderPointerError::IncoherentLayout);
        }
        let anchor = match target {
            SliderPointerTarget::Thumb => SliderPointerAnchor::grab(input.layout, point),
            SliderPointerTarget::Track => SliderPointerAnchor::center(input.layout),
        }
        .map_err(SliderPointerError::Anchor)?;
        let mut hold = PointerHold {
            id: id.to_owned(),
            target,
            phase: if input.routing == SliderPointerRouting::Continuous {
                SliderPointerPhase::Pending
            } else {
                SliderPointerPhase::TerminationOnly
            },
            control_region: *input.control_region,
            target_region: *region,
            anchor,
            edit: SliderEditSession::begin(input.current, input.revision, input.value_policy)
                .map_err(SliderPointerError::Edit)?,
        };
        let edit = preview(&input, &hold, point)?;
        if !input.enabled || input.read_only {
            return Ok(close(&input, SliderPointerOutcome::Unavailable));
        }
        if input.routing == SliderPointerRouting::Unavailable {
            return Ok(close(&input, SliderPointerOutcome::DeliveryUnavailable));
        }
        hold.edit = edit
            .session()
            .expect("live preview must retain its session")
            .clone();
        result.preview = *edit.preview();
        if hold.phase == SliderPointerPhase::Pending {
            result.routing = Some(CaptureChange::Acquire { id: id.to_owned() });
        }
        result.state.hold = Some(hold);
        result.outcome = SliderPointerOutcome::Armed;
    }
    Ok(result)
}
