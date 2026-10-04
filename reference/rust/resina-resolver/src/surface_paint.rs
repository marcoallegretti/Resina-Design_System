use crate::{
    ColorFallbackError, CompiledTheme, FocusIndicatorIr, FocusIrError, FocusIrInput,
    OpaqueSurfaceError, OpaqueSurfaceInput, OpaqueSurfaceIr, SrgbFallback, ThemeResolutionError,
    focus_ir::resolve_focus_ir_with_snapshot,
    opaque_surface::{OpaqueSurfaceRequest, resolve_opaque_surface_with_snapshot},
    srgb_input::{SrgbInput, SrgbInputError},
    theme_request::compile_theme_request_document,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::InteractionState;
use resina_tokens::parse_token_document;
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

pub struct SurfacePaintInput<'a> {
    pub body: OpaqueSurfaceInput<'a>,
    pub surrounding_color: Option<&'a SrgbFallback>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfacePaintIr {
    pub(crate) schema_version: &'static str,
    pub(crate) body: OpaqueSurfaceIr,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) focus: Option<FocusIndicatorIr>,
}

impl SurfacePaintIr {
    pub fn body(&self) -> &OpaqueSurfaceIr {
        &self.body
    }
    pub fn focus(&self) -> Option<&FocusIndicatorIr> {
        self.focus.as_ref()
    }
}

#[derive(Debug)]
pub enum SurfacePaintResolutionError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    Theme(ThemeResolutionError),
    Body(OpaqueSurfaceError),
    Focus(FocusIrError),
    InvalidColorSpace(&'static str),
    Color(&'static str, ColorFallbackError),
    MissingSurroundingColor,
    TranslucentSurroundingColor,
}

impl fmt::Display for SurfacePaintResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "surface paint parse failed: {error}"),
            Self::Request(error) => write!(formatter, "invalid surface paint request: {error}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::Theme(error) => write!(formatter, "surface paint theme: {error}"),
            Self::Body(error) => write!(formatter, "surface paint body: {error}"),
            Self::Focus(error) => write!(formatter, "surface paint focus: {error}"),
            Self::InvalidColorSpace(field) => write!(formatter, "{field} must use sRGB"),
            Self::Color(field, error) => write!(formatter, "invalid {field}: {error}"),
            Self::MissingSurroundingColor => {
                formatter.write_str("focused surface requires surroundingColor")
            }
            Self::TranslucentSurroundingColor => {
                formatter.write_str("surroundingColor must be opaque")
            }
        }
    }
}
impl std::error::Error for SurfacePaintResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) | Self::Request(error) => Some(error),
            Self::Theme(error) => Some(error),
            Self::Body(error) => Some(error),
            Self::Focus(error) => Some(error),
            Self::Color(_, error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SurfacePaintRequest {
    schema_version: String,
    body: OpaqueSurfaceRequest,
    #[serde(default, deserialize_with = "present_surrounding")]
    surrounding_color: Option<SrgbInput>,
}
fn present_surrounding<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<SrgbInput>, D::Error> {
    SrgbInput::deserialize(deserializer).map(Some)
}

pub fn resolve_surface_paint_source(
    source: &str,
) -> Result<SurfacePaintIr, SurfacePaintResolutionError> {
    let document = parse_token_document(source).map_err(SurfacePaintResolutionError::Parse)?;
    let request: SurfacePaintRequest =
        serde_json::from_value(document).map_err(SurfacePaintResolutionError::Request)?;
    let (theme, environment, owned) = parse_surface_paint_request(request)?;
    resolve_surface_paint(&theme, &environment, owned.input())
}

pub(crate) struct OwnedPaintInput {
    surface: resina_model::SurfaceIntent,
    size: resina_model::SurfaceSize,
    appearance: resina_model::OpaqueSurfaceAppearance,
    foreground_role: resina_model::ColorRole,
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
    backdrop: Option<SrgbFallback>,
    adjacent: SrgbFallback,
    surrounding: Option<SrgbFallback>,
}
impl OwnedPaintInput {
    pub(crate) fn input(&self) -> SurfacePaintInput<'_> {
        SurfacePaintInput {
            body: OpaqueSurfaceInput {
                surface: &self.surface,
                size: self.size,
                appearance: &self.appearance,
                foreground_role: self.foreground_role,
                post_treatment_backdrop: self.backdrop.as_ref(),
                adjacent_color: &self.adjacent,
                minimum_content_contrast: self.minimum_content_contrast,
                minimum_edge_contrast: self.minimum_edge_contrast,
            },
            surrounding_color: self.surrounding.as_ref(),
        }
    }
}
pub(crate) fn parse_surface_paint_request(
    request: SurfacePaintRequest,
) -> Result<(CompiledTheme, EnvironmentSnapshot, OwnedPaintInput), SurfacePaintResolutionError> {
    if request.schema_version != "0.1.0" || request.body.schema_version != "0.1.0" {
        return Err(SurfacePaintResolutionError::UnsupportedVersion);
    }
    let body = request.body;
    let (theme, environment) =
        compile_theme_request_document(body.theme).map_err(SurfacePaintResolutionError::Theme)?;
    let color = |field, input: SrgbInput| {
        input.into_fallback().map_err(|error| match error {
            SrgbInputError::InvalidColorSpace => {
                SurfacePaintResolutionError::InvalidColorSpace(field)
            }
            SrgbInputError::Color(error) => SurfacePaintResolutionError::Color(field, error),
        })
    };
    let backdrop = body
        .post_treatment_backdrop
        .map(|input| color("postTreatmentBackdrop", input))
        .transpose()?;
    let adjacent = color("adjacentColor", body.adjacent_color)?;
    let surrounding = request
        .surrounding_color
        .map(|input| color("surroundingColor", input))
        .transpose()?;
    Ok((
        theme,
        environment,
        OwnedPaintInput {
            surface: body.surface,
            size: body.size,
            appearance: body.appearance,
            foreground_role: body.foreground_role,
            minimum_content_contrast: body.minimum_content_contrast,
            minimum_edge_contrast: body.minimum_edge_contrast,
            backdrop,
            adjacent,
            surrounding,
        },
    ))
}

pub fn resolve_surface_paint(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    input: SurfacePaintInput<'_>,
) -> Result<SurfacePaintIr, SurfacePaintResolutionError> {
    if input
        .surrounding_color
        .is_some_and(|color| color.alpha() != 1.0)
    {
        return Err(SurfacePaintResolutionError::TranslucentSurroundingColor);
    }
    let snapshot = theme.resolve(environment).map_err(|error| {
        SurfacePaintResolutionError::Theme(ThemeResolutionError::Resolve(error))
    })?;
    let surface = input.body.surface;
    let size = input.body.size;
    let appearance = input.body.appearance;
    let body = resolve_opaque_surface_with_snapshot(theme, environment, &snapshot, input.body)
        .map_err(SurfacePaintResolutionError::Body)?;
    let focus = if surface
        .states()
        .states()
        .contains(&InteractionState::Focused)
    {
        let surrounding_color = input
            .surrounding_color
            .ok_or(SurfacePaintResolutionError::MissingSurroundingColor)?;
        Some(
            resolve_focus_ir_with_snapshot(
                theme,
                environment,
                &snapshot,
                FocusIrInput {
                    surface,
                    size,
                    shape_assignments: appearance.shape_assignments(),
                    depth_assignments: appearance.depth_assignments(),
                    key_light: appearance.key_light(),
                    surrounding_color,
                },
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
