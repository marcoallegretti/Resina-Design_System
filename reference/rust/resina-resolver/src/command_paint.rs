use crate::{
    CompiledTheme, ImperceptibleResponse, OpaqueSurfaceError, OpaqueSurfaceIr, SurfacePaintInput,
    SurfacePaintIr, SurfacePaintResolutionError, bind_surface,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    CommandAppearance, CommandPhase, CommandResponse, MaterialFamily, MaterialRole,
    resolve_command_phase,
};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
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
    Imperceptible(ImperceptibleResponse),
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
            Self::Imperceptible(e) => write!(f, "command {e}"),
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

struct Request {
    schema_version: String,
    surface: crate::surface_paint::SurfacePaintRequest,
    command_appearance: CommandAppearance,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequestMembers {
    schema_version: String,
    surface: crate::surface_paint::SurfacePaintRequest,
    command_appearance: CommandAppearance,
}

impl<'de> Deserialize<'de> for Request {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RequestVisitor;
        impl<'de> Visitor<'de> for RequestVisitor {
            type Value = Request;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a command paint request object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let input = RequestMembers::deserialize(MapAccessDeserializer::new(map))?;
                Ok(Request {
                    schema_version: input.schema_version,
                    surface: input.surface,
                    command_appearance: input.command_appearance,
                })
            }
        }
        deserializer.deserialize_map(RequestVisitor)
    }
}

pub fn resolve_command_paint_source(source: &str) -> Result<CommandPaintIr, CommandPaintError> {
    resolve_source(source, resolve_command_paint)
}

/// Source form of [`resolve_command_reservation_body`], reading one command paint request.
pub fn resolve_command_reservation_body_source(
    source: &str,
) -> Result<OpaqueSurfaceIr, CommandPaintError> {
    resolve_source(source, resolve_command_reservation_body)
}

fn resolve_source<T>(
    source: &str,
    resolve: impl FnOnce(
        &CompiledTheme,
        &EnvironmentSnapshot,
        CommandPaintInput<'_>,
    ) -> Result<T, CommandPaintError>,
) -> Result<T, CommandPaintError> {
    let document = resina_tokens::parse_token_document(source).map_err(CommandPaintError::Parse)?;
    let request: Request = serde_json::from_value(document).map_err(CommandPaintError::Request)?;
    crate::pigment_source::validate_pigment_source(
        source,
        &["surface", "body", "appearance", "pigmentProfiles"],
    )
    .map_err(CommandPaintError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(CommandPaintError::UnsupportedVersion);
    }
    let (theme, environment, owned) =
        crate::surface_paint::parse_surface_paint_request(request.surface)?;
    resolve(
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

/// Resolves the deepest body any static response or projected motion sample can paint for this
/// command: its surface at the projected depth bound. Layout reserves the target from this
/// body's silhouette.
pub fn resolve_command_reservation_body(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    input: CommandPaintInput<'_>,
) -> Result<OpaqueSurfaceIr, CommandPaintError> {
    let deepest = CommandResponse::try_new(0.0, CommandResponse::MAX_DEPTH_SCALE)
        .expect("the projected depth bound is a valid response");
    resolve_command_paint_with_response(theme, environment, input, |_, _| Ok((deepest, ())))
        .map(|(paint, ())| paint.paint.body().clone())
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
    if let Some(imperceptible) = crate::control_paint::imperceptible_profile(
        theme,
        input.surface.body.appearance,
        surface,
        &binding,
        input.command_appearance,
    )? {
        return Err(CommandPaintError::Imperceptible(imperceptible));
    }
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
