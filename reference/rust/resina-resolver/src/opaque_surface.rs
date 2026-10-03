use crate::{
    EdgeContrastResult, ExtrudedContourError, ExtrudedContourResult, InsetContourError,
    KeyLightError, KeyLightResult, OpaquePigmentResult, ShapeFallbackError, SrgbFallback,
    SurfaceReadabilityError, ThemeResolutionError, resolve_elevation_depth,
    resolve_extruded_contour, resolve_inset_contour, resolve_key_light, resolve_opaque_pigment,
    resolve_shape_fallback, resolve_surface_readability,
    srgb_input::{SrgbInput, SrgbInputError},
    theme_request::compile_theme_request_document,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorRole, FrostRepresentation, MaterialFamily, MaterialRole, OpaqueSurfaceAppearance,
    PhysicalVector, StateSet, SurfaceForm, SurfaceIntent, SurfaceSize,
};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::fmt;

pub struct OpaqueSurfaceInput<'a> {
    pub surface: &'a SurfaceIntent,
    pub size: SurfaceSize,
    pub appearance: &'a OpaqueSurfaceAppearance,
    pub foreground_role: ColorRole,
    pub post_treatment_backdrop: Option<&'a SrgbFallback>,
    pub adjacent_color: &'a SrgbFallback,
    pub minimum_content_contrast: f64,
    pub minimum_edge_contrast: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpaqueSurfaceRequest {
    schema_version: String,
    theme: Value,
    surface: SurfaceIntent,
    size: SurfaceSize,
    appearance: OpaqueSurfaceAppearance,
    foreground_role: ColorRole,
    #[serde(default, deserialize_with = "present_backdrop")]
    post_treatment_backdrop: Option<SrgbInput>,
    adjacent_color: SrgbInput,
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
}

fn present_backdrop<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<SrgbInput>, D::Error> {
    SrgbInput::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacedContour {
    offset: PhysicalVector,
    contour: ExtrudedContourResult,
}
impl PlacedContour {
    pub fn offset(&self) -> PhysicalVector {
        self.offset
    }
    pub fn contour(&self) -> &ExtrudedContourResult {
        &self.contour
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpaqueSurfaceGeometry {
    front: ExtrudedContourResult,
    silhouette: ExtrudedContourResult,
    edge_interior: PlacedContour,
    highlight_outer: PlacedContour,
    content: PlacedContour,
}
impl OpaqueSurfaceGeometry {
    pub fn front(&self) -> &ExtrudedContourResult {
        &self.front
    }
    pub fn silhouette(&self) -> &ExtrudedContourResult {
        &self.silhouette
    }
    pub fn edge_interior(&self) -> &PlacedContour {
        &self.edge_interior
    }
    pub fn highlight_outer(&self) -> &PlacedContour {
        &self.highlight_outer
    }
    pub fn content(&self) -> &PlacedContour {
        &self.content
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpaqueSurfaceIr {
    schema_version: &'static str,
    representation: &'static str,
    material_role: MaterialRole,
    color_role: ColorRole,
    material_family: MaterialFamily,
    form: SurfaceForm,
    states: StateSet,
    #[serde(skip_serializing_if = "Option::is_none")]
    frost_representation: Option<FrostRepresentation>,
    geometry: OpaqueSurfaceGeometry,
    pigment: OpaquePigmentResult,
    lighting: KeyLightResult,
    edge_width: f64,
    highlight_width: f64,
    edge: EdgeContrastResult,
    foreground_role: ColorRole,
    foreground: SrgbFallback,
    content_contrast_ratio: f64,
    content_fallback_applied: bool,
}
impl OpaqueSurfaceIr {
    pub fn geometry(&self) -> &OpaqueSurfaceGeometry {
        &self.geometry
    }
    pub fn pigment(&self) -> &OpaquePigmentResult {
        &self.pigment
    }
    pub fn lighting(&self) -> &KeyLightResult {
        &self.lighting
    }
    pub fn edge(&self) -> &EdgeContrastResult {
        &self.edge
    }
    pub fn foreground(&self) -> &SrgbFallback {
        &self.foreground
    }
    pub fn material_family(&self) -> MaterialFamily {
        self.material_family
    }
    pub fn material_role(&self) -> MaterialRole {
        self.material_role
    }
    pub fn color_role(&self) -> ColorRole {
        self.color_role
    }
    pub fn form(&self) -> &SurfaceForm {
        &self.form
    }
    pub fn states(&self) -> &StateSet {
        &self.states
    }
    pub fn frost_representation(&self) -> Option<FrostRepresentation> {
        self.frost_representation
    }
    pub fn edge_width(&self) -> f64 {
        self.edge_width
    }
    pub fn highlight_width(&self) -> f64 {
        self.highlight_width
    }
    pub fn foreground_role(&self) -> ColorRole {
        self.foreground_role
    }
    pub fn content_contrast_ratio(&self) -> f64 {
        self.content_contrast_ratio
    }
    pub fn content_fallback_applied(&self) -> bool {
        self.content_fallback_applied
    }
}

#[derive(Debug)]
pub enum OpaqueSurfaceError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    Theme(ThemeResolutionError),
    Readability(SurfaceReadabilityError),
    Shape(ShapeFallbackError),
    Depth(Vec<crate::ElevationDepthResolutionError>),
    InvalidColorSpace(&'static str),
    Color(&'static str, resina_color::ColorFallbackError),
    TranslucentBody,
    Inset(InsetContourError),
    Contour(ExtrudedContourError),
    Light(KeyLightError),
    InvalidBandExtent,
    InsufficientContentArea,
    UnrepresentableBand,
}
impl fmt::Display for OpaqueSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(formatter, "opaque surface parse failed: {e}"),
            Self::Request(e) => write!(formatter, "invalid opaque surface request: {e}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::Theme(e) => write!(formatter, "opaque surface theme: {e}"),
            Self::Readability(e) => write!(formatter, "opaque surface readability: {e}"),
            Self::Shape(e) => write!(formatter, "opaque surface shape: {e}"),
            Self::Depth(errors) => {
                formatter.write_str("opaque surface depth binding failed")?;
                for e in errors {
                    write!(formatter, "\n  {e}")?;
                }
                Ok(())
            }
            Self::Color(field, e) => write!(formatter, "invalid {field}: {e}"),
            Self::InvalidColorSpace(field) => write!(formatter, "{field} must use sRGB"),
            Self::TranslucentBody => {
                formatter.write_str("opaque surface IR requires a resolved opaque body")
            }
            Self::Inset(e) => write!(formatter, "opaque surface inset: {e}"),
            Self::Contour(e) => write!(formatter, "opaque surface contour: {e}"),
            Self::Light(e) => write!(formatter, "opaque surface light: {e}"),
            Self::InvalidBandExtent => {
                formatter.write_str("combined edge and highlight extent must be finite")
            }
            Self::InsufficientContentArea => formatter.write_str(
                "surface must leave positive content area after edge and highlight bands",
            ),
            Self::UnrepresentableBand => {
                formatter.write_str("surface bands cannot be represented at these numeric bounds")
            }
        }
    }
}
impl std::error::Error for OpaqueSurfaceError {}

pub fn resolve_opaque_surface_source(source: &str) -> Result<OpaqueSurfaceIr, OpaqueSurfaceError> {
    let document = parse_token_document(source).map_err(OpaqueSurfaceError::Parse)?;
    let request: OpaqueSurfaceRequest =
        serde_json::from_value(document).map_err(OpaqueSurfaceError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(OpaqueSurfaceError::UnsupportedVersion);
    }
    let (theme, environment) =
        compile_theme_request_document(request.theme).map_err(OpaqueSurfaceError::Theme)?;
    let parse_color = |field, input: SrgbInput| {
        input.into_fallback().map_err(|error| match error {
            SrgbInputError::InvalidColorSpace => OpaqueSurfaceError::InvalidColorSpace(field),
            SrgbInputError::Color(error) => OpaqueSurfaceError::Color(field, error),
        })
    };
    let backdrop = request
        .post_treatment_backdrop
        .map(|input| parse_color("postTreatmentBackdrop", input))
        .transpose()?;
    let adjacent = parse_color("adjacentColor", request.adjacent_color)?;
    resolve_opaque_surface(
        &theme,
        &environment,
        OpaqueSurfaceInput {
            surface: &request.surface,
            size: request.size,
            appearance: &request.appearance,
            foreground_role: request.foreground_role,
            post_treatment_backdrop: backdrop.as_ref(),
            adjacent_color: &adjacent,
            minimum_content_contrast: request.minimum_content_contrast,
            minimum_edge_contrast: request.minimum_edge_contrast,
        },
    )
}

pub fn resolve_opaque_surface(
    theme: &crate::CompiledTheme,
    environment: &EnvironmentSnapshot,
    input: OpaqueSurfaceInput<'_>,
) -> Result<OpaqueSurfaceIr, OpaqueSurfaceError> {
    let resolution = theme
        .resolve(environment)
        .map_err(|e| OpaqueSurfaceError::Theme(ThemeResolutionError::Resolve(e)))?;
    let readable = resolve_surface_readability(
        input.surface,
        &resolution,
        input.foreground_role,
        input.post_treatment_backdrop,
        input.adjacent_color,
        input.minimum_content_contrast,
        input.minimum_edge_contrast,
    )
    .map_err(OpaqueSurfaceError::Readability)?;
    if readable.body().alpha() != 1.0 {
        return Err(OpaqueSurfaceError::TranslucentBody);
    }
    let binding = readable.binding();
    let family = binding.material_family();
    let bands = input.appearance.bands_for(family);
    let extent = bands.edge_width() + bands.highlight_width();
    if !extent.is_finite() {
        return Err(OpaqueSurfaceError::InvalidBandExtent);
    }
    let radii = resolve_shape_fallback(
        binding.form().shape(),
        input.size,
        input.appearance.shape_assignments(),
        theme.tokens(),
    )
    .map_err(OpaqueSurfaceError::Shape)?;
    let depths = resolve_elevation_depth(input.appearance.depth_assignments(), theme.tokens())
        .map_err(OpaqueSurfaceError::Depth)?;
    let depth = depths[&binding.form().elevation()]["value"]
        .as_f64()
        .expect("validated depth is numeric");
    let lighting = resolve_key_light(input.appearance.key_light(), depth, &[])
        .map_err(OpaqueSurfaceError::Light)?;
    let edge_inset = resolve_inset_contour(input.size, radii, bands.edge_width())
        .map_err(OpaqueSurfaceError::Inset)?;
    let content_inset =
        resolve_inset_contour(input.size, radii, extent).map_err(OpaqueSurfaceError::Inset)?;
    if content_inset.size().width == 0.0 || content_inset.size().height == 0.0 {
        return Err(OpaqueSurfaceError::InsufficientContentArea);
    }
    if edge_inset.size().width >= input.size.width
        || edge_inset.size().height >= input.size.height
        || (bands.highlight_width() > 0.0
            && (content_inset.size().width >= edge_inset.size().width
                || content_inset.size().height >= edge_inset.size().height))
    {
        return Err(OpaqueSurfaceError::UnrepresentableBand);
    }
    let zero = PhysicalVector { x: 0.0, y: 0.0 };
    let contour = |size, radii, offset| {
        resolve_extruded_contour(size, radii, environment.layout_direction(), offset)
            .map_err(OpaqueSurfaceError::Contour)
    };
    let placed =
        |inset: &crate::InsetContourResult, offset| -> Result<PlacedContour, OpaqueSurfaceError> {
            Ok(PlacedContour {
                offset: PhysicalVector {
                    x: inset.inset(),
                    y: inset.inset(),
                },
                contour: contour(inset.size(), inset.radii(), offset)?,
            })
        };
    let geometry = OpaqueSurfaceGeometry {
        front: contour(input.size, radii, zero)?,
        silhouette: contour(input.size, radii, lighting.side_offset())?,
        edge_interior: placed(&edge_inset, lighting.side_offset())?,
        highlight_outer: placed(&edge_inset, zero)?,
        content: placed(&content_inset, zero)?,
    };
    let pigment =
        resolve_opaque_pigment(family, readable.body(), input.appearance.pigment_profiles())
            .expect("readability selected an opaque body");
    Ok(OpaqueSurfaceIr {
        schema_version: "0.1.0",
        representation: "opaque",
        material_role: binding.material_role(),
        color_role: binding.color_role(),
        material_family: family,
        form: binding.form().clone(),
        states: binding.states().clone(),
        frost_representation: readable.frost_representation(),
        geometry,
        pigment,
        lighting,
        edge_width: bands.edge_width(),
        highlight_width: bands.highlight_width(),
        edge: readable.edge().clone(),
        foreground_role: readable.foreground_role(),
        foreground: readable.foreground().clone(),
        content_contrast_ratio: readable.content_contrast_ratio(),
        content_fallback_applied: readable.content_fallback_applied(),
    })
}
