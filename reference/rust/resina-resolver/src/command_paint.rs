use crate::{
    CompiledTheme, OpaqueSurfaceError, SurfacePaintInput, SurfacePaintIr,
    SurfacePaintResolutionError, bind_surface,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    CommandAppearance, CommandPhase, CommandResponse, MaterialFamily, MaterialRole,
    resolve_command_phase,
};
use serde::{Deserialize, Serialize};
use std::fmt;

pub struct CommandPaintInput<'a> {
    pub surface: SurfacePaintInput<'a>,
    pub command_appearance: &'a CommandAppearance,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandPaintIr {
    schema_version: &'static str,
    phase: CommandPhase,
    response: CommandResponse,
    paint: SurfacePaintIr,
}
impl CommandPaintIr {
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
pub enum CommandPaintError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    Scope(&'static str),
    Paint(SurfacePaintResolutionError),
    Motion(resina_motion::SpringError),
}
impl fmt::Display for CommandPaintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "command paint parse failed: {e}"),
            Self::Request(e) => write!(f, "invalid command paint request: {e}"),
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
            Self::Scope(e) => f.write_str(e),
            Self::Paint(e) => write!(f, "command {e}"),
            Self::Motion(e) => write!(f, "command motion: {e}"),
        }
    }
}
impl std::error::Error for CommandPaintError {}
impl From<SurfacePaintResolutionError> for CommandPaintError {
    fn from(error: SurfacePaintResolutionError) -> Self {
        Self::Paint(error)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    surface: crate::surface_paint::SurfacePaintRequest,
    command_appearance: CommandAppearance,
}

pub fn resolve_command_paint_source(source: &str) -> Result<CommandPaintIr, CommandPaintError> {
    let document = resina_tokens::parse_token_document(source).map_err(CommandPaintError::Parse)?;
    let request: Request = serde_json::from_value(document).map_err(CommandPaintError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(CommandPaintError::UnsupportedVersion);
    }
    let (theme, environment, owned) =
        crate::surface_paint::parse_surface_paint_request(request.surface)?;
    resolve_command_paint(
        &theme,
        &environment,
        CommandPaintInput {
            surface: owned.input(),
            command_appearance: &request.command_appearance,
        },
    )
}

pub fn resolve_command_paint(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    input: CommandPaintInput<'_>,
) -> Result<CommandPaintIr, CommandPaintError> {
    resolve_command_paint_with_response(theme, environment, input, |_, response| Ok((response, ())))
        .map(|(paint, ())| paint)
}

pub(crate) fn resolve_command_paint_with_response<T>(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    input: CommandPaintInput<'_>,
    sample: impl FnOnce(
        MaterialFamily,
        CommandResponse,
    ) -> Result<(CommandResponse, T), CommandPaintError>,
) -> Result<(CommandPaintIr, T), CommandPaintError> {
    let surface = input.surface.body.surface;
    if !matches!(
        surface.material_role(),
        MaterialRole::ControlPassive
            | MaterialRole::ControlInteractive
            | MaterialRole::ControlPrimary
    ) {
        return Err(CommandPaintError::Scope(
            "command paint requires a control material role",
        ));
    }
    let phase = resolve_command_phase(surface.states()).map_err(CommandPaintError::Scope)?;
    if input
        .surface
        .surrounding_color
        .is_some_and(|color| color.alpha() != 1.0)
    {
        return Err(SurfacePaintResolutionError::TranslucentSurroundingColor.into());
    }
    let snapshot = theme.resolve(environment).map_err(|error| {
        SurfacePaintResolutionError::Theme(crate::ThemeResolutionError::Resolve(error))
    })?;
    let binding = bind_surface(surface, &snapshot).map_err(|error| {
        SurfacePaintResolutionError::Body(OpaqueSurfaceError::Readability(
            crate::SurfaceReadabilityError::Binding(error),
        ))
    })?;
    let response = input
        .command_appearance
        .response_for(binding.material_family(), phase)
        .map_err(CommandPaintError::Scope)?;
    let (response, sampled) = sample(binding.material_family(), response)?;
    let paint = crate::control_paint::resolve_control_paint(
        theme,
        environment,
        &snapshot,
        input.surface,
        binding,
        response,
        true,
    )?;
    Ok((
        CommandPaintIr {
            schema_version: "0.1.0",
            phase,
            response,
            paint,
        },
        sampled,
    ))
}
