use crate::{
    CompiledTheme, OpaqueSurfaceError, OpaqueSurfaceInput, SurfacePaintInput, SurfacePaintIr,
    SurfacePaintResolutionError, bind_surface,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorRole, CommandAppearance, CommandPhase, CommandResponse, InteractionState, MaterialFamily,
    MaterialRole,
};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor, value::MapAccessDeserializer, value::StringDeserializer},
};
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
    Motion(resina_motion::SpringError),
}
impl fmt::Display for TogglePartPaintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "toggle part paint parse failed: {e}"),
            Self::Request(e) => write!(f, "invalid toggle part paint request: {e}"),
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
            Self::Scope(e) => f.write_str(e),
            Self::Paint(e) => write!(f, "toggle part {e}"),
            Self::Motion(e) => write!(f, "toggle part motion: {e}"),
        }
    }
}
impl std::error::Error for TogglePartPaintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(e) | Self::Request(e) => Some(e),
            Self::Paint(e) => Some(e),
            Self::Motion(e) => Some(e),
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
    resolve_toggle_part_paint_with_response(theme, environment, input, |_, response| {
        Ok((response, ()))
    })
    .map(|(paint, ())| paint)
}

pub(crate) fn resolve_toggle_part_paint_with_response<T>(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    input: TogglePartPaintInput<'_>,
    sample: impl FnOnce(
        MaterialFamily,
        CommandResponse,
    ) -> Result<(CommandResponse, T), TogglePartPaintError>,
) -> Result<(TogglePartPaintIr, T), TogglePartPaintError> {
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
    let (response, sampled) = sample(binding.material_family(), response)?;
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
    Ok((
        TogglePartPaintIr {
            schema_version: "0.1.0",
            part: input.part,
            checked,
            phase,
            response,
            paint,
        },
        sampled,
    ))
}

struct Request {
    schema_version: String,
    part: TogglePart,
    surface: crate::surface_paint::SurfacePaintRequest,
    checked_color_role: ColorRole,
    interaction_appearance: CommandAppearance,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequestMembers {
    schema_version: String,
    #[serde(deserialize_with = "toggle_part_name")]
    part: TogglePart,
    surface: crate::surface_paint::SurfacePaintRequest,
    #[serde(deserialize_with = "checked_color_role_name")]
    checked_color_role: ColorRole,
    interaction_appearance: CommandAppearance,
}

impl<'de> Deserialize<'de> for Request {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RequestVisitor;
        impl<'de> Visitor<'de> for RequestVisitor {
            type Value = Request;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a toggle part paint request object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let input = RequestMembers::deserialize(MapAccessDeserializer::new(map))?;
                Ok(Request {
                    schema_version: input.schema_version,
                    part: input.part,
                    surface: input.surface,
                    checked_color_role: input.checked_color_role,
                    interaction_appearance: input.interaction_appearance,
                })
            }
        }
        deserializer.deserialize_map(RequestVisitor)
    }
}

pub(crate) fn toggle_part_name<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<TogglePart, D::Error> {
    TogglePart::deserialize(StringDeserializer::<D::Error>::new(String::deserialize(
        deserializer,
    )?))
}

pub(crate) fn checked_color_role_name<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<ColorRole, D::Error> {
    ColorRole::deserialize(StringDeserializer::<D::Error>::new(String::deserialize(
        deserializer,
    )?))
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

#[cfg(test)]
mod request_tests {
    use super::*;
    use serde_json::{Value, json};

    fn baseline() -> Value {
        serde_json::from_str(include_str!(
            "../../../../conformance/ir/toggle-part-paint-request.json"
        ))
        .unwrap()
    }

    #[test]
    fn known_toggle_names_decode_from_canonical_and_escaped_strings() {
        let schema: Value = serde_json::from_str(include_str!(
            "../../../../schemas/surface-binding.schema.json"
        ))
        .unwrap();
        for role in schema["properties"]["colorRole"]["enum"]
            .as_array()
            .unwrap()
        {
            let mut request = baseline();
            request["checkedColorRole"] = role.clone();
            let expected: ColorRole = serde_json::from_value(role.clone()).unwrap();
            assert_eq!(
                serde_json::from_value::<Request>(request.clone())
                    .unwrap()
                    .checked_color_role,
                expected
            );
            let name = role.as_str().unwrap();
            let source = request.to_string().replace(
                &format!("\"checkedColorRole\":{role}"),
                &format!(
                    "\"\\u0063heckedColorRole\":\"\\u{:04x}{}\"",
                    name.as_bytes()[0],
                    &name[1..]
                ),
            );
            assert_eq!(
                serde_json::from_str::<Request>(&source)
                    .unwrap()
                    .checked_color_role,
                expected
            );
            request["checkedColorRole"] = json!({name:null});
            assert!(serde_json::from_value::<Request>(request).is_err());
        }
        for (name, expected) in [("track", TogglePart::Track), ("thumb", TogglePart::Thumb)] {
            let mut request = baseline();
            request["part"] = json!(name);
            assert_eq!(
                serde_json::from_value::<Request>(request.clone())
                    .unwrap()
                    .part,
                expected
            );
            let source = request.to_string().replace(
                &format!("\"part\":\"{name}\""),
                &format!(
                    "\"\\u0070art\":\"\\u{:04x}{}\"",
                    name.as_bytes()[0],
                    &name[1..]
                ),
            );
            assert_eq!(
                serde_json::from_str::<Request>(&source).unwrap().part,
                expected
            );
            request["part"] = json!({name:null});
            assert!(serde_json::from_value::<Request>(request).is_err());
        }
        for member in ["part", "checkedColorRole"] {
            for invalid in [
                Value::Null,
                json!(true),
                json!(1),
                json!([]),
                json!("unknown"),
            ] {
                let mut request = baseline();
                request[member] = invalid;
                assert!(serde_json::from_value::<Request>(request).is_err());
            }
        }
    }
}
