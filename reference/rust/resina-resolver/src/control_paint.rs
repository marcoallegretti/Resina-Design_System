use crate::{
    BoundSurface, CompiledTheme, FocusIrError, HeadlessResolution, OpaqueSurfaceError,
    SurfacePaintInput, SurfacePaintIr, SurfacePaintResolutionError,
    background_contrast::EdgeBackground, focus_ir::resolve_focus_geometry,
    opaque_surface::resolve_opaque_body_geometry, resolve_elevation_depth, resolve_focus_indicator,
    resolve_focus_indicator_over_ranges, resolve_shape_fallback,
    surface_readability::resolve_bound_body_readability,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorRole, CommandAppearance, CommandPhase, CommandResponse, InteractionState,
    OpaqueSurfaceAppearance, SurfaceIntent, SurfaceSize,
};
use std::fmt;

const MINIMUM_STATE_COLOR_DIFFERENCE: f64 = 0.04;
const MINIMUM_STATE_DEPTH_CHANGE: f64 = 1.0;
const DEPTH_TOLERANCE: f64 = 1e-12;

/// A selected hover, pressed or disabled response whose body differs from rest by less than
/// ΔE_OK 0.04 and whose final extrusion differs by less than 1 px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImperceptibleResponse {
    pub phase: CommandPhase,
    pub color_difference: f64,
    pub depth_change: f64,
}
impl fmt::Display for ImperceptibleResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} response is imperceptible from rest: body ΔE_OK {} is below {MINIMUM_STATE_COLOR_DIFFERENCE} and extrusion change {} px is below {MINIMUM_STATE_DEPTH_CHANGE} px",
            self.phase, self.color_difference, self.depth_change
        )
    }
}

pub(crate) struct ControlPaintInput<'a> {
    pub surface: &'a SurfaceIntent,
    pub size: SurfaceSize,
    pub appearance: &'a OpaqueSurfaceAppearance,
    pub foreground_role: ColorRole,
    pub post_treatment_backdrop: Option<&'a crate::SrgbFallback>,
    pub adjacent: EdgeBackground<'a>,
    pub surrounding: Option<EdgeBackground<'a>>,
    pub minimum_content_contrast: f64,
    pub minimum_edge_contrast: f64,
}

pub(crate) fn resolve_control_paint(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    snapshot: &HeadlessResolution,
    input: SurfacePaintInput<'_>,
    binding: BoundSurface,
    response: CommandResponse,
    focus_owner: bool,
) -> Result<SurfacePaintIr, SurfacePaintResolutionError> {
    resolve_control_paint_inputs(
        theme,
        environment,
        snapshot,
        ControlPaintInput {
            surface: input.body.surface,
            size: input.body.size,
            appearance: input.body.appearance,
            foreground_role: input.body.foreground_role,
            post_treatment_backdrop: input.body.post_treatment_backdrop,
            adjacent: EdgeBackground::Uniform(input.body.adjacent_color),
            surrounding: input.surrounding_color.map(EdgeBackground::Uniform),
            minimum_content_contrast: input.body.minimum_content_contrast,
            minimum_edge_contrast: input.body.minimum_edge_contrast,
        },
        binding,
        response,
        focus_owner,
    )
}

pub(crate) fn resolve_control_paint_inputs(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    snapshot: &HeadlessResolution,
    input: ControlPaintInput<'_>,
    mut binding: BoundSurface,
    response: CommandResponse,
    focus_owner: bool,
) -> Result<SurfacePaintIr, SurfacePaintResolutionError> {
    let surface = input.surface;
    binding.apply_command_response(response);
    let readable = resolve_bound_body_readability(
        binding,
        snapshot,
        input.foreground_role,
        input.post_treatment_backdrop,
        input.adjacent,
        input.minimum_content_contrast,
        input.minimum_edge_contrast,
    )
    .map_err(|error| SurfacePaintResolutionError::Body(OpaqueSurfaceError::Readability(error)))?;
    let appearance = input.appearance;
    let depth = semantic_depth(theme, appearance, surface)? * response.depth_scale();
    let size = input.size;
    let body = resolve_opaque_body_geometry(theme, environment, size, appearance, &readable, depth)
        .map_err(SurfacePaintResolutionError::Body)?;
    let focus = if focus_owner
        && surface
            .states()
            .states()
            .contains(&InteractionState::Focused)
    {
        let surrounding = input
            .surrounding
            .ok_or(SurfacePaintResolutionError::MissingSurroundingColor)?;
        let indicator = match surrounding {
            EdgeBackground::Uniform(color) => resolve_focus_indicator(surface, snapshot, color),
            EdgeBackground::Ranges(ranges) => {
                resolve_focus_indicator_over_ranges(surface, snapshot, ranges)
            }
        }
        .map_err(|e| SurfacePaintResolutionError::Focus(FocusIrError::Indicator(e)))?;
        let radii = resolve_shape_fallback(
            surface.form().shape(),
            size,
            appearance.shape_assignments(),
            theme.tokens(),
        )
        .map_err(|e| SurfacePaintResolutionError::Focus(FocusIrError::Shape(e)))?;
        Some(
            resolve_focus_geometry(
                environment,
                size,
                appearance.key_light(),
                indicator,
                radii,
                depth,
            )
            .map_err(SurfacePaintResolutionError::Focus)?,
        )
    } else {
        None
    };
    Ok(SurfacePaintIr {
        schema_version: "0.1.0",
        body,
        focus,
    })
}

fn semantic_depth(
    theme: &CompiledTheme,
    appearance: &OpaqueSurfaceAppearance,
    surface: &SurfaceIntent,
) -> Result<f64, SurfacePaintResolutionError> {
    let depths = resolve_elevation_depth(appearance.depth_assignments(), theme.tokens())
        .map_err(|error| SurfacePaintResolutionError::Body(OpaqueSurfaceError::Depth(error)))?;
    Ok(depths[&surface.form().elevation()]["value"]
        .as_f64()
        .expect("validated depth is numeric"))
}

fn oklab_difference(first: &crate::SrgbFallback, second: &crate::SrgbFallback) -> f64 {
    let first = resina_color::srgb_to_oklab(first.components())
        .expect("bound opaque body has encoded sRGB channels in [0, 1]");
    let second = resina_color::srgb_to_oklab(second.components())
        .expect("mixed opaque body has encoded sRGB channels in [0, 1]");
    first
        .iter()
        .zip(second)
        .map(|(first, second)| (first - second).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// Measures every authored hover, pressed and disabled response of the bound family on the opaque
/// body it is applied to, whichever state is being resolved.
pub(crate) fn imperceptible_profile(
    theme: &CompiledTheme,
    appearance: &OpaqueSurfaceAppearance,
    surface: &SurfaceIntent,
    binding: &BoundSurface,
    profile: &CommandAppearance,
) -> Result<Option<ImperceptibleResponse>, SurfacePaintResolutionError> {
    let depth = semantic_depth(theme, appearance, surface)?;
    for phase in [
        CommandPhase::Hover,
        CommandPhase::Pressed,
        CommandPhase::Disabled,
    ] {
        let response = profile
            .response_for(binding.material_family(), phase)
            .expect("a bound command family has an authored profile");
        if let Some(imperceptible) = imperceptible_response(binding, depth, phase, response) {
            return Ok(Some(imperceptible));
        }
    }
    Ok(None)
}

fn imperceptible_response(
    binding: &BoundSurface,
    depth: f64,
    phase: CommandPhase,
    response: CommandResponse,
) -> Option<ImperceptibleResponse> {
    let rest = binding.opaque_color_fallback();
    let color_difference = oklab_difference(rest, &crate::surface::mix_body_color(rest, response));
    let depth_change = (depth * response.depth_scale() - depth).abs();
    if color_difference >= MINIMUM_STATE_COLOR_DIFFERENCE
        || depth_change >= MINIMUM_STATE_DEPTH_CHANGE - DEPTH_TOLERANCE
    {
        return None;
    }
    Some(ImperceptibleResponse {
        phase,
        color_difference,
        depth_change,
    })
}
