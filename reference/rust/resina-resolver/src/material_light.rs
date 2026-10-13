use crate::{OpaquePigmentResult, SrgbFallback, resolve_key_light, resolve_opaque_pigment_source};
use resina_color::OpaqueSrgbRange;
use resina_environment::{EnvironmentSnapshot, QualityPolicy};
use resina_model::{KeyLight, MaterialLight, PhysicalVector};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialLightError(String);

impl fmt::Display for MaterialLightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "material light: {}", self.0)
    }
}
impl std::error::Error for MaterialLightError {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request<'a> {
    schema_version: String,
    #[serde(borrow)]
    light: &'a RawValue,
    environment: EnvironmentSnapshot,
    enabled: bool,
    interaction_active: bool,
    #[serde(borrow)]
    pigment: &'a RawValue,
    normals: Vec<Normal>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Normal {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Deserialize)]
struct CoefficientSources<'a> {
    #[serde(borrow)]
    ambient: &'a RawValue,
    #[serde(borrow)]
    interaction: Option<InteractionSource<'a>>,
}

#[derive(Deserialize)]
struct InteractionSource<'a> {
    #[serde(borrow)]
    strength: &'a RawValue,
}

#[derive(Debug, Serialize)]
struct ResolvedInteraction {
    direction: PhysicalVector,
    strength: f64,
}

#[derive(Debug, Serialize)]
struct Sample {
    weight: f64,
    color: SrgbFallback,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialLightResult {
    schema_version: &'static str,
    pigment: OpaquePigmentResult,
    key_direction: PhysicalVector,
    ambient: f64,
    interaction: Option<ResolvedInteraction>,
    color_ranges: [OpaqueSrgbRange; 2],
    samples: Vec<Sample>,
}

pub fn resolve_material_light_source(
    source: &str,
) -> Result<MaterialLightResult, MaterialLightError> {
    let error = |e: &dyn fmt::Display| MaterialLightError(e.to_string());
    let document = parse_token_document(source).map_err(|e| error(&e))?;
    if !document.is_object() {
        return Err(MaterialLightError("request must be an object".into()));
    }
    if document
        .get("normals")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|normals| normals.iter().any(|normal| !normal.is_object()))
    {
        return Err(MaterialLightError("normals must contain objects".into()));
    }
    let request: Request<'_> = serde_json::from_str(source).map_err(|e| error(&e))?;
    if request.schema_version != "0.1.0" {
        return Err(MaterialLightError("schemaVersion must be 0.1.0".into()));
    }
    let light: MaterialLight = serde_json::from_str(request.light.get()).map_err(|e| error(&e))?;
    let coefficients: CoefficientSources<'_> =
        serde_json::from_str(request.light.get()).map_err(|e| error(&e))?;
    if light.ambient().is_some() {
        crate::pigment_source::validate_nonnegative_coefficient(
            coefficients.ambient.get(),
            "ambient",
        )
        .map_err(|e| error(&e))?;
    }
    if let Some(interaction) = coefficients.interaction {
        crate::pigment_source::validate_nonnegative_coefficient(
            interaction.strength.get(),
            "interaction strength",
        )
        .map_err(|e| error(&e))?;
    }
    let pigment = resolve_opaque_pigment_source(request.pigment.get()).map_err(|e| error(&e))?;
    for (index, normal) in request.normals.iter().enumerate() {
        if [normal.x, normal.y, normal.z]
            .iter()
            .any(|v| !v.is_finite() || !(-1.0..=1.0).contains(v))
            || (normal.x * normal.x + normal.y * normal.y + normal.z * normal.z - 1.0).abs() > 1e-12
        {
            return Err(MaterialLightError(format!(
                "normal {index} must be finite and unit length"
            )));
        }
    }
    let key_direction = resolve_key_light(light.key_light(), 0.0, &[])
        .map_err(|e| error(&e))?
        .direction();
    let preferences = request.environment.accessibility_preferences();
    let ambient = if preferences.high_contrast {
        0.0
    } else {
        light.ambient().unwrap_or(0.0)
    };
    let allow_interaction = request.enabled
        && request.interaction_active
        && request.environment.renderer_capabilities().dynamic_lighting
        && request.environment.quality_policy() != QualityPolicy::Economy
        && !preferences.high_contrast
        && !preferences.reduced_motion;
    let interaction = match light.interaction() {
        Some(input) if allow_interaction && input.strength() > 0.0 => {
            let key = KeyLight::try_new(input.direction()).map_err(|e| error(&e))?;
            Some(ResolvedInteraction {
                direction: resolve_key_light(&key, 0.0, &[])
                    .map_err(|e| error(&e))?
                    .direction(),
                strength: input.strength(),
            })
        }
        _ => None,
    };
    let samples = request
        .normals
        .iter()
        .map(|normal| {
            let mut response =
                (normal.x * key_direction.x + normal.y * key_direction.y).clamp(-1.0, 1.0);
            if let Some(interaction) = &interaction {
                let local = (normal.x * interaction.direction.x
                    + normal.y * interaction.direction.y)
                    .clamp(-1.0, 1.0);
                response = (1.0 - interaction.strength) * response + interaction.strength * local;
            }
            let weight = ((1.0 - ambient) * response).clamp(-1.0, 1.0);
            let color = if weight == 0.0 {
                pigment.body().clone()
            } else {
                let endpoint = if weight > 0.0 {
                    pigment.highlight()
                } else {
                    pigment.side()
                };
                let body = pigment.body().components();
                let tip = endpoint.components();
                let proportion = weight.abs();
                let components = std::array::from_fn::<_, 3, _>(|i| {
                    body[i] * (1.0 - proportion) + tip[i] * proportion
                });
                resina_color::resolve_srgb_fallback(
                    &serde_json::json!({"colorSpace":"srgb","components":components,"alpha":1}),
                )
                .map_err(|e| error(&e))?
            };
            Ok(Sample { weight, color })
        })
        .collect::<Result<Vec<_>, MaterialLightError>>()?;
    Ok(MaterialLightResult {
        schema_version: "0.1.0",
        color_ranges: pigment.color_ranges(),
        pigment,
        key_direction,
        ambient,
        interaction,
        samples,
    })
}
