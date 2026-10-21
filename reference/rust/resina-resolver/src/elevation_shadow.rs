use crate::{
    ExtrudedContourResult, SrgbFallback, resolve_extruded_contour, resolve_key_light,
    srgb_input::{SrgbInput, SrgbInputError},
};
use resina_color::{OpaqueSrgbRange, opaque_contrast_over_range, resolve_srgb_fallback};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ElevationRole, KeyLight, LogicalCornerRadii, PhysicalBounds, PhysicalVector, SurfaceSize,
};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

const VERSION: &str = "0.1.0";
const MAX_DEPTH_SCALE: f64 = 2.0;
const PENUMBRA_PER_DEPTH: f64 = 0.5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElevationShadowError(String);

impl fmt::Display for ElevationShadowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "elevation shadow: {}", self.0)
    }
}

impl std::error::Error for ElevationShadowError {}

fn error(message: impl fmt::Display) -> ElevationShadowError {
    ElevationShadowError(message.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    environment: EnvironmentSnapshot,
    key_light: KeyLight,
    profiles: ShadowProfiles,
    elevation: ElevationRole,
    elevation_depth: f64,
    depth_scale: f64,
    size: SurfaceSize,
    radii: LogicalCornerRadii,
    adjacent_color: SrgbInput,
    edge: EdgeRequest,
    #[serde(deserialize_with = "Option::deserialize")]
    focus: Option<FocusRequest>,
    distance_samples: Vec<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ShadowProfiles {
    schema_version: String,
    roles: RoleProfiles,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RoleProfiles {
    embedded: ShadowProfile,
    base: ShadowProfile,
    raised: ShadowProfile,
    floating: ShadowProfile,
    overlay: ShadowProfile,
    modal: ShadowProfile,
}

impl RoleProfiles {
    fn all(&self) -> [(ElevationRole, &'static str, ShadowProfile); 6] {
        [
            (ElevationRole::Embedded, "embedded", self.embedded),
            (ElevationRole::Base, "base", self.base),
            (ElevationRole::Raised, "raised", self.raised),
            (ElevationRole::Floating, "floating", self.floating),
            (ElevationRole::Overlay, "overlay", self.overlay),
            (ElevationRole::Modal, "modal", self.modal),
        ]
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ShadowProfile {
    shadow_shade: f64,
    contact_margin: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EdgeRequest {
    outline: SrgbInput,
    outline_strong: SrgbInput,
    minimum_contrast: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FocusRequest {
    color: SrgbInput,
    minimum_contrast: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElevationShadowResult {
    schema_version: &'static str,
    elevation: ElevationRole,
    key_direction: PhysicalVector,
    final_depth: f64,
    side_offset: PhysicalVector,
    shadow_shade: f64,
    contact_margin: f64,
    shadow: Option<Shadow>,
    edge: Guard,
    focus: Option<Guard>,
    samples: Vec<Sample>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Shadow {
    sweep: PhysicalVector,
    hull: ExtrudedContourResult,
    band_thickness: FaceThickness,
    band_color: SrgbFallback,
    color_range: OpaqueSrgbRange,
    delta_e_ok: f64,
    penumbra_width: f64,
    paint_bounds: PhysicalBounds,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct FaceThickness {
    top: f64,
    right: f64,
    bottom: f64,
    left: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Guard {
    color_role: &'static str,
    color: SrgbFallback,
    contrast_ratio: f64,
    fallback_applied: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct Sample {
    weight: f64,
    color: SrgbFallback,
}

fn require_object(document: &Value, pointer: &str) -> Result<(), ElevationShadowError> {
    match document.pointer(pointer) {
        Some(value) if !value.is_object() => Err(error(format!(
            "{} must be a JSON object",
            if pointer.is_empty() {
                "request"
            } else {
                pointer
            }
        ))),
        _ => Ok(()),
    }
}

fn color(field: &str, input: SrgbInput) -> Result<SrgbFallback, ElevationShadowError> {
    let color = input.into_fallback().map_err(|e| match e {
        SrgbInputError::InvalidColorSpace => error(format!("{field} must use sRGB")),
        SrgbInputError::Color(e) => error(format!("invalid {field}: {e}")),
    })?;
    if color.alpha() != 1.0 {
        return Err(error(format!("{field} must be opaque")));
    }
    Ok(color)
}

fn minimum_contrast(field: &str, value: f64) -> Result<f64, ElevationShadowError> {
    if !value.is_finite() || !(1.0..=21.0).contains(&value) {
        return Err(error(format!("{field} must be finite and in [1, 21]")));
    }
    Ok(value)
}

fn opaque(components: [f64; 3]) -> Result<SrgbFallback, ElevationShadowError> {
    resolve_srgb_fallback(
        &serde_json::json!({"colorSpace": "srgb", "components": components, "alpha": 1}),
    )
    .map_err(error)
}

fn shaded(adjacent: &SrgbFallback, shade: f64) -> Result<SrgbFallback, ElevationShadowError> {
    opaque(adjacent.components().map(|channel| channel * (1.0 - shade)))
}

fn positive(value: f64) -> f64 {
    if value > 0.0 { value } else { 0.0 }
}

fn select(
    candidates: [(&'static str, &SrgbFallback); 2],
    range: &OpaqueSrgbRange,
    minimum: f64,
    label: &str,
) -> Result<Guard, ElevationShadowError> {
    let mut ratios = [0.0; 2];
    for (index, (role, candidate)) in candidates.into_iter().enumerate() {
        let ratio = opaque_contrast_over_range(candidate, range).map_err(error)?;
        if ratio >= minimum {
            return Ok(Guard {
                color_role: role,
                color: candidate.clone(),
                contrast_ratio: ratio,
                fallback_applied: index == 1,
            });
        }
        ratios[index] = ratio;
    }
    Err(error(format!(
        "{label} contrast over the shadow range is insufficient: {} {}, {} {}",
        candidates[0].0, ratios[0], candidates[1].0, ratios[1]
    )))
}

fn oklab_distance(
    first: &SrgbFallback,
    second: &SrgbFallback,
) -> Result<f64, ElevationShadowError> {
    let first = resina_color::srgb_to_oklab(first.components()).map_err(error)?;
    let second = resina_color::srgb_to_oklab(second.components()).map_err(error)?;
    Ok(first
        .iter()
        .zip(second)
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f64>()
        .sqrt())
}

pub fn resolve_elevation_shadow_source(
    source: &str,
) -> Result<ElevationShadowResult, ElevationShadowError> {
    let document = parse_token_document(source).map_err(error)?;
    require_object(&document, "")?;
    if document
        .get("schemaVersion")
        .and_then(Value::as_str)
        .is_some_and(|version| version != VERSION)
    {
        return Err(error("/schemaVersion must be 0.1.0"));
    }
    require_object(&document, "/profiles")?;
    require_object(&document, "/profiles/roles")?;
    if let Some(roles) = document
        .pointer("/profiles/roles")
        .and_then(Value::as_object)
    {
        for name in roles.keys() {
            require_object(&document, &format!("/profiles/roles/{name}"))?;
        }
    }
    require_object(&document, "/edge")?;
    if document.get("focus").is_some_and(|focus| !focus.is_null()) {
        require_object(&document, "/focus")?;
    }
    let request: Request = serde_json::from_value(document).map_err(error)?;
    debug_assert_eq!(request.schema_version, VERSION);
    resolve(request)
}

fn resolve(request: Request) -> Result<ElevationShadowResult, ElevationShadowError> {
    let profiles = &request.profiles;
    if profiles.schema_version != VERSION {
        return Err(error("/profiles/schemaVersion must be 0.1.0"));
    }
    for (_, name, profile) in profiles.roles.all() {
        if !profile.shadow_shade.is_finite() || !(0.0..1.0).contains(&profile.shadow_shade) {
            return Err(error(format!(
                "/profiles/roles/{name}/shadowShade must be finite and in [0, 1)"
            )));
        }
        if !profile.contact_margin.is_finite() || profile.contact_margin < 0.0 {
            return Err(error(format!(
                "/profiles/roles/{name}/contactMargin must be finite and nonnegative"
            )));
        }
    }
    let profile = profiles
        .roles
        .all()
        .into_iter()
        .find(|(role, _, _)| *role == request.elevation)
        .map(|(_, _, profile)| profile)
        .expect("every elevation role has a profile");
    if !request.elevation_depth.is_finite() || request.elevation_depth < 0.0 {
        return Err(error("/elevationDepth must be finite and nonnegative"));
    }
    if !request.depth_scale.is_finite() || !(0.0..=MAX_DEPTH_SCALE).contains(&request.depth_scale) {
        return Err(error("/depthScale must be finite and in [0, 2]"));
    }
    let adjacent = color("/adjacentColor", request.adjacent_color)?;
    let outline = color("/edge/outline", request.edge.outline)?;
    let outline_strong = color("/edge/outlineStrong", request.edge.outline_strong)?;
    let edge_minimum = minimum_contrast("/edge/minimumContrast", request.edge.minimum_contrast)?;
    let focus = request
        .focus
        .map(|focus| {
            Ok::<_, ElevationShadowError>((
                color("/focus/color", focus.color)?,
                minimum_contrast("/focus/minimumContrast", focus.minimum_contrast)?,
            ))
        })
        .transpose()?;
    if let Some(index) = request.distance_samples.iter().position(|d| !d.is_finite()) {
        return Err(error(format!("/distanceSamples/{index} must be finite")));
    }

    let final_depth = request.elevation_depth * request.depth_scale;
    if !final_depth.is_finite() {
        return Err(error("final depth exceeds finite range"));
    }
    let light = resolve_key_light(&request.key_light, final_depth, &[]).map_err(error)?;
    let direction = light.direction();
    let reach = 2.0 * final_depth + profile.contact_margin;
    let sweep = PhysicalVector {
        x: -direction.x * reach,
        y: -direction.y * reach,
    };
    if !reach.is_finite() || !sweep.x.is_finite() || !sweep.y.is_finite() {
        return Err(error("shadow sweep exceeds finite range"));
    }
    let hull = resolve_extruded_contour(
        request.size,
        request.radii,
        request.environment.layout_direction(),
        sweep,
    )
    .map_err(error)?;

    let shade = profile.shadow_shade;
    let shadow = match hull.bounds() {
        Some(bounds) if final_depth > 0.0 => {
            let preferences = request.environment.accessibility_preferences();
            let penumbra_width = if request.environment.renderer_capabilities().advanced_shadow
                && !preferences.high_contrast
            {
                PENUMBRA_PER_DEPTH * final_depth
            } else {
                0.0
            };
            let paint_bounds = PhysicalBounds {
                x: bounds.x - penumbra_width,
                y: bounds.y - penumbra_width,
                width: bounds.width + 2.0 * penumbra_width,
                height: bounds.height + 2.0 * penumbra_width,
            };
            if [
                penumbra_width,
                paint_bounds.x,
                paint_bounds.y,
                paint_bounds.width,
                paint_bounds.height,
            ]
            .iter()
            .any(|value| !value.is_finite())
            {
                return Err(error("shadow paint bounds exceed finite range"));
            }
            let extent = final_depth + profile.contact_margin;
            let band_color = shaded(&adjacent, shade)?;
            Some(Shadow {
                sweep,
                band_thickness: FaceThickness {
                    top: positive(direction.y * extent),
                    right: positive(-direction.x * extent),
                    bottom: positive(-direction.y * extent),
                    left: positive(direction.x * extent),
                },
                delta_e_ok: oklab_distance(&band_color, &adjacent)?,
                color_range: OpaqueSrgbRange::try_new(band_color.clone(), adjacent.clone())
                    .map_err(error)?,
                band_color,
                hull,
                penumbra_width,
                paint_bounds,
            })
        }
        _ => None,
    };

    let range = match &shadow {
        Some(shadow) => shadow.color_range.clone(),
        None => OpaqueSrgbRange::try_new(adjacent.clone(), adjacent.clone()).map_err(error)?,
    };
    let edge = select(
        [("outline", &outline), ("outline.strong", &outline_strong)],
        &range,
        edge_minimum,
        "edge",
    )?;
    let focus = focus
        .map(|(focus, minimum)| {
            select(
                [("focus", &focus), ("outline.strong", &outline_strong)],
                &range,
                minimum,
                "focus",
            )
        })
        .transpose()?;

    let samples = request
        .distance_samples
        .iter()
        .map(|&distance| {
            let weight = match &shadow {
                None => 0.0,
                Some(_) if distance <= 0.0 => 1.0,
                Some(shadow) if distance < shadow.penumbra_width => {
                    1.0 - distance / shadow.penumbra_width
                }
                Some(_) => 0.0,
            };
            Ok(Sample {
                weight,
                color: shaded(&adjacent, shade * weight)?,
            })
        })
        .collect::<Result<Vec<_>, ElevationShadowError>>()?;

    Ok(ElevationShadowResult {
        schema_version: VERSION,
        elevation: request.elevation,
        key_direction: direction,
        final_depth,
        side_offset: light.side_offset(),
        shadow_shade: shade,
        contact_margin: profile.contact_margin,
        shadow,
        edge,
        focus,
        samples,
    })
}
