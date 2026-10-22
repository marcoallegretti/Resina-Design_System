use crate::{
    BoundSurface, CompiledTheme, EdgeContrastError, EdgeContrastResult, FocusIndicatorIr,
    FrostSurfaceReadabilityError, KeyLightResult, OpaquePigmentError, OpaqueSurfaceError,
    OpaqueSurfaceGeometry, SrgbFallback, SurfaceBindingError, SurfacePaintIr,
    SurfacePaintResolutionError, SurfaceReadabilityError, ThemeResolutionError,
    background_contrast::EdgeBackground,
    bind_surface,
    control_paint::{ControlPaintInput, resolve_control_focus},
    frost_surface_readability::validate_surface_scope,
    opaque_contrast_ratio,
    opaque_surface::{foreground_role_name, resolve_opaque_body_geometry, resolve_surface_shape},
    resolve_elevation_depth, resolve_opaque_pigment,
    srgb_input::{RangeInput, SrgbInputError},
    surface_readability::resolve_bound_body_readability,
    theme_request::compile_theme_request_document,
};
use resina_color::{
    ContrastRangeError, OpaqueSrgbRange, composite_srgb_over_opaque, opaque_contrast_over_range,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorRole, FrostRepresentation, InteractionState, MaterialFamily, MaterialRole,
    OpaquePigmentProfile, OpaqueSurfaceAppearance, StateSet, SurfaceForm, SurfaceIntent,
    SurfaceSize,
};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

const MAXIMUM_DIFFUSION_RADIUS: f64 = 65536.0;

pub struct TranslucentFrostInput<'a> {
    pub surface: &'a SurfaceIntent,
    pub size: SurfaceSize,
    pub appearance: &'a OpaqueSurfaceAppearance,
    pub foreground_role: ColorRole,
    pub diffusion_radius: f64,
    pub backdrop: &'a OpaqueSrgbRange,
    pub adjacent_ranges: &'a [OpaqueSrgbRange],
    pub surrounding_ranges: Option<&'a [OpaqueSrgbRange]>,
    pub minimum_content_contrast: f64,
    pub minimum_edge_contrast: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    theme: Value,
    surface: SurfaceIntent,
    size: SurfaceSize,
    appearance: OpaqueSurfaceAppearance,
    #[serde(deserialize_with = "foreground_role_name")]
    foreground_role: ColorRole,
    diffusion: DiffusionInput,
    backdrop: RangeInput,
    adjacent_ranges: Vec<RangeInput>,
    #[serde(default, deserialize_with = "present_ranges")]
    surrounding_ranges: Option<Vec<RangeInput>>,
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DiffusionMembers {
    radius: f64,
}

struct DiffusionInput {
    radius: f64,
}

impl<'de> Deserialize<'de> for DiffusionInput {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct DiffusionVisitor;
        impl<'de> serde::de::Visitor<'de> for DiffusionVisitor {
            type Value = DiffusionInput;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a diffusion object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<Self::Value, A::Error> {
                let members = DiffusionMembers::deserialize(
                    serde::de::value::MapAccessDeserializer::new(map),
                )?;
                Ok(DiffusionInput {
                    radius: members.radius,
                })
            }
        }
        deserializer.deserialize_map(DiffusionVisitor)
    }
}

fn present_ranges<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<RangeInput>>, D::Error> {
    Vec::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FrostFallbackReason {
    Selection,
    Legibility,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Diffusion {
    radius: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColorRange {
    lower: SrgbFallback,
    upper: SrgbFallback,
}

impl From<&OpaqueSrgbRange> for ColorRange {
    fn from(range: &OpaqueSrgbRange) -> Self {
        Self {
            lower: range.lower().clone(),
            upper: range.upper().clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslucentPigment {
    schema_version: &'static str,
    material_family: MaterialFamily,
    profile: OpaquePigmentProfile,
    body: SrgbFallback,
    side: SrgbFallback,
    highlight: SrgbFallback,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackdropEvidence {
    range: ColorRange,
    composited_body: ColorRange,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslucentFrostBody {
    material_role: MaterialRole,
    color_role: ColorRole,
    material_family: MaterialFamily,
    form: SurfaceForm,
    states: StateSet,
    #[serde(skip_serializing_if = "Option::is_none")]
    diffusion: Option<Diffusion>,
    geometry: OpaqueSurfaceGeometry,
    pigment: TranslucentPigment,
    lighting: KeyLightResult,
    edge_width: f64,
    highlight_width: f64,
    edge: EdgeContrastResult,
    foreground_role: ColorRole,
    foreground: SrgbFallback,
    backdrop: BackdropEvidence,
    content_contrast_ratio: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "appearance",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum FrostAppearance {
    Translucent {
        body: Box<TranslucentFrostBody>,
        #[serde(skip_serializing_if = "Option::is_none")]
        focus: Option<Box<FocusIndicatorIr>>,
    },
    Opaque {
        fallback_reason: FrostFallbackReason,
        paint: Box<SurfacePaintIr>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslucentFrostIr {
    schema_version: &'static str,
    frost_representation: FrostRepresentation,
    #[serde(flatten)]
    appearance: FrostAppearance,
}

impl TranslucentFrostIr {
    pub fn appearance(&self) -> &FrostAppearance {
        &self.appearance
    }
}

#[derive(Debug)]
pub enum TranslucentFrostError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    InvalidRequestShape,
    UnsupportedVersion,
    Theme(ThemeResolutionError),
    Range(ContrastRangeError),
    Binding(SurfaceBindingError),
    Scope(FrostSurfaceReadabilityError),
    InvalidDiffusionRadius,
    InvalidContentThreshold,
    EmptySurroundingRanges,
    MissingSurroundingRanges,
    MissingForeground(ColorRole),
    TranslucentForeground(ColorRole),
    InsufficientContrast { selected: f64, opaque: f64 },
    Edge(EdgeContrastError),
    Pigment(OpaquePigmentError),
    Surface(OpaqueSurfaceError),
    Readability(SurfaceReadabilityError),
    Paint(SurfacePaintResolutionError),
}

impl fmt::Display for TranslucentFrostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "translucent Frost parse failed: {e}"),
            Self::Request(e) => write!(f, "invalid translucent Frost request: {e}"),
            Self::InvalidRequestShape => {
                f.write_str("translucent Frost request must be a JSON object")
            }
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
            Self::Theme(e) => write!(f, "translucent Frost theme: {e}"),
            Self::Range(e) => write!(f, "translucent Frost range: {e}"),
            Self::Binding(e) => write!(f, "translucent Frost binding: {e}"),
            Self::Scope(e) => write!(f, "translucent Frost scope: {e}"),
            Self::InvalidDiffusionRadius => {
                write!(
                    f,
                    "diffusion radius must be positive and at most {MAXIMUM_DIFFUSION_RADIUS}"
                )
            }
            Self::InvalidContentThreshold => {
                f.write_str("minimumContentContrast must be finite and in [1, 21]")
            }
            Self::EmptySurroundingRanges => f.write_str("surrounding ranges must not be empty"),
            Self::MissingSurroundingRanges => {
                f.write_str("focused surface requires surrounding ranges")
            }
            Self::MissingForeground(role) => write!(f, "missing resolved color role {role:?}"),
            Self::TranslucentForeground(role) => {
                write!(
                    f,
                    "foreground role {role:?} must resolve to an opaque color"
                )
            }
            Self::InsufficientContrast { selected, opaque } => write!(
                f,
                "Frost content contrast is insufficient: translucent bound {selected}, opaque fallback {opaque}"
            ),
            Self::Edge(e) => write!(f, "translucent Frost edge: {e}"),
            Self::Pigment(e) => write!(f, "translucent Frost pigment: {e}"),
            Self::Surface(e) => write!(f, "translucent Frost surface: {e}"),
            Self::Readability(e) => write!(f, "translucent Frost readability: {e}"),
            Self::Paint(e) => write!(f, "translucent Frost paint: {e}"),
        }
    }
}

impl std::error::Error for TranslucentFrostError {}

fn range(input: RangeInput) -> Result<OpaqueSrgbRange, TranslucentFrostError> {
    let color = |input: crate::srgb_input::SrgbInput| {
        input.into_fallback().map_err(|error| {
            TranslucentFrostError::Surface(match error {
                SrgbInputError::InvalidColorSpace => {
                    OpaqueSurfaceError::InvalidColorSpace("range bound")
                }
                SrgbInputError::Color(error) => OpaqueSurfaceError::Color("range bound", error),
            })
        })
    };
    OpaqueSrgbRange::try_new(color(input.lower)?, color(input.upper)?)
        .map_err(TranslucentFrostError::Range)
}

pub fn resolve_translucent_frost_source(
    source: &str,
) -> Result<TranslucentFrostIr, TranslucentFrostError> {
    let document = parse_token_document(source).map_err(TranslucentFrostError::Parse)?;
    if !document.is_object() {
        return Err(TranslucentFrostError::InvalidRequestShape);
    }
    let request: Request =
        serde_json::from_value(document).map_err(TranslucentFrostError::Request)?;
    crate::pigment_source::validate_pigment_source(source, &["appearance", "pigmentProfiles"])
        .map_err(TranslucentFrostError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(TranslucentFrostError::UnsupportedVersion);
    }
    let (theme, environment) =
        compile_theme_request_document(request.theme).map_err(TranslucentFrostError::Theme)?;
    let backdrop = range(request.backdrop)?;
    let adjacent = request
        .adjacent_ranges
        .into_iter()
        .map(range)
        .collect::<Result<Vec<_>, _>>()?;
    let surrounding = request
        .surrounding_ranges
        .map(|ranges| ranges.into_iter().map(range).collect::<Result<Vec<_>, _>>())
        .transpose()?;
    resolve_translucent_frost(
        &theme,
        &environment,
        TranslucentFrostInput {
            surface: &request.surface,
            size: request.size,
            appearance: &request.appearance,
            foreground_role: request.foreground_role,
            diffusion_radius: request.diffusion.radius,
            backdrop: &backdrop,
            adjacent_ranges: &adjacent,
            surrounding_ranges: surrounding.as_deref(),
            minimum_content_contrast: request.minimum_content_contrast,
            minimum_edge_contrast: request.minimum_edge_contrast,
        },
    )
}

pub fn resolve_translucent_frost(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    input: TranslucentFrostInput<'_>,
) -> Result<TranslucentFrostIr, TranslucentFrostError> {
    if !input.diffusion_radius.is_finite()
        || input.diffusion_radius <= 0.0
        || input.diffusion_radius > MAXIMUM_DIFFUSION_RADIUS
    {
        return Err(TranslucentFrostError::InvalidDiffusionRadius);
    }
    if !input.minimum_content_contrast.is_finite()
        || !(1.0..=21.0).contains(&input.minimum_content_contrast)
    {
        return Err(TranslucentFrostError::InvalidContentThreshold);
    }
    if input.surrounding_ranges.is_some_and(<[_]>::is_empty) {
        return Err(TranslucentFrostError::EmptySurroundingRanges);
    }
    let focused = input
        .surface
        .states()
        .states()
        .contains(&InteractionState::Focused);
    if focused && input.surrounding_ranges.is_none() {
        return Err(TranslucentFrostError::MissingSurroundingRanges);
    }
    let snapshot = theme
        .resolve(environment)
        .map_err(|error| TranslucentFrostError::Theme(ThemeResolutionError::Resolve(error)))?;
    let binding = bind_surface(input.surface, &snapshot).map_err(TranslucentFrostError::Binding)?;
    validate_surface_scope(&binding).map_err(TranslucentFrostError::Scope)?;
    let adjacent = EdgeBackground::Ranges(input.adjacent_ranges);
    adjacent.validate().map_err(TranslucentFrostError::Edge)?;
    let foreground = snapshot
        .color_fallbacks()
        .get(&input.foreground_role)
        .ok_or(TranslucentFrostError::MissingForeground(
            input.foreground_role,
        ))?
        .clone();
    if foreground.alpha() != 1.0 {
        return Err(TranslucentFrostError::TranslucentForeground(
            input.foreground_role,
        ));
    }
    let representation = binding
        .frost_representation()
        .expect("Frost binding publishes its representation");
    let depths = resolve_elevation_depth(input.appearance.depth_assignments(), theme.tokens())
        .map_err(|errors| TranslucentFrostError::Surface(OpaqueSurfaceError::Depth(errors)))?;
    let depth = depths[&binding.form().elevation()]["value"]
        .as_f64()
        .expect("validated depth is numeric");
    let translucent = representation != FrostRepresentation::OpaqueDimensional;
    let mut certificate = None;
    let mut reason = FrostFallbackReason::Selection;
    if translucent {
        let body = binding
            .frost_portable_body()
            .expect("Frost binding publishes its body");
        let composited = composite_range(body, input.backdrop)?;
        let bound = opaque_contrast_over_range(&foreground, &composited)
            .expect("foreground and composited range are opaque");
        if bound >= input.minimum_content_contrast {
            certificate = Some((composited, bound));
        } else {
            let opaque = opaque_contrast_ratio(&foreground, binding.opaque_color_fallback())
                .expect("opaque fallback is opaque");
            if opaque < input.minimum_content_contrast {
                return Err(TranslucentFrostError::InsufficientContrast {
                    selected: bound,
                    opaque,
                });
            }
            reason = FrostFallbackReason::Legibility;
        }
    }
    let surrounding = input.surrounding_ranges.map(EdgeBackground::Ranges);
    let paint_input = ControlPaintInput {
        surface: input.surface,
        size: input.size,
        appearance: input.appearance,
        foreground_role: input.foreground_role,
        post_treatment_backdrop: None,
        adjacent,
        surrounding,
        minimum_content_contrast: input.minimum_content_contrast,
        minimum_edge_contrast: input.minimum_edge_contrast,
    };
    let focus = resolve_control_focus(theme, environment, &snapshot, &paint_input, depth, true)
        .map_err(TranslucentFrostError::Paint)?;
    let Some((composited, bound)) = certificate else {
        let focus = if translucent {
            focus.map(FocusIndicatorIr::with_opaque_frost_fallback)
        } else {
            focus
        };
        let binding = if translucent {
            binding.with_opaque_frost_fallback()
        } else {
            binding
        };
        let readable = resolve_bound_body_readability(
            binding,
            &snapshot,
            input.foreground_role,
            Some(input.backdrop.lower()),
            adjacent,
            input.minimum_content_contrast,
            input.minimum_edge_contrast,
        )
        .map_err(TranslucentFrostError::Readability)?;
        let readable = if reason == FrostFallbackReason::Legibility {
            readable.with_content_fallback_applied()
        } else {
            readable
        };
        let body = resolve_opaque_body_geometry(
            theme,
            environment,
            input.size,
            input.appearance,
            &readable,
            depth,
        )
        .map_err(TranslucentFrostError::Surface)?;
        return Ok(TranslucentFrostIr {
            schema_version: "0.1.0",
            frost_representation: FrostRepresentation::OpaqueDimensional,
            appearance: FrostAppearance::Opaque {
                fallback_reason: reason,
                paint: Box::new(SurfacePaintIr {
                    schema_version: "0.1.0",
                    body,
                    focus,
                }),
            },
        });
    };
    let outline = snapshot
        .opaque_color_fallbacks()
        .get(&ColorRole::Outline)
        .ok_or(TranslucentFrostError::MissingForeground(ColorRole::Outline))?;
    let strong = snapshot
        .opaque_color_fallbacks()
        .get(&ColorRole::OutlineStrong)
        .ok_or(TranslucentFrostError::MissingForeground(
            ColorRole::OutlineStrong,
        ))?;
    let edge = adjacent
        .resolve(outline, strong, input.minimum_edge_contrast)
        .map_err(TranslucentFrostError::Edge)?;
    let shape = resolve_surface_shape(
        theme,
        environment,
        input.size,
        input.appearance,
        &binding,
        depth,
    )
    .map_err(TranslucentFrostError::Surface)?;
    let pigment = translucent_pigment(&binding, input.appearance)?;
    let diffusion =
        (representation != FrostRepresentation::TranslucentPigmented).then_some(Diffusion {
            radius: input.diffusion_radius,
        });
    Ok(TranslucentFrostIr {
        schema_version: "0.1.0",
        frost_representation: representation,
        appearance: FrostAppearance::Translucent {
            body: Box::new(TranslucentFrostBody {
                material_role: binding.material_role(),
                color_role: binding.color_role(),
                material_family: binding.material_family(),
                form: binding.form().clone(),
                states: binding.states().clone(),
                diffusion,
                geometry: shape.geometry,
                pigment,
                lighting: shape.lighting,
                edge_width: shape.edge_width,
                highlight_width: shape.highlight_width,
                edge,
                foreground_role: input.foreground_role,
                foreground,
                backdrop: BackdropEvidence {
                    range: ColorRange::from(input.backdrop),
                    composited_body: ColorRange::from(&composited),
                },
                content_contrast_ratio: bound,
            }),
            focus: focus.map(Box::new),
        },
    })
}

fn composite_range(
    body: &SrgbFallback,
    backdrop: &OpaqueSrgbRange,
) -> Result<OpaqueSrgbRange, TranslucentFrostError> {
    let over = |color: &SrgbFallback| {
        composite_srgb_over_opaque(body, color).expect("range bounds are opaque")
    };
    OpaqueSrgbRange::try_new(over(backdrop.lower()), over(backdrop.upper()))
        .map_err(TranslucentFrostError::Range)
}

fn translucent_pigment(
    binding: &BoundSurface,
    appearance: &OpaqueSurfaceAppearance,
) -> Result<TranslucentPigment, TranslucentFrostError> {
    let body = binding
        .frost_portable_body()
        .expect("Frost binding publishes its body");
    let flat = body.with_alpha(1.0).expect("unit alpha is valid");
    let opaque = resolve_opaque_pigment(
        binding.material_family(),
        &flat,
        appearance.pigment_profiles(),
    )
    .map_err(TranslucentFrostError::Pigment)?;
    let keep_alpha = |color: &SrgbFallback| {
        color
            .with_alpha(body.alpha())
            .expect("body alpha is a valid alpha")
    };
    Ok(TranslucentPigment {
        schema_version: "0.1.0",
        material_family: binding.material_family(),
        profile: *opaque.profile(),
        body: body.clone(),
        side: keep_alpha(opaque.side()),
        highlight: keep_alpha(opaque.highlight()),
    })
}
