use crate::{
    BoundSurface, CompiledTheme, FocusIrError, FocusIrInput, HeadlessResolution,
    OpaqueSurfaceError, SurfacePaintInput, SurfacePaintIr, SurfacePaintResolutionError,
    focus_ir::resolve_focus_geometry, opaque_surface::resolve_opaque_body_geometry,
    resolve_elevation_depth, resolve_focus_indicator, resolve_shape_fallback,
    surface_readability::resolve_bound_body_readability,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{CommandResponse, InteractionState};

pub(crate) fn resolve_control_paint(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    snapshot: &HeadlessResolution,
    input: SurfacePaintInput<'_>,
    mut binding: BoundSurface,
    response: CommandResponse,
    focus_owner: bool,
) -> Result<SurfacePaintIr, SurfacePaintResolutionError> {
    let surface = input.body.surface;
    binding.apply_command_response(response);
    let readable = resolve_bound_body_readability(
        binding,
        snapshot,
        input.body.foreground_role,
        input.body.post_treatment_backdrop,
        crate::background_contrast::EdgeBackground::Uniform(input.body.adjacent_color),
        input.body.minimum_content_contrast,
        input.body.minimum_edge_contrast,
    )
    .map_err(|error| SurfacePaintResolutionError::Body(OpaqueSurfaceError::Readability(error)))?;
    let appearance = input.body.appearance;
    let depths = resolve_elevation_depth(appearance.depth_assignments(), theme.tokens())
        .map_err(|error| SurfacePaintResolutionError::Body(OpaqueSurfaceError::Depth(error)))?;
    let depth = depths[&surface.form().elevation()]["value"]
        .as_f64()
        .expect("validated depth is numeric")
        * response.depth_scale();
    let size = input.body.size;
    let body = resolve_opaque_body_geometry(theme, environment, input.body, &readable, depth)
        .map_err(SurfacePaintResolutionError::Body)?;
    let focus = if focus_owner
        && surface
            .states()
            .states()
            .contains(&InteractionState::Focused)
    {
        let surrounding_color = input
            .surrounding_color
            .ok_or(SurfacePaintResolutionError::MissingSurroundingColor)?;
        let indicator = resolve_focus_indicator(surface, snapshot, surrounding_color)
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
                FocusIrInput {
                    surface,
                    size,
                    shape_assignments: appearance.shape_assignments(),
                    depth_assignments: appearance.depth_assignments(),
                    key_light: appearance.key_light(),
                    surrounding_color,
                },
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
