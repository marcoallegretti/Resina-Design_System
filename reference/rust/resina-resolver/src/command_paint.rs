use crate::{
    CompiledTheme, ControlRelief, ImperceptibleResponse, LitControlBodyError, LitControlBodyIr,
    LitControlPaintIr, OpaqueSurfaceError, SurfacePaintInput, SurfacePaintResolutionError,
    bind_surface, control_paint::LitControlPaintError,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    CommandAppearance, CommandPhase, CommandResponse, MaterialFamily, MaterialLightChannels,
    MaterialRole, resolve_command_phase,
};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use std::fmt;

pub struct CommandPaintInput<'a> {
    pub surface: SurfacePaintInput<'a>,
    pub command_appearance: &'a CommandAppearance,
    pub light: &'a MaterialLightChannels,
    pub relief: &'a ControlRelief,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandPaintIr {
    schema_version: &'static str,
    phase: CommandPhase,
    response: CommandResponse,
    paint: LitControlPaintIr,
}
impl CommandPaintIr {
    pub fn phase(&self) -> CommandPhase {
        self.phase
    }
    pub fn response(&self) -> CommandResponse {
        self.response
    }
    pub fn paint(&self) -> &LitControlPaintIr {
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
    Body(LitControlBodyError),
    Imperceptible(ImperceptibleResponse),
    Motion(resina_motion::SpringError),
}
impl fmt::Display for CommandPaintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "command paint parse failed: {e}"),
            Self::Request(e) => write!(f, "invalid command paint request: {e}"),
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.2.0"),
            Self::Scope(e) => f.write_str(e),
            Self::Paint(e) => write!(f, "command {e}"),
            Self::Body(e) => write!(f, "command {e}"),
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
impl From<LitControlPaintError> for CommandPaintError {
    fn from(error: LitControlPaintError) -> Self {
        match error {
            LitControlPaintError::Paint(error) => Self::Paint(error),
            LitControlPaintError::Body(error) => Self::Body(error),
        }
    }
}

struct Request {
    schema_version: String,
    surface: crate::surface_paint::SurfacePaintRequest,
    command_appearance: CommandAppearance,
    light: MaterialLightChannels,
    relief: ControlRelief,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequestMembers {
    schema_version: String,
    surface: crate::surface_paint::SurfacePaintRequest,
    command_appearance: CommandAppearance,
    light: MaterialLightChannels,
    relief: ControlRelief,
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
                    light: input.light,
                    relief: input.relief,
                })
            }
        }
        deserializer.deserialize_map(RequestVisitor)
    }
}

/// Validates the original pigment and light channel coefficient sources of a command request.
pub(crate) fn validate_command_sources(source: &str) -> Result<(), CommandPaintError> {
    crate::pigment_source::validate_pigment_source(
        source,
        &["surface", "body", "appearance", "pigmentProfiles"],
    )
    .map_err(CommandPaintError::Request)?;
    crate::pigment_source::validate_light_channel_source(source, &["light"])
        .map_err(CommandPaintError::Request)
}

pub fn resolve_command_paint_source(source: &str) -> Result<CommandPaintIr, CommandPaintError> {
    resolve_source(source, resolve_command_paint)
}

/// Source form of [`resolve_command_reservation_body`], reading one command paint request.
pub fn resolve_command_reservation_body_source(
    source: &str,
) -> Result<LitControlBodyIr, CommandPaintError> {
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
    validate_command_sources(source)?;
    if request.schema_version != "0.2.0" {
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
            light: &request.light,
            relief: &request.relief,
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
/// command: its lit body at the projected depth bound. Layout reserves the target from this
/// body's silhouette.
pub fn resolve_command_reservation_body(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    input: CommandPaintInput<'_>,
) -> Result<LitControlBodyIr, CommandPaintError> {
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
    let mut paint = crate::control_paint::resolve_lit_control_paint(
        theme,
        environment,
        &snapshot,
        input.surface,
        binding,
        response,
        input.light,
        ("commandBody", input.relief.command_body()),
        true,
    )?;
    if phase == CommandPhase::Disabled {
        paint
            .body_mut()
            .dim_disabled_label(crate::disabled_label::disabled_label_bound(environment));
    }
    Ok((
        CommandPaintIr {
            schema_version: "0.2.0",
            phase,
            response,
            paint,
        },
        sampled,
    ))
}
