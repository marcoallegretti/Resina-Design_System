use super::*;
use crate::material_light::{ResolvedMaterialLight, admit_material_light};
use resina_environment::EnvironmentSnapshot;
use serde_json::value::RawValue;

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialReliefPaint {
    face: ReliefPaint,
    light: ResolvedMaterialLight,
}

impl Serialize for MaterialReliefPaint {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Paint<'a> {
            surface: &'a SurfaceRelief,
            pigment: &'a OpaquePigmentResult,
            light: &'a ResolvedMaterialLight,
            edge: &'a EdgeContrastResult,
            edge_width: f64,
            foreground: &'a SrgbFallback,
            content_contrast_ratio: f64,
            content_bounds: PhysicalBounds,
            color_ranges: Vec<OpaqueSrgbRange>,
        }
        Paint {
            surface: &self.face.surface,
            pigment: &self.face.pigment,
            light: &self.light,
            edge: &self.face.edge,
            edge_width: self.face.edge_width,
            foreground: &self.face.foreground,
            content_contrast_ratio: self.face.content_contrast_ratio,
            content_bounds: self.face.content_bounds,
            color_ranges: self.paint_color_ranges(),
        }
        .serialize(serializer)
    }
}

impl MaterialReliefPaint {
    pub(crate) fn new(face: ReliefPaint, light: ResolvedMaterialLight) -> Self {
        Self { face, light }
    }

    pub(crate) fn dim_disabled_label(&mut self, bound: f64) {
        let face = &mut self.face;
        face.foreground =
            crate::disabled_label::dim_disabled_label(&face.foreground, face.pigment.body(), bound);
        face.content_contrast_ratio =
            crate::opaque_contrast_ratio(&face.foreground, face.pigment.body())
                .expect("the dimmed label and resolved body are opaque");
    }

    #[cfg(test)]
    pub(crate) fn edge_interior_contains(&self, point: PhysicalVector) -> bool {
        self.face.edge_interior_contains(point)
    }

    pub(crate) fn terms(&self) -> DirectTerms<'_> {
        self.face.terms()
    }

    pub(crate) fn sampler(&self) -> FaceSampler<'_> {
        self.face.sampler()
    }

    pub(crate) fn sampler_keeping<'a>(&'a self, shared: Option<Shared<'a>>) -> FaceSampler<'a> {
        self.face.sampler_keeping(shared)
    }

    pub(crate) fn keeping<'a>(&self, reuse: &'a ReliefReuse) -> FaceKeeping<'a> {
        reuse.keeping(&self.face)
    }

    pub(crate) fn face_color_using(
        &self,
        point: PhysicalVector,
        terms: &mut impl FaceTerms,
    ) -> Result<SrgbFallback, ReliefPaintError> {
        let normal = terms
            .relief(point)
            .map_err(|e| ReliefPaintError(e.to_string()))?
            .ok_or_else(|| ReliefPaintError("face color requires a point of the outline".into()))?
            .normal;
        terms.shade(normal, |normal| self.shade(normal))
    }

    fn shade(&self, normal: crate::FaceNormal) -> Result<SrgbFallback, ReliefPaintError> {
        self.light
            .sample(self.face.pigment(), normal.x, normal.y)
            .map(|(_, color)| color)
            .map_err(|e| ReliefPaintError(e.to_string()))
    }

    pub fn surface(&self) -> &SurfaceRelief {
        self.face.surface()
    }

    pub fn pigment(&self) -> &OpaquePigmentResult {
        self.face.pigment()
    }

    pub fn edge(&self) -> &EdgeContrastResult {
        self.face.edge()
    }

    pub fn foreground(&self) -> &SrgbFallback {
        self.face.foreground()
    }

    pub fn content_contrast_ratio(&self) -> f64 {
        self.face.content_contrast_ratio
    }

    pub fn edge_width(&self) -> f64 {
        self.face.edge_width
    }

    pub fn content_bounds(&self) -> PhysicalBounds {
        self.face.content_bounds()
    }

    pub(crate) fn uniform_contains_box(&self, bounds: PhysicalBounds) -> bool {
        self.face.uniform_contains_box(bounds)
    }

    pub fn paint_color_ranges(&self) -> Vec<OpaqueSrgbRange> {
        self.face.paint_color_ranges()
    }

    pub fn sample_paint(
        &self,
        point: PhysicalVector,
    ) -> Result<Option<SrgbFallback>, ReliefPaintError> {
        self.face
            .sample_paint_with(point, |normal| self.shade(normal))
    }

    pub(crate) fn sample_paint_using(
        &self,
        point: PhysicalVector,
        terms: &mut impl FaceTerms,
    ) -> Result<Option<SrgbFallback>, ReliefPaintError> {
        self.face
            .sample_paint_using(point, terms, |normal| self.shade(normal))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialReliefPaintResult {
    pub schema_version: &'static str,
    pub paint: MaterialReliefPaint,
    pub samples: Vec<ReliefPaintPoint>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request<'a> {
    schema_version: String,
    #[serde(borrow)]
    outline: &'a RawValue,
    profile: SurfaceReliefProfile,
    #[serde(borrow)]
    pigment: &'a RawValue,
    #[serde(borrow)]
    light: &'a RawValue,
    environment: EnvironmentSnapshot,
    enabled: bool,
    interaction_active: bool,
    #[serde(borrow)]
    edge: &'a RawValue,
    edge_width: f64,
    foreground: SrgbInput,
    minimum_content_contrast: f64,
    points: Vec<PhysicalVector>,
}

pub fn resolve_material_relief_paint_source(
    source: &str,
) -> Result<MaterialReliefPaintResult, ReliefPaintError> {
    let error = |e: &dyn fmt::Display| ReliefPaintError(e.to_string());
    let document = parse_token_document(source).map_err(|e| error(&e))?;
    for pointer in [
        "",
        "/outline",
        "/profile",
        "/pigment",
        "/light",
        "/environment",
        "/edge",
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
    let request: Request<'_> = serde_json::from_str(source).map_err(|e| error(&e))?;
    if request.schema_version != "0.1.0" {
        return Err(ReliefPaintError("schemaVersion must be 0.1.0".into()));
    }
    if request
        .points
        .iter()
        .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return Err(ReliefPaintError("points must be finite".into()));
    }
    let outline = resolve_surface_outline_source(request.outline.get()).map_err(|e| error(&e))?;
    let surface = resolve_surface_relief(outline, request.profile).map_err(|e| error(&e))?;
    let pigment = resolve_opaque_pigment_source(request.pigment.get()).map_err(|e| error(&e))?;
    let light = admit_material_light(request.light.get()).map_err(|e| error(&e))?;
    let edge = resolve_edge_contrast_source(request.edge.get()).map_err(|e| error(&e))?;
    let foreground = request.foreground.into_fallback().map_err(|e| match e {
        SrgbInputError::InvalidColorSpace => ReliefPaintError("foreground must use sRGB".into()),
        SrgbInputError::Color(e) => error(&e),
    })?;
    let face = resolve_relief_paint(
        surface,
        pigment,
        light.key_light(),
        edge,
        request.edge_width,
        foreground,
        request.minimum_content_contrast,
    )?;
    let light = ResolvedMaterialLight::resolve(
        &light,
        &request.environment,
        request.enabled,
        request.interaction_active,
    )
    .map_err(|e| error(&e))?;
    let paint = MaterialReliefPaint::new(face, light);
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
    Ok(MaterialReliefPaintResult {
        schema_version: "0.1.0",
        paint,
        samples,
    })
}
