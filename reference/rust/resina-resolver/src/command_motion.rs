use crate::{
    CommandPaintError, CommandPaintInput, CommandPaintIr, CompiledTheme,
    command_paint::resolve_command_paint_with_response,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    CommandAppearance, CommandResponse, MaterialFamily, SpringDynamics, SpringState,
};
use resina_motion::{SpringTrajectorySample, sample_spring_trajectory};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandMotionChannel {
    pub dynamics: SpringDynamics,
    pub initial: SpringState,
}
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandMotionChannels {
    pub body_mix: CommandMotionChannel,
    pub depth_scale: CommandMotionChannel,
}
pub struct CommandMotionInput<'a> {
    pub command: CommandPaintInput<'a>,
    pub channels: &'a CommandMotionChannels,
    pub time: f64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandMotionPolicy {
    Spring,
    CastImmediate,
    ReducedMotion,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandProjection {
    None,
    LowerBound,
    UpperBound,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandMotionIr {
    schema_version: &'static str,
    policy: CommandMotionPolicy,
    target: CommandResponse,
    body_mix: SpringTrajectorySample,
    depth_scale: SpringTrajectorySample,
    body_mix_projection: CommandProjection,
    depth_scale_projection: CommandProjection,
    command: CommandPaintIr,
}
impl CommandMotionIr {
    pub fn policy(&self) -> CommandMotionPolicy {
        self.policy
    }
    pub fn target(&self) -> CommandResponse {
        self.target
    }
    pub fn body_mix(&self) -> &SpringTrajectorySample {
        &self.body_mix
    }
    pub fn depth_scale(&self) -> &SpringTrajectorySample {
        &self.depth_scale
    }
    pub fn body_mix_projection(&self) -> CommandProjection {
        self.body_mix_projection
    }
    pub fn depth_scale_projection(&self) -> CommandProjection {
        self.depth_scale_projection
    }
    pub fn command(&self) -> &CommandPaintIr {
        &self.command
    }
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
pub fn resolve_command_motion(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    input: CommandMotionInput<'_>,
) -> Result<CommandMotionIr, CommandPaintError> {
    let mut sampled = None;
    let command = resolve_command_paint_with_response(
        theme,
        environment,
        input.command,
        |family, target| {
            let policy = if environment.accessibility_preferences().reduced_motion {
                CommandMotionPolicy::ReducedMotion
            } else if family == MaterialFamily::Cast {
                CommandMotionPolicy::CastImmediate
            } else {
                CommandMotionPolicy::Spring
            };
            let immediate = policy != CommandMotionPolicy::Spring;
            let sample = |channel: &CommandMotionChannel, target| {
                sample_spring_trajectory(
                    &channel.dynamics,
                    channel.initial,
                    target,
                    input.time,
                    immediate,
                )
                .map_err(CommandPaintError::Motion)
            };
            let body_mix = sample(&input.channels.body_mix, target.body_mix())?;
            let depth_scale = sample(&input.channels.depth_scale, target.depth_scale())?;
            let (mix, mix_projection) = project(body_mix.state().position(), -1.0, 1.0);
            let (depth, depth_projection) = project(depth_scale.state().position(), 0.0, 1.0);
            let response = CommandResponse::try_new(mix, depth)
                .expect("finite samples projected into response bounds");
            sampled = Some((
                policy,
                target,
                body_mix,
                depth_scale,
                mix_projection,
                depth_projection,
            ));
            Ok(response)
        },
    )?;
    let (policy, target, body_mix, depth_scale, body_mix_projection, depth_scale_projection) =
        sampled.expect("successful paint samples both channels");
    Ok(CommandMotionIr {
        schema_version: "0.1.0",
        policy,
        target,
        body_mix,
        depth_scale,
        body_mix_projection,
        depth_scale_projection,
        command,
    })
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    surface: crate::surface_paint::SurfacePaintRequest,
    command_appearance: CommandAppearance,
    channels: CommandMotionChannels,
    time: f64,
}
pub fn resolve_command_motion_source(source: &str) -> Result<CommandMotionIr, CommandPaintError> {
    let document = resina_tokens::parse_token_document(source).map_err(CommandPaintError::Parse)?;
    let request: Request = serde_json::from_value(document).map_err(CommandPaintError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(CommandPaintError::UnsupportedVersion);
    }
    let (theme, environment, owned) =
        crate::surface_paint::parse_surface_paint_request(request.surface)?;
    resolve_command_motion(
        &theme,
        &environment,
        CommandMotionInput {
            command: CommandPaintInput {
                surface: owned.input(),
                command_appearance: &request.command_appearance,
            },
            channels: &request.channels,
            time: request.time,
        },
    )
}
