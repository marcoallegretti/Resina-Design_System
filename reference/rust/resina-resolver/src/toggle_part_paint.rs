use crate::{
    CompiledTheme, OpaqueSurfaceError, OpaqueSurfaceInput, SurfacePaintInput, SurfacePaintIr,
    SurfacePaintResolutionError, bind_surface,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorRole, CommandAppearance, CommandPhase, CommandResponse, InteractionState, MaterialRole,
};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TogglePart {
    Track,
    Thumb,
}

pub struct TogglePartPaintInput<'a> {
    pub part: TogglePart,
    pub surface: SurfacePaintInput<'a>,
    pub checked_color_role: ColorRole,
    pub interaction_appearance: &'a CommandAppearance,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TogglePartPaintIr {
    schema_version: &'static str,
    part: TogglePart,
    checked: bool,
    phase: CommandPhase,
    response: CommandResponse,
    paint: SurfacePaintIr,
}
impl TogglePartPaintIr {
    pub fn part(&self) -> TogglePart {
        self.part
    }
    pub fn checked(&self) -> bool {
        self.checked
    }
    pub fn phase(&self) -> CommandPhase {
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
pub enum TogglePartPaintError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    Scope(&'static str),
    Paint(SurfacePaintResolutionError),
}
impl fmt::Display for TogglePartPaintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "toggle part paint parse failed: {e}"),
            Self::Request(e) => write!(f, "invalid toggle part paint request: {e}"),
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
            Self::Scope(e) => f.write_str(e),
            Self::Paint(e) => write!(f, "toggle part {e}"),
        }
    }
}
impl std::error::Error for TogglePartPaintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(e) | Self::Request(e) => Some(e),
            Self::Paint(e) => Some(e),
            _ => None,
        }
    }
}
impl From<SurfacePaintResolutionError> for TogglePartPaintError {
    fn from(e: SurfacePaintResolutionError) -> Self {
        Self::Paint(e)
    }
}

pub fn resolve_toggle_part_paint(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    input: TogglePartPaintInput<'_>,
) -> Result<TogglePartPaintIr, TogglePartPaintError> {
    let original = input.surface.body.surface;
    if !matches!(
        original.material_role(),
        MaterialRole::ControlPassive
            | MaterialRole::ControlInteractive
            | MaterialRole::ControlPrimary
    ) {
        return Err(TogglePartPaintError::Scope(
            "toggle parts require a persistent control material role",
        ));
    }
    let states = original.states().states();
    if states.iter().any(|state| {
        !matches!(
            state,
            InteractionState::Rest
                | InteractionState::Hover
                | InteractionState::Pressed
                | InteractionState::Checked
                | InteractionState::Focused
                | InteractionState::Disabled
        )
    }) {
        return Err(TogglePartPaintError::Scope(
            "toggle part paint supports only rest, hover, pressed, checked, disabled and focused states",
        ));
    }
    let phase = if states.contains(&InteractionState::Disabled) {
        CommandPhase::Disabled
    } else if states.contains(&InteractionState::Pressed) {
        CommandPhase::Pressed
    } else if states.contains(&InteractionState::Hover) {
        CommandPhase::Hover
    } else {
        CommandPhase::Rest
    };
    let checked = states.contains(&InteractionState::Checked);
    if input
        .surface
        .surrounding_color
        .is_some_and(|color| color.alpha() != 1.0)
    {
        return Err(SurfacePaintResolutionError::TranslucentSurroundingColor.into());
    }
    let selected = if checked {
        original.clone().with_color_role(input.checked_color_role)
    } else {
        original.clone()
    };
    let snapshot = theme.resolve(environment).map_err(|error| {
        SurfacePaintResolutionError::Theme(crate::ThemeResolutionError::Resolve(error))
    })?;
    let binding = bind_surface(&selected, &snapshot).map_err(|e| {
        SurfacePaintResolutionError::Body(OpaqueSurfaceError::Readability(
            crate::SurfaceReadabilityError::Binding(e),
        ))
    })?;
    let response = input
        .interaction_appearance
        .response_for(binding.material_family(), phase)
        .map_err(TogglePartPaintError::Scope)?;
    let b = input.surface.body;
    let paint = crate::control_paint::resolve_control_paint(
        theme,
        environment,
        &snapshot,
        SurfacePaintInput {
            body: OpaqueSurfaceInput {
                surface: &selected,
                size: b.size,
                appearance: b.appearance,
                foreground_role: b.foreground_role,
                post_treatment_backdrop: b.post_treatment_backdrop,
                adjacent_color: b.adjacent_color,
                minimum_content_contrast: b.minimum_content_contrast,
                minimum_edge_contrast: b.minimum_edge_contrast,
            },
            surrounding_color: input.surface.surrounding_color,
        },
        binding,
        response,
        input.part == TogglePart::Track,
    )?;
    Ok(TogglePartPaintIr {
        schema_version: "0.1.0",
        part: input.part,
        checked,
        phase,
        response,
        paint,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    part: TogglePart,
    surface: crate::surface_paint::SurfacePaintRequest,
    checked_color_role: ColorRole,
    interaction_appearance: CommandAppearance,
}
pub fn resolve_toggle_part_paint_source(
    source: &str,
) -> Result<TogglePartPaintIr, TogglePartPaintError> {
    let document =
        resina_tokens::parse_token_document(source).map_err(TogglePartPaintError::Parse)?;
    let request: Request =
        serde_json::from_value(document).map_err(TogglePartPaintError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(TogglePartPaintError::UnsupportedVersion);
    }
    let (theme, environment, owned) =
        crate::surface_paint::parse_surface_paint_request(request.surface)?;
    resolve_toggle_part_paint(
        &theme,
        &environment,
        TogglePartPaintInput {
            part: request.part,
            surface: owned.input(),
            checked_color_role: request.checked_color_role,
            interaction_appearance: &request.interaction_appearance,
        },
    )
}
