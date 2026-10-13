use crate::{
    EdgeContrastResult, OpaquePigmentResult, OutlineCurvature, SrgbFallback, SurfaceRelief,
    SurfaceReliefProfile, opaque_contrast_ratio, resolve_edge_contrast_source, resolve_key_light,
    resolve_opaque_pigment_source, resolve_surface_outline_source, resolve_surface_relief,
    srgb_input::{SrgbInput, SrgbInputError},
};
use resina_color::OpaqueSrgbRange;
use resina_model::{KeyLight, PhysicalBounds, PhysicalVector};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use std::fmt;
mod edge;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReliefPaintError(String);

impl fmt::Display for ReliefPaintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "relief paint: {}", self.0)
    }
}
impl std::error::Error for ReliefPaintError {}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReliefPaint {
    surface: SurfaceRelief,
    pigment: OpaquePigmentResult,
    direction: PhysicalVector,
    edge: EdgeContrastResult,
    edge_width: f64,
    foreground: SrgbFallback,
    content_contrast_ratio: f64,
    content_bounds: PhysicalBounds,
}

#[derive(Debug, Serialize)]
pub struct ReliefPaintPoint {
    pub point: PhysicalVector,
    pub color: Option<SrgbFallback>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReliefPaintResult {
    pub schema_version: &'static str,
    pub paint: ReliefPaint,
    pub samples: Vec<ReliefPaintPoint>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PaintRequest<'a> {
    schema_version: String,
    #[serde(borrow)]
    outline: &'a serde_json::value::RawValue,
    profile: SurfaceReliefProfile,
    #[serde(borrow)]
    pigment: &'a serde_json::value::RawValue,
    key_light: KeyLight,
    edge: serde_json::Value,
    edge_width: f64,
    foreground: SrgbInput,
    minimum_content_contrast: f64,
    points: Vec<PhysicalVector>,
}

pub fn resolve_relief_paint_source(source: &str) -> Result<ReliefPaintResult, ReliefPaintError> {
    let error = |e: &dyn fmt::Display| ReliefPaintError(e.to_string());
    let document = parse_token_document(source).map_err(|e| error(&e))?;
    for pointer in [
        "",
        "/outline",
        "/profile",
        "/pigment",
        "/pigment/body",
        "/pigment/profiles",
        "/keyLight",
        "/edge",
        "/edge/outline",
        "/edge/outlineStrong",
        "/edge/adjacentColor",
        "/foreground",
    ] {
        if !document
            .pointer(pointer)
            .is_some_and(serde_json::Value::is_object)
        {
            return Err(ReliefPaintError(format!("{pointer} must be an object")));
        }
    }
    let points = document
        .get("points")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ReliefPaintError("points must be an array".into()))?;
    if points.iter().any(|point| !point.is_object()) {
        return Err(ReliefPaintError("points must contain objects".into()));
    }
    let request: PaintRequest<'_> = serde_json::from_str(source).map_err(|e| error(&e))?;
    if request.schema_version != "0.1.0" {
        return Err(ReliefPaintError("schemaVersion must be 0.1.0".into()));
    }
    let outline = resolve_surface_outline_source(request.outline.get()).map_err(|e| error(&e))?;
    let surface = resolve_surface_relief(outline, request.profile).map_err(|e| error(&e))?;
    let pigment = resolve_opaque_pigment_source(request.pigment.get()).map_err(|e| error(&e))?;
    let edge = resolve_edge_contrast_source(&request.edge.to_string()).map_err(|e| error(&e))?;
    let foreground = request.foreground.into_fallback().map_err(|e| match e {
        SrgbInputError::InvalidColorSpace => ReliefPaintError("foreground must use sRGB".into()),
        SrgbInputError::Color(e) => error(&e),
    })?;
    let paint = resolve_relief_paint(
        surface,
        pigment,
        &request.key_light,
        edge,
        request.edge_width,
        foreground,
        request.minimum_content_contrast,
    )?;
    let samples = request
        .points
        .into_iter()
        .map(|point| {
            Ok(ReliefPaintPoint {
                point,
                color: paint.sample_paint(point)?,
            })
        })
        .collect::<Result<Vec<_>, ReliefPaintError>>()?;
    Ok(ReliefPaintResult {
        schema_version: "0.1.0",
        paint,
        samples,
    })
}

pub fn resolve_relief_paint(
    surface: SurfaceRelief,
    pigment: OpaquePigmentResult,
    key_light: &KeyLight,
    edge: EdgeContrastResult,
    edge_width: f64,
    foreground: SrgbFallback,
    minimum_content_contrast: f64,
) -> Result<ReliefPaint, ReliefPaintError> {
    let fail = |s: &str| ReliefPaintError(s.into());
    let size = surface.outline().size();
    if !edge_width.is_finite()
        || edge_width <= 0.0
        || 2.0 * edge_width >= size.width
        || 2.0 * edge_width >= size.height
        || size.width - edge_width == size.width
        || size.height - edge_width == size.height
    {
        return Err(fail(
            "edgeWidth must preserve a positive representable interior",
        ));
    }
    for corner in surface.outline().corners() {
        if corner.x > edge_width
            && corner.y > edge_width
            && (corner.x - edge_width == corner.x || corner.y - edge_width == corner.y)
        {
            return Err(fail("edgeWidth loses a corner inset"));
        }
    }
    if foreground.alpha() != 1.0 {
        return Err(fail("foreground must be opaque"));
    }
    if !minimum_content_contrast.is_finite() || !(1.0..=21.0).contains(&minimum_content_contrast) {
        return Err(fail("minimumContentContrast must be finite and in [1,21]"));
    }
    let contrast =
        opaque_contrast_ratio(&foreground, pigment.body()).map_err(|e| fail(&e.to_string()))?;
    if contrast < minimum_content_contrast {
        return Err(fail(
            "foreground contrast is insufficient on the uniform body",
        ));
    }
    let direction = resolve_key_light(key_light, 0.0, &[])
        .map_err(|e| fail(&e.to_string()))?
        .direction();
    let mut paint = ReliefPaint {
        surface,
        pigment,
        direction,
        edge,
        edge_width,
        foreground,
        content_contrast_ratio: contrast,
        content_bounds: PhysicalBounds {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
    };
    paint.content_bounds = paint.reserve_content()?;
    Ok(paint)
}

impl ReliefPaint {
    pub fn surface(&self) -> &SurfaceRelief {
        &self.surface
    }
    pub fn pigment(&self) -> &OpaquePigmentResult {
        &self.pigment
    }
    pub fn edge(&self) -> &EdgeContrastResult {
        &self.edge
    }
    pub fn foreground(&self) -> &SrgbFallback {
        &self.foreground
    }
    pub fn content_bounds(&self) -> PhysicalBounds {
        self.content_bounds
    }

    fn edge_interior_contains(&self, point: PhysicalVector) -> bool {
        edge::contains(self.surface.outline(), self.edge_width, point)
    }

    fn reserve_content(&self) -> Result<PhysicalBounds, ReliefPaintError> {
        let inset = self.edge_width;
        let diagonal = match self.surface.outline().curvature() {
            OutlineCurvature::Circular => 1.0 - 0.5_f64.sqrt(),
            OutlineCurvature::Quartic => 1.0 - 0.5_f64.sqrt().sqrt(),
        } + 8.0 * f64::EPSILON;
        let pad = |axis: f64| {
            if axis <= inset {
                inset
            } else {
                (inset + (axis - inset) * diagonal).next_up()
            }
        };
        let corners = self.surface.outline().corners();
        let plateau = self.surface.content_bounds();
        let size = self.surface.outline().size();
        let left = pad(corners[0].x).max(pad(corners[3].x)).max(plateau.x);
        let top = pad(corners[0].y).max(pad(corners[1].y)).max(plateau.y);
        let right =
            (size.width - pad(corners[1].x).max(pad(corners[2].x))).min(plateau.x + plateau.width);
        let bottom = (size.height - pad(corners[2].y).max(pad(corners[3].y)))
            .min(plateau.y + plateau.height);
        let bounds = PhysicalBounds {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        };
        if !bounds.width.is_finite()
            || !bounds.height.is_finite()
            || bounds.width <= 0.0
            || bounds.height <= 0.0
            || left + bounds.width == left
            || top + bounds.height == top
        {
            return Err(ReliefPaintError(
                "content rectangle must preserve representable positive area".into(),
            ));
        }
        for x in [left, left + bounds.width] {
            for y in [top, top + bounds.height] {
                let point = PhysicalVector { x, y };
                if !self.edge_interior_contains(point)
                    || self.sample_paint(point)?.as_ref() != Some(self.pigment.body())
                {
                    return Err(ReliefPaintError(
                        "content rectangle cannot certify uniform body containment".into(),
                    ));
                }
            }
        }
        Ok(bounds)
    }

    pub fn sample_paint(
        &self,
        point: PhysicalVector,
    ) -> Result<Option<SrgbFallback>, ReliefPaintError> {
        if !self
            .surface
            .outline()
            .contains(point)
            .map_err(|e| ReliefPaintError(e.to_string()))?
        {
            return Ok(None);
        }
        if !self.edge_interior_contains(point) {
            return Ok(Some(self.edge.color().clone()));
        }
        let normal = self
            .surface
            .sample(point)
            .map_err(|e| ReliefPaintError(e.to_string()))?
            .ok_or_else(|| ReliefPaintError("interior point lost outline containment".into()))?
            .normal;
        let q = (normal.x * self.direction.x + normal.y * self.direction.y).clamp(-1.0, 1.0);
        if q == 0.0 {
            return Ok(Some(self.pigment.body().clone()));
        }
        let endpoint = if q > 0.0 {
            self.pigment.highlight()
        } else {
            self.pigment.side()
        };
        let body = self.pigment.body().components();
        let tip = endpoint.components();
        let weight = q.abs();
        let components =
            std::array::from_fn::<_, 3, _>(|i| body[i] * (1.0 - weight) + tip[i] * weight);
        let color = resina_color::resolve_srgb_fallback(&serde_json::json!({
            "colorSpace":"srgb","components":components,"alpha":1
        }))
        .map_err(|e| ReliefPaintError(e.to_string()))?;
        Ok(Some(color))
    }

    pub fn paint_color_ranges(&self) -> Vec<OpaqueSrgbRange> {
        let range = |endpoint: &SrgbFallback| {
            let body = self.pigment.body().components();
            let tip = endpoint.components();
            let margin = 4.0 * f64::EPSILON;
            let lower = std::array::from_fn::<_, 3, _>(|i| {
                (body[i].min(tip[i]) - margin).next_down().max(0.0)
            });
            let upper = std::array::from_fn::<_, 3, _>(|i| {
                (body[i].max(tip[i]) + margin).next_up().min(1.0)
            });
            let color = |components: [f64; 3]| {
                resina_color::resolve_srgb_fallback(&serde_json::json!({
                    "colorSpace":"srgb","components":components,"alpha":1
                }))
                .expect("bounded opaque interpolation range")
            };
            OpaqueSrgbRange::try_new(color(lower), color(upper)).expect("ordered pigment range")
        };
        vec![
            range(self.pigment.highlight()),
            range(self.pigment.side()),
            OpaqueSrgbRange::try_new(self.edge.color().clone(), self.edge.color().clone())
                .expect("opaque edge"),
        ]
    }
}
