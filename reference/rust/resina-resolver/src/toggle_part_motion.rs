use crate::{
    CommandMotionChannels, CommandMotionPolicy, CommandProjection, CompiledTheme,
    TogglePartPaintError, TogglePartPaintInput, TogglePartPaintIr,
    toggle_part_paint::resolve_toggle_part_paint_with_response,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{CommandAppearance, CommandResponse};
use resina_motion::SpringTrajectorySample;
use serde::{Deserialize, Serialize};

pub struct TogglePartMotionInput<'a> {
    pub part: TogglePartPaintInput<'a>,
    pub channels: &'a CommandMotionChannels,
    pub time: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TogglePartMotionIr {
    schema_version: &'static str,
    policy: CommandMotionPolicy,
    target: CommandResponse,
    body_mix: SpringTrajectorySample,
    depth_scale: SpringTrajectorySample,
    body_mix_projection: CommandProjection,
    depth_scale_projection: CommandProjection,
    part_paint: TogglePartPaintIr,
}
impl TogglePartMotionIr {
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
    pub fn part_paint(&self) -> &TogglePartPaintIr {
        &self.part_paint
    }
}
pub fn resolve_toggle_part_motion(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    input: TogglePartMotionInput<'_>,
) -> Result<TogglePartMotionIr, TogglePartPaintError> {
    let mut sampled = None;
    let part_paint = resolve_toggle_part_paint_with_response(
        theme,
        environment,
        input.part,
        |family, target| {
            let sample = crate::control_motion::sample_control_response(
                family,
                environment.accessibility_preferences().reduced_motion,
                target,
                input.channels,
                input.time,
            )
            .map_err(TogglePartPaintError::Motion)?;
            let response = sample.response;
            sampled = Some((sample, target));
            Ok(response)
        },
    )?;
    let (sample, target) = sampled.expect("successful paint samples both channels");
    Ok(TogglePartMotionIr {
        schema_version: "0.1.0",
        policy: sample.policy,
        target,
        body_mix: sample.body_mix,
        depth_scale: sample.depth_scale,
        body_mix_projection: sample.body_mix_projection,
        depth_scale_projection: sample.depth_scale_projection,
        part_paint,
    })
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    surface: crate::surface_paint::SurfacePaintRequest,
    interaction_appearance: CommandAppearance,
    part: crate::TogglePart,
    checked_color_role: resina_model::ColorRole,
    channels: CommandMotionChannels,
    time: f64,
}
pub fn resolve_toggle_part_motion_source(
    source: &str,
) -> Result<TogglePartMotionIr, TogglePartPaintError> {
    let document =
        resina_tokens::parse_token_document(source).map_err(TogglePartPaintError::Parse)?;
    let request: Request =
        serde_json::from_value(document).map_err(TogglePartPaintError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(TogglePartPaintError::UnsupportedVersion);
    }
    let (theme, environment, owned) =
        crate::surface_paint::parse_surface_paint_request(request.surface)?;
    resolve_toggle_part_motion(
        &theme,
        &environment,
        TogglePartMotionInput {
            part: TogglePartPaintInput {
                surface: owned.input(),
                interaction_appearance: &request.interaction_appearance,
                part: request.part,
                checked_color_role: request.checked_color_role,
            },
            channels: &request.channels,
            time: request.time,
        },
    )
}
