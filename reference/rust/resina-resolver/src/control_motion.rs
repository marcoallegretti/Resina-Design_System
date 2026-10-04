use crate::{CommandMotionChannels, CommandMotionPolicy, CommandProjection};
use resina_model::{CommandResponse, MaterialFamily};
use resina_motion::{SpringError, SpringTrajectorySample, sample_spring_trajectory};

pub(crate) struct ControlMotionSample {
    pub policy: CommandMotionPolicy,
    pub body_mix: SpringTrajectorySample,
    pub depth_scale: SpringTrajectorySample,
    pub body_mix_projection: CommandProjection,
    pub depth_scale_projection: CommandProjection,
    pub response: CommandResponse,
}
fn project(value: f64, lower: f64, upper: f64) -> (f64, CommandProjection) {
    if value < lower {
        (lower, CommandProjection::LowerBound)
    } else if value > upper {
        (upper, CommandProjection::UpperBound)
    } else {
        (value, CommandProjection::None)
    }
}
pub(crate) fn sample_control_response(
    family: MaterialFamily,
    reduced_motion: bool,
    target: CommandResponse,
    channels: &CommandMotionChannels,
    time: f64,
) -> Result<ControlMotionSample, SpringError> {
    let policy = if reduced_motion {
        CommandMotionPolicy::ReducedMotion
    } else if family == MaterialFamily::Cast {
        CommandMotionPolicy::CastImmediate
    } else {
        CommandMotionPolicy::Spring
    };
    let immediate = policy != CommandMotionPolicy::Spring;
    let body_mix = sample_spring_trajectory(
        &channels.body_mix.dynamics,
        channels.body_mix.initial,
        target.body_mix(),
        time,
        immediate,
    )?;
    let depth_scale = sample_spring_trajectory(
        &channels.depth_scale.dynamics,
        channels.depth_scale.initial,
        target.depth_scale(),
        time,
        immediate,
    )?;
    let (mix, body_mix_projection) = project(body_mix.state().position(), -1.0, 1.0);
    let (depth, depth_scale_projection) = project(depth_scale.state().position(), 0.0, 1.0);
    let response = CommandResponse::try_new(mix, depth)
        .expect("finite samples projected into response bounds");
    Ok(ControlMotionSample {
        policy,
        body_mix,
        depth_scale,
        body_mix_projection,
        depth_scale_projection,
        response,
    })
}
