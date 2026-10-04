use crate::{ToggleSnapshot, ToggleSnapshotError};
use resina_model::{MaterialFamily, PhysicalBounds, SpringDynamics, SpringState};
use resina_motion::{SpringError, SpringTrajectorySample, sample_spring_trajectory};
use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ToggleTravelPolicy {
    Spring,
    CastImmediate,
    ReducedMotion,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ToggleTravelProjection {
    None,
    OffEndpoint,
    OnEndpoint,
}
#[derive(Debug)]
pub struct ToggleTravelSnapshot<'snapshot, 'paint> {
    snapshot: &'snapshot ToggleSnapshot<'paint>,
    policy: ToggleTravelPolicy,
    trajectory: SpringTrajectorySample,
    projection: ToggleTravelProjection,
    thumb_bounds: PhysicalBounds,
}
impl<'snapshot, 'paint> ToggleTravelSnapshot<'snapshot, 'paint> {
    pub fn snapshot(&self) -> &'snapshot ToggleSnapshot<'paint> {
        self.snapshot
    }
    pub fn policy(&self) -> ToggleTravelPolicy {
        self.policy
    }
    pub fn trajectory(&self) -> &SpringTrajectorySample {
        &self.trajectory
    }
    pub fn projection(&self) -> ToggleTravelProjection {
        self.projection
    }
    pub fn thumb_bounds(&self) -> PhysicalBounds {
        self.thumb_bounds
    }
}
#[derive(Debug)]
pub enum ToggleTravelError {
    Motion(SpringError),
    Placement(ToggleSnapshotError),
    UnsupportedFamily,
    NumericRange,
}
impl fmt::Display for ToggleTravelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Motion(error) => write!(f, "toggle travel: {error}"),
            Self::Placement(error) => write!(f, "toggle travel placement: {error}"),
            Self::UnsupportedFamily => {
                f.write_str("toggle travel requires a persistent thumb material")
            }
            Self::NumericRange => {
                f.write_str("toggle travel placement exceeds representable arithmetic")
            }
        }
    }
}
impl std::error::Error for ToggleTravelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Motion(error) => Some(error),
            Self::Placement(error) => Some(error),
            _ => None,
        }
    }
}
pub fn resolve_toggle_travel<'snapshot, 'paint>(
    snapshot: &'snapshot ToggleSnapshot<'paint>,
    dynamics: &SpringDynamics,
    initial: SpringState,
    time: f64,
) -> Result<ToggleTravelSnapshot<'snapshot, 'paint>, ToggleTravelError> {
    let family = snapshot.thumb().paint().body().material_family();
    let policy = if snapshot.reduced_motion {
        ToggleTravelPolicy::ReducedMotion
    } else {
        match family {
            MaterialFamily::Cast => ToggleTravelPolicy::CastImmediate,
            MaterialFamily::Frost | MaterialFamily::Elastomer => ToggleTravelPolicy::Spring,
            MaterialFamily::Gel => return Err(ToggleTravelError::UnsupportedFamily),
        }
    };
    let target = if snapshot.layout().checked() {
        1.0
    } else {
        0.0
    };
    let trajectory = sample_spring_trajectory(
        dynamics,
        initial,
        target,
        time,
        policy != ToggleTravelPolicy::Spring,
    )
    .map_err(ToggleTravelError::Motion)?;
    let raw = trajectory.state().position();
    let (progress, projection) = if raw < 0.0 {
        (0.0, ToggleTravelProjection::OffEndpoint)
    } else if raw > 1.0 {
        (1.0, ToggleTravelProjection::OnEndpoint)
    } else {
        (raw, ToggleTravelProjection::None)
    };
    let off = snapshot.layout().off_thumb_bounds();
    let on = snapshot.layout().on_thumb_bounds();
    let thumb_bounds = if progress == 0.0 {
        off
    } else if progress == 1.0 {
        on
    } else {
        let x = off.x + (on.x - off.x) * progress;
        if !x.is_finite() || x == off.x || x == on.x {
            return Err(ToggleTravelError::NumericRange);
        }
        PhysicalBounds { x, ..off }
    };
    snapshot
        .validate_thumb_placement(thumb_bounds)
        .map_err(ToggleTravelError::Placement)?;
    Ok(ToggleTravelSnapshot {
        snapshot,
        policy,
        trajectory,
        projection,
        thumb_bounds,
    })
}
