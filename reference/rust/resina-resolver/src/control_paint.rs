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
    ColorRole, CommandResponse, InteractionState, OpaqueSurfaceAppearance, SurfaceIntent,
    SurfaceSize,
};

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
    let depths = resolve_elevation_depth(appearance.depth_assignments(), theme.tokens())
        .map_err(|error| SurfacePaintResolutionError::Body(OpaqueSurfaceError::Depth(error)))?;
    let depth = depths[&surface.form().elevation()]["value"]
        .as_f64()
        .expect("validated depth is numeric")
        * response.depth_scale();
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
