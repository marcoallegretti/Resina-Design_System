use crate::{
    CompiledTheme, OpaqueSurfaceError, SrgbFallback, SurfacePaintIr, SurfacePaintResolutionError,
    SurfaceReadabilityError, ThemeResolutionError,
    background_contrast::EdgeBackground,
    bind_surface,
    control_paint::{ControlPaintInput, resolve_control_paint_inputs},
};
use resina_color::OpaqueSrgbRange;
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorRole, CommandResponse, InteractionState, MaterialRole, OpaqueSurfaceAppearance,
    SliderAppearance, SliderPart, SliderPhase, SurfaceIntent, SurfaceSize, resolve_slider_phase,
};
use serde::Serialize;
use std::fmt;

pub struct SliderPartPaintInput<'a> {
    pub part: SliderPart,
    pub read_only: bool,
    pub surface: &'a SurfaceIntent,
    pub size: SurfaceSize,
    pub appearance: &'a OpaqueSurfaceAppearance,
    pub interaction_appearance: &'a SliderAppearance,
    pub foreground_role: ColorRole,
    pub post_treatment_backdrop: Option<&'a SrgbFallback>,
    pub adjacent_ranges: &'a [OpaqueSrgbRange],
    pub surrounding_ranges: Option<&'a [OpaqueSrgbRange]>,
    pub minimum_content_contrast: f64,
    pub minimum_edge_contrast: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliderPartPaintIr {
    schema_version: &'static str,
    part: SliderPart,
    read_only: bool,
    phase: SliderPhase,
    response: CommandResponse,
    paint: SurfacePaintIr,
}
impl SliderPartPaintIr {
    pub fn part(&self) -> SliderPart {
        self.part
    }
    pub fn read_only(&self) -> bool {
        self.read_only
    }
    pub fn phase(&self) -> SliderPhase {
        self.phase
    }
    pub fn response(&self) -> CommandResponse {
        self.response
    }
    pub fn paint(&self) -> &SurfacePaintIr {
        &self.paint
    }
}

#[derive(Debug)]
pub enum SliderPartPaintError {
    Scope(&'static str),
    Paint(SurfacePaintResolutionError),
}
impl fmt::Display for SliderPartPaintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scope(error) => f.write_str(error),
            Self::Paint(error) => write!(f, "slider part {error}"),
        }
    }
}
impl std::error::Error for SliderPartPaintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Paint(error) => Some(error),
            Self::Scope(_) => None,
        }
    }
}

pub fn resolve_slider_part_paint(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    input: SliderPartPaintInput<'_>,
) -> Result<SliderPartPaintIr, SliderPartPaintError> {
    if !matches!(
        input.surface.material_role(),
        MaterialRole::ControlPassive
            | MaterialRole::ControlInteractive
            | MaterialRole::ControlPrimary
    ) {
        return Err(SliderPartPaintError::Scope(
            "slider parts require a persistent control material role",
        ));
    }
    let phase = resolve_slider_phase(input.surface.states(), input.read_only)
        .map_err(SliderPartPaintError::Scope)?;
    let focus_owner = input.part == SliderPart::Thumb;
    if input
        .surrounding_ranges
        .is_some_and(|ranges| ranges.is_empty())
    {
        return Err(SliderPartPaintError::Scope(
            "surrounding ranges must not be empty",
        ));
    }
    if focus_owner
        && input.surface.states().contains(InteractionState::Focused)
        && input.surrounding_ranges.is_none()
    {
        return Err(SliderPartPaintError::Scope(
            "focused slider thumb requires surrounding ranges",
        ));
    }
    let snapshot = theme.resolve(environment).map_err(|error| {
        SliderPartPaintError::Paint(SurfacePaintResolutionError::Theme(
            ThemeResolutionError::Resolve(error),
        ))
    })?;
    let binding = bind_surface(input.surface, &snapshot).map_err(|error| {
        SliderPartPaintError::Paint(SurfacePaintResolutionError::Body(
            OpaqueSurfaceError::Readability(SurfaceReadabilityError::Binding(error)),
        ))
    })?;
    let response = input
        .interaction_appearance
        .response_for(input.part, binding.material_family(), phase)
        .map_err(SliderPartPaintError::Scope)?;
    let paint = resolve_control_paint_inputs(
        theme,
        environment,
        &snapshot,
        ControlPaintInput {
            surface: input.surface,
            size: input.size,
            appearance: input.appearance,
            foreground_role: input.foreground_role,
            post_treatment_backdrop: input.post_treatment_backdrop,
            adjacent: EdgeBackground::Ranges(input.adjacent_ranges),
            surrounding: input.surrounding_ranges.map(EdgeBackground::Ranges),
            minimum_content_contrast: input.minimum_content_contrast,
            minimum_edge_contrast: input.minimum_edge_contrast,
        },
        binding,
        response,
        focus_owner,
    )
    .map_err(SliderPartPaintError::Paint)?;
    Ok(SliderPartPaintIr {
        schema_version: "0.1.0",
        part: input.part,
        read_only: input.read_only,
        phase,
        response,
        paint,
    })
}
