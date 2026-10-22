use crate::{
    BoundSurface, CompiledTheme, FocusIrError, HeadlessResolution, LitControlBodyError,
    LitControlBodyInput, LitControlIdentity, LitControlPaintIr, OpaqueSurfaceError,
    SurfacePaintInput, SurfacePaintIr, SurfacePaintResolutionError, SurfaceReliefProfile,
    background_contrast::EdgeBackground, focus_ir::resolve_focus_geometry,
    opaque_surface::resolve_opaque_body_geometry, resolve_elevation_depth, resolve_focus_indicator,
    resolve_focus_indicator_over_ranges, resolve_lit_control_body, resolve_shape_fallback,
    surface_readability::resolve_bound_body_readability,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorRole, CommandAppearance, CommandPhase, CommandResponse, InteractionState,
    MaterialLightChannels, OpaqueSurfaceAppearance, SurfaceIntent, SurfaceSize,
};
use std::fmt;

const MINIMUM_STATE_COLOR_DIFFERENCE: f64 = 0.04;
const MINIMUM_STATE_DEPTH_CHANGE: f64 = 1.0;
const DEPTH_TOLERANCE: f64 = 1e-12;

/// An authored response whose body differs from the reference phase's body by less than
/// ΔE_OK 0.04 and whose final extrusion differs by less than 1 px. The reference is rest, or
/// pressed for the disabled response.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImperceptibleResponse {
    pub phase: CommandPhase,
    pub reference: CommandPhase,
    pub color_difference: f64,
    pub depth_change: f64,
}
impl fmt::Display for ImperceptibleResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} response is imperceptible from {}: body ΔE_OK {} is below {MINIMUM_STATE_COLOR_DIFFERENCE} and extrusion change {} px is below {MINIMUM_STATE_DEPTH_CHANGE} px",
            self.phase,
            format!("{:?}", self.reference).to_lowercase(),
            self.color_difference,
            self.depth_change
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
    let focus = resolve_control_focus(theme, environment, snapshot, &input, depth, focus_owner)?;
    Ok(SurfacePaintIr {
        schema_version: "0.1.0",
        body,
        focus,
    })
}

#[derive(Debug)]
pub(crate) enum LitControlPaintError {
    Paint(SurfacePaintResolutionError),
    Body(LitControlBodyError),
}

impl From<SurfacePaintResolutionError> for LitControlPaintError {
    fn from(error: SurfacePaintResolutionError) -> Self {
        Self::Paint(error)
    }
}

/// Resolves one control part as a lit control body, with the relief profile selected by its part
/// name, and its independent focus ring when the part owns it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_lit_control_paint(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    snapshot: &HeadlessResolution,
    input: SurfacePaintInput<'_>,
    mut binding: BoundSurface,
    response: CommandResponse,
    light: &MaterialLightChannels,
    part: (&str, SurfaceReliefProfile),
    focus_owner: bool,
) -> Result<LitControlPaintIr, LitControlPaintError> {
    let input = control_paint_input(input);
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
    if readable.body().alpha() != 1.0 {
        return Err(SurfacePaintResolutionError::Body(OpaqueSurfaceError::TranslucentBody).into());
    }
    let appearance = input.appearance;
    let depth = semantic_depth(theme, appearance, surface)? * response.depth_scale();
    let radii = resolve_shape_fallback(
        surface.form().shape(),
        input.size,
        appearance.shape_assignments(),
        theme.tokens(),
    )
    .map_err(|e| SurfacePaintResolutionError::Body(OpaqueSurfaceError::Shape(e)))?;
    let bound = readable.binding();
    let (name, relief) = part;
    let body = resolve_lit_control_body(LitControlBodyInput {
        identity: LitControlIdentity {
            material_role: bound.material_role(),
            color_role: bound.color_role(),
            material_family: bound.material_family(),
            form: bound.form().clone(),
            states: bound.states().clone(),
            frost_representation: readable.frost_representation(),
            foreground_role: readable.foreground_role(),
            content_fallback_applied: readable.content_fallback_applied(),
        },
        size: input.size,
        radii,
        relief,
        body: readable.body(),
        pigment_profiles: appearance.pigment_profiles(),
        key_light: appearance.key_light(),
        light,
        environment,
        edge: readable.edge().clone(),
        edge_width: appearance.bands_for(bound.material_family()).edge_width(),
        foreground: readable.foreground().clone(),
        minimum_content_contrast: input.minimum_content_contrast,
        depth,
    })
    .map_err(|error| LitControlPaintError::Body(error.for_part(name)))?;
    let focus = resolve_control_focus(theme, environment, snapshot, &input, depth, focus_owner)?;
    Ok(LitControlPaintIr::new(body, focus))
}

fn control_paint_input(input: SurfacePaintInput<'_>) -> ControlPaintInput<'_> {
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
    }
}

fn resolve_control_focus(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    snapshot: &HeadlessResolution,
    input: &ControlPaintInput<'_>,
    depth: f64,
    focus_owner: bool,
) -> Result<Option<crate::FocusIndicatorIr>, SurfacePaintResolutionError> {
    let surface = input.surface;
    if !focus_owner
        || !surface
            .states()
            .states()
            .contains(&InteractionState::Focused)
    {
        return Ok(None);
    }
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
        input.size,
        input.appearance.shape_assignments(),
        theme.tokens(),
    )
    .map_err(|e| SurfacePaintResolutionError::Focus(FocusIrError::Shape(e)))?;
    Ok(Some(
        resolve_focus_geometry(
            environment,
            input.size,
            input.appearance.key_light(),
            indicator,
            radii,
            depth,
        )
        .map_err(SurfacePaintResolutionError::Focus)?,
    ))
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

/// Measures every authored hover, pressed and disabled response of the bound family against rest,
/// and the disabled response against pressed, on the opaque body the responses are applied to,
/// whichever state is being resolved.
pub(crate) fn imperceptible_profile(
    theme: &CompiledTheme,
    appearance: &OpaqueSurfaceAppearance,
    surface: &SurfaceIntent,
    binding: &BoundSurface,
    profile: &CommandAppearance,
) -> Result<Option<ImperceptibleResponse>, SurfacePaintResolutionError> {
    let depth = semantic_depth(theme, appearance, surface)?;
    let response = |phase| {
        profile
            .response_for(binding.material_family(), phase)
            .expect("a bound command family has an authored profile")
    };
    for (phase, reference) in [
        (CommandPhase::Hover, CommandPhase::Rest),
        (CommandPhase::Pressed, CommandPhase::Rest),
        (CommandPhase::Disabled, CommandPhase::Rest),
        (CommandPhase::Disabled, CommandPhase::Pressed),
    ] {
        let rest = binding.opaque_color_fallback();
        let [first, second] = [reference, phase].map(response);
        let color_difference = oklab_difference(
            &crate::surface::mix_body_color(rest, first),
            &crate::surface::mix_body_color(rest, second),
        );
        let depth_change = (depth * second.depth_scale() - depth * first.depth_scale()).abs();
        if color_difference < MINIMUM_STATE_COLOR_DIFFERENCE
            && depth_change < MINIMUM_STATE_DEPTH_CHANGE - DEPTH_TOLERANCE
        {
            return Ok(Some(ImperceptibleResponse {
                phase,
                reference,
                color_difference,
                depth_change,
            }));
        }
    }
    Ok(None)
}
