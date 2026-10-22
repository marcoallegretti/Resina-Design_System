use crate::{
    EdgeContrastResult, ExtrudedContourResult, FocusIndicatorIr, KeyLightResult,
    MaterialReliefPaint, OutlineCurvature, PlacedContour, SrgbFallback, SurfacePaintError,
    SurfaceReliefProfile,
    material_light::{MaterialLightPolicy, ResolvedMaterialLight},
    opaque_paint::{contains, placed_contains},
    relief_paint::{FaceKeeping, FaceSampler, FaceTerms, ReliefReuse},
    resolve_extruded_contour, resolve_inset_contour, resolve_key_light, resolve_opaque_pigment,
    resolve_relief_paint, resolve_surface_outline, resolve_surface_relief,
    surface_relief::FlatnessSampler,
};
use resina_color::OpaqueSrgbRange;
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorRole, CommandPhase, FrostRepresentation, InteractionState, KeyLight, LogicalCornerRadii,
    MaterialFamily, MaterialLightChannels, MaterialRole, OpaquePigmentProfiles, PhysicalVector,
    StateSet, SurfaceForm, SurfaceSize,
};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LitControlBodyError(String);

impl fmt::Display for LitControlBodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "lit control body: {}", self.0)
    }
}
impl std::error::Error for LitControlBodyError {}

fn fail(error: impl fmt::Display) -> LitControlBodyError {
    LitControlBodyError(error.to_string())
}

impl LitControlBodyError {
    pub(crate) fn for_part(self, part: &str) -> Self {
        Self(format!("{part}: {}", self.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlRelief {
    command_body: SurfaceReliefProfile,
    toggle_track: SurfaceReliefProfile,
    toggle_thumb: SurfaceReliefProfile,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ControlReliefMembers {
    schema_version: String,
    command_body: SurfaceReliefProfile,
    toggle_track: SurfaceReliefProfile,
    toggle_thumb: SurfaceReliefProfile,
}

impl ControlRelief {
    pub fn command_body(&self) -> SurfaceReliefProfile {
        self.command_body
    }
    pub fn toggle_track(&self) -> SurfaceReliefProfile {
        self.toggle_track
    }
    pub fn toggle_thumb(&self) -> SurfaceReliefProfile {
        self.toggle_thumb
    }
}

pub fn parse_control_relief_source(source: &str) -> Result<ControlRelief, LitControlBodyError> {
    control_relief(parse_token_document(source).map_err(fail)?)
}

/// Parses one light channels object, validating the original ambient and interaction strength
/// sources even for channels that policy or phase later suppresses.
pub fn parse_light_channels_source(
    source: &str,
) -> Result<MaterialLightChannels, LitControlBodyError> {
    crate::pigment_source::validate_light_channel_source(source, &[]).map_err(fail)?;
    serde_json::from_str(source).map_err(fail)
}

impl<'de> Deserialize<'de> for ControlRelief {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        control_relief(serde_json::Value::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)
    }
}

fn control_relief(document: serde_json::Value) -> Result<ControlRelief, LitControlBodyError> {
    for pointer in ["", "/commandBody", "/toggleTrack", "/toggleThumb"] {
        if !document
            .pointer(pointer)
            .is_some_and(serde_json::Value::is_object)
        {
            let part = pointer.trim_start_matches('/');
            return Err(fail(if part.is_empty() {
                "control relief must be an object".to_owned()
            } else {
                format!("control relief {part} must be an object")
            }));
        }
    }
    let members: ControlReliefMembers = serde_json::from_value(document).map_err(fail)?;
    if members.schema_version != "0.1.0" {
        return Err(fail("control relief schemaVersion must be 0.1.0"));
    }
    for (part, profile) in [
        ("commandBody", members.command_body),
        ("toggleTrack", members.toggle_track),
        ("toggleThumb", members.toggle_thumb),
    ] {
        crate::surface_relief::validate_relief_profile(profile)
            .map_err(|error| fail(format!("control relief {part}: {error}")))?;
    }
    Ok(ControlRelief {
        command_body: members.command_body,
        toggle_track: members.toggle_track,
        toggle_thumb: members.toggle_thumb,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct LitControlIdentity {
    pub material_role: MaterialRole,
    pub color_role: ColorRole,
    pub material_family: MaterialFamily,
    pub form: SurfaceForm,
    pub states: StateSet,
    pub frost_representation: Option<FrostRepresentation>,
    pub foreground_role: ColorRole,
    pub content_fallback_applied: bool,
}

pub struct LitControlBodyInput<'a> {
    pub identity: LitControlIdentity,
    pub size: SurfaceSize,
    pub radii: LogicalCornerRadii,
    pub relief: SurfaceReliefProfile,
    pub body: &'a SrgbFallback,
    pub pigment_profiles: &'a OpaquePigmentProfiles,
    pub key_light: &'a KeyLight,
    pub light: &'a MaterialLightChannels,
    pub environment: &'a EnvironmentSnapshot,
    pub edge: EdgeContrastResult,
    pub edge_width: f64,
    pub foreground: SrgbFallback,
    pub minimum_content_contrast: f64,
    pub depth: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LitControlBodyGeometry {
    silhouette: ExtrudedContourResult,
    edge_interior: PlacedContour,
}

impl LitControlBodyGeometry {
    pub fn silhouette(&self) -> &ExtrudedContourResult {
        &self.silhouette
    }
    pub fn edge_interior(&self) -> &PlacedContour {
        &self.edge_interior
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LitControlBodyIr {
    schema_version: &'static str,
    representation: &'static str,
    material_role: MaterialRole,
    color_role: ColorRole,
    material_family: MaterialFamily,
    form: SurfaceForm,
    states: StateSet,
    #[serde(skip_serializing_if = "Option::is_none")]
    frost_representation: Option<FrostRepresentation>,
    face: MaterialReliefPaint,
    lighting: KeyLightResult,
    depth: f64,
    geometry: LitControlBodyGeometry,
    foreground_role: ColorRole,
    content_fallback_applied: bool,
}

fn lit_control_phase(states: &StateSet) -> Result<CommandPhase, LitControlBodyError> {
    if states.states().iter().any(|state| {
        !matches!(
            state,
            InteractionState::Rest
                | InteractionState::Hover
                | InteractionState::Pressed
                | InteractionState::Disabled
                | InteractionState::Focused
                | InteractionState::Checked
        )
    }) {
        return Err(fail(
            "lit control bodies support only rest, hover, pressed, disabled, focused and checked states",
        ));
    }
    Ok(if states.contains(InteractionState::Disabled) {
        CommandPhase::Disabled
    } else if states.contains(InteractionState::Pressed) {
        CommandPhase::Pressed
    } else if states.contains(InteractionState::Hover) {
        CommandPhase::Hover
    } else {
        CommandPhase::Rest
    })
}

pub fn resolve_lit_control_body(
    input: LitControlBodyInput<'_>,
) -> Result<LitControlBodyIr, LitControlBodyError> {
    let identity = input.identity;
    let phase = lit_control_phase(&identity.states)?;
    if !matches!(
        identity.material_role,
        MaterialRole::ControlPassive
            | MaterialRole::ControlInteractive
            | MaterialRole::ControlPrimary
    ) {
        return Err(fail("lit control bodies require a control material role"));
    }
    if identity.material_family == MaterialFamily::Gel {
        return Err(fail(
            "lit control bodies do not support the Gel material family",
        ));
    }
    let frost = identity.material_family == MaterialFamily::Frost;
    if identity.frost_representation.is_some() != frost
        || (identity.content_fallback_applied && !frost)
    {
        return Err(fail(
            "only Frost reports a representation or content fallback",
        ));
    }
    if !input.depth.is_finite() || input.depth < 0.0 {
        return Err(fail("depth must be finite and nonnegative"));
    }
    let outline = resolve_surface_outline(
        input.size,
        input.radii,
        input.environment.layout_direction(),
        OutlineCurvature::Circular,
        input.edge_width,
    )
    .map_err(fail)?;
    for (name, corner) in ["topLeft", "topRight", "bottomRight", "bottomLeft"]
        .into_iter()
        .zip(outline.corners())
    {
        if corner.x != corner.y {
            return Err(fail(format!(
                "{name} corner must be circular with equal extents"
            )));
        }
        let shoulder = input.relief.shoulder_width;
        let tau = 2.0 * (shoulder / corner.x + shoulder / corner.x);
        if corner.x > 0.0 && shoulder > 0.0 && (tau >= 1.0 || tau.is_nan()) {
            return Err(fail(format!(
                "{name} corner radius {} px cannot carry shoulderWidth {shoulder} px: tau must be below 1",
                corner.x
            )));
        }
    }
    let surface = resolve_surface_relief(outline, input.relief).map_err(fail)?;
    let pigment =
        resolve_opaque_pigment(identity.material_family, input.body, input.pigment_profiles)
            .map_err(fail)?;
    let face = resolve_relief_paint(
        surface,
        pigment,
        input.key_light,
        input.edge,
        input.edge_width,
        input.foreground,
        input.minimum_content_contrast,
    )
    .map_err(fail)?;
    let light = ResolvedMaterialLight::admit(
        input.key_light,
        input.light,
        MaterialLightPolicy::of(input.environment),
        matches!(phase, CommandPhase::Hover | CommandPhase::Pressed),
    )
    .map_err(fail)?;
    let lighting = resolve_key_light(input.key_light, input.depth, &[]).map_err(fail)?;
    let layout = input.environment.layout_direction();
    let offset = lighting.side_offset();
    let inset = resolve_inset_contour(input.size, input.radii, input.edge_width).map_err(fail)?;
    let geometry = LitControlBodyGeometry {
        silhouette: resolve_extruded_contour(input.size, input.radii, layout, offset)
            .map_err(fail)?,
        edge_interior: PlacedContour::new(
            PhysicalVector {
                x: input.edge_width,
                y: input.edge_width,
            },
            resolve_extruded_contour(inset.size(), inset.radii(), layout, offset).map_err(fail)?,
        ),
    };
    Ok(LitControlBodyIr {
        schema_version: "0.1.0",
        representation: "opaque",
        material_role: identity.material_role,
        color_role: identity.color_role,
        material_family: identity.material_family,
        form: identity.form,
        states: identity.states,
        frost_representation: identity.frost_representation,
        face: MaterialReliefPaint::new(face, light),
        lighting,
        depth: input.depth,
        geometry,
        foreground_role: identity.foreground_role,
        content_fallback_applied: identity.content_fallback_applied,
    })
}

impl LitControlBodyIr {
    pub(crate) fn dim_disabled_label(&mut self, bound: f64) {
        self.face.dim_disabled_label(bound);
    }
    pub fn face(&self) -> &MaterialReliefPaint {
        &self.face
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
    pub fn frost_representation(&self) -> Option<FrostRepresentation> {
        self.frost_representation
    }
    pub fn foreground_role(&self) -> ColorRole {
        self.foreground_role
    }
    pub fn content_fallback_applied(&self) -> bool {
        self.content_fallback_applied
    }
    pub fn lighting(&self) -> &KeyLightResult {
        &self.lighting
    }
    pub fn depth(&self) -> f64 {
        self.depth
    }
    pub fn geometry(&self) -> &LitControlBodyGeometry {
        &self.geometry
    }
    pub fn states(&self) -> &StateSet {
        &self.states
    }
    pub fn material_family(&self) -> MaterialFamily {
        self.material_family
    }

    pub fn paint_color_ranges(&self) -> Vec<OpaqueSrgbRange> {
        self.face.paint_color_ranges()
    }

    /// Whether every point of the closed box from `min` to `max` is certified to have the exactly
    /// flat face normal. This is an evaluation aid for realizations that skip sampling such regions;
    /// it adds nothing to `sample_paint`, which remains the definition of the body's paint.
    pub fn flat_face_over(&self, min: PhysicalVector, max: PhysicalVector) -> bool {
        self.face.surface().flat_over(min, max)
    }

    pub fn sample_paint(
        &self,
        point: PhysicalVector,
    ) -> Result<Option<SrgbFallback>, LitControlBodyError> {
        self.sample_paint_using(point, &mut self.face.terms())
    }

    /// An evaluator of [`sample_paint`](Self::sample_paint) for many points of one realization,
    /// which reuses the face terms that many points share.
    pub fn sampler(&self) -> LitControlBodySampler<'_> {
        LitControlBodySampler {
            body: self,
            face: self.face.sampler(),
        }
    }

    /// Samplers for one render of this body that reuse, and add to, the relief samples `reuse`
    /// keeps. They are equal to [`sample_paint`](Self::sample_paint) at every point.
    pub fn samplers<'a>(&'a self, reuse: &'a ReliefReuse) -> LitControlBodySamplers<'a> {
        LitControlBodySamplers {
            body: self,
            keeping: self.face.keeping(reuse),
        }
    }

    fn sample_paint_using(
        &self,
        point: PhysicalVector,
        terms: &mut impl FaceTerms,
    ) -> Result<Option<SrgbFallback>, LitControlBodyError> {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(fail("paint point must be finite"));
        }
        let offset = self.lighting.side_offset();
        if offset.x == 0.0 && offset.y == 0.0 {
            return self.face.sample_paint_using(point, terms).map_err(fail);
        }
        let swept_interior = |point| placed_contains(&self.geometry.edge_interior, point);
        if self
            .face
            .surface()
            .outline()
            .contains(point)
            .map_err(fail)?
        {
            if swept_interior(point).map_err(fail)? || terms.edge_interior_contains(point) {
                return self
                    .face
                    .face_color_using(point, terms)
                    .map(Some)
                    .map_err(fail);
            }
            return Ok(Some(self.face.edge().color().clone()));
        }
        if !contains(&self.geometry.silhouette, point).map_err(fail)? {
            return Ok(None);
        }
        if swept_interior(point).map_err(fail)? {
            Ok(Some(self.face.pigment().side().clone()))
        } else {
            Ok(Some(self.face.edge().color().clone()))
        }
    }
}

/// See [`LitControlBodyIr::samplers`]. Dropping it keeps the relief samples its samplers added.
pub struct LitControlBodySamplers<'a> {
    body: &'a LitControlBodyIr,
    keeping: FaceKeeping<'a>,
}

impl LitControlBodySamplers<'_> {
    pub fn sampler(&self) -> LitControlBodySampler<'_> {
        LitControlBodySampler {
            body: self.body,
            face: self.body.face.sampler_keeping(Some(self.keeping.shared())),
        }
    }

    /// An evaluator of [`flat_face_over`](LitControlBodyIr::flat_face_over) that reuses, and adds
    /// to, the certifications kept for this face.
    pub fn flatness(&self) -> LitControlBodyFlatness<'_> {
        LitControlBodyFlatness(self.body.face.flatness_keeping(Some(self.keeping.shared())))
    }
}

/// See [`LitControlBodySamplers::flatness`].
pub struct LitControlBodyFlatness<'a>(FlatnessSampler<'a>);

impl LitControlBodyFlatness<'_> {
    /// Equal to [`LitControlBodyIr::flat_face_over`] for every box.
    pub fn flat_face_over(&mut self, min: PhysicalVector, max: PhysicalVector) -> bool {
        self.0.flat_over(min, max)
    }
}

/// See [`LitControlBodyIr::sampler`].
pub struct LitControlBodySampler<'a> {
    body: &'a LitControlBodyIr,
    face: FaceSampler<'a>,
}

impl LitControlBodySampler<'_> {
    /// Equal to [`LitControlBodyIr::sample_paint`] at every point.
    pub fn sample_paint(
        &mut self,
        point: PhysicalVector,
    ) -> Result<Option<SrgbFallback>, LitControlBodyError> {
        self.body.sample_paint_using(point, &mut self.face)
    }
}

/// The [lit control body](LitControlBodyIr) with its independent navigation ring, present exactly
/// when the body states include `focused`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LitControlPaintIr {
    schema_version: &'static str,
    body: LitControlBodyIr,
    #[serde(skip_serializing_if = "Option::is_none")]
    focus: Option<FocusIndicatorIr>,
}

impl LitControlPaintIr {
    pub(crate) fn new(body: LitControlBodyIr, focus: Option<FocusIndicatorIr>) -> Self {
        Self {
            schema_version: "0.1.0",
            body,
            focus,
        }
    }
    pub(crate) fn body_mut(&mut self) -> &mut LitControlBodyIr {
        &mut self.body
    }
    pub fn body(&self) -> &LitControlBodyIr {
        &self.body
    }
    pub fn focus(&self) -> Option<&FocusIndicatorIr> {
        self.focus.as_ref()
    }

    /// Samples the body, then the separated focus ring where the body paints nothing.
    pub fn sample_paint(
        &self,
        point: PhysicalVector,
    ) -> Result<Option<SrgbFallback>, LitControlPaintSampleError> {
        if let Some(color) = self
            .body
            .sample_paint(point)
            .map_err(LitControlPaintSampleError::Body)?
        {
            return Ok(Some(color));
        }
        match &self.focus {
            Some(focus) => focus
                .sample_paint(point)
                .map_err(LitControlPaintSampleError::Focus),
            None => Ok(None),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LitControlPaintSampleError {
    Body(LitControlBodyError),
    Focus(SurfacePaintError),
}

impl fmt::Display for LitControlPaintSampleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Body(error) => error.fmt(f),
            Self::Focus(error) => write!(f, "lit control focus: {error}"),
        }
    }
}

impl std::error::Error for LitControlPaintSampleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Body(error) => Some(error),
            Self::Focus(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relief_paint::MAX_KEPT_TERMS;
    use serde_json::{Value, json};

    fn body(depth: f64, radii: [f64; 4], layout: &str) -> LitControlBodyIr {
        let mut source: Value = serde_json::from_str(include_str!(
            "../../../../conformance/ir/lit-control-body-source.json"
        ))
        .unwrap();
        source["environment"]["layoutDirection"] = json!(layout);
        let [top_start, top_end, bottom_end, bottom_start] =
            radii.map(|radius| resina_model::CornerRadius {
                x: radius,
                y: radius,
            });
        let environment: EnvironmentSnapshot =
            serde_json::from_value(source["environment"].clone()).unwrap();
        let body = resina_color::resolve_srgb_fallback(&source["body"]).unwrap();
        let profiles: OpaquePigmentProfiles =
            serde_json::from_value(source["pigmentProfiles"].clone()).unwrap();
        let key_light: KeyLight = serde_json::from_value(source["keyLight"].clone()).unwrap();
        let light: MaterialLightChannels = serde_json::from_value(source["light"].clone()).unwrap();
        resolve_lit_control_body(LitControlBodyInput {
            identity: LitControlIdentity {
                material_role: MaterialRole::ControlInteractive,
                color_role: serde_json::from_value(json!("surface.high")).unwrap(),
                material_family: MaterialFamily::Cast,
                form: serde_json::from_value(source["identity"]["form"].clone()).unwrap(),
                states: serde_json::from_value(source["identity"]["states"].clone()).unwrap(),
                frost_representation: None,
                foreground_role: serde_json::from_value(json!("content.primary")).unwrap(),
                content_fallback_applied: false,
            },
            size: SurfaceSize {
                width: 96.0,
                height: 48.0,
            },
            radii: LogicalCornerRadii {
                top_start,
                top_end,
                bottom_end,
                bottom_start,
            },
            relief: serde_json::from_value(source["relief"].clone()).unwrap(),
            body: &body,
            pigment_profiles: &profiles,
            key_light: &key_light,
            light: &light,
            environment: &environment,
            edge: crate::resolve_edge_contrast_source(&source["edge"].to_string()).unwrap(),
            edge_width: 1.0,
            foreground: resina_color::resolve_srgb_fallback(&source["foreground"]).unwrap(),
            minimum_content_contrast: 3.0,
            depth,
        })
        .unwrap()
    }

    #[test]
    fn reused_relief_terms_reproduce_point_paint_across_renders() {
        let bits = |color: Option<SrgbFallback>| {
            color.map(|color| {
                (
                    color.components().map(f64::to_bits),
                    color.alpha().to_bits(),
                )
            })
        };
        let reuse = ReliefReuse::default();
        let mut kept = Vec::new();
        for (depth, radii) in [
            (0.5, [20.0, 12.0, 16.0, 10.0]),
            (1.5, [20.0, 12.0, 16.0, 10.0]),
            (1.5, [24.0; 4]),
            (0.5, [20.0, 12.0, 16.0, 10.0]),
            (0.5, [20.0, 12.0, 16.0, 10.0]),
        ] {
            let body = body(depth, radii, "ltr");
            let samplers = body.samplers(&reuse);
            let mut sampler = samplers.sampler();
            let mut flatness = samplers.flatness();
            let mut flat = 0;
            for iy in 0..200 {
                for ix in 0..350 {
                    let point = PhysicalVector {
                        x: f64::from(ix) * 0.29 - 2.0 + 0.0137,
                        y: f64::from(iy) * 0.29 - 2.0 + 0.0071,
                    };
                    assert_eq!(
                        bits(sampler.sample_paint(point).unwrap()),
                        bits(body.sample_paint(point).unwrap()),
                        "{depth} {radii:?} {point:?}"
                    );
                    // Boxes of two extents share each corner point, so a box keyed by its
                    // minimum alone would return the other's certification.
                    if ix % 3 != 0 || iy % 3 != 0 {
                        continue;
                    }
                    for extent in [0.21, 2.9] {
                        let max = PhysicalVector {
                            x: point.x + extent,
                            y: point.y + extent,
                        };
                        let certified = body.flat_face_over(point, max);
                        assert_eq!(
                            flatness.flat_face_over(point, max),
                            certified,
                            "{depth} {radii:?} {point:?} {extent}"
                        );
                        flat += usize::from(certified);
                    }
                }
            }
            assert!(flat > 5_000 && flat < 12_000, "{flat}");
            drop((sampler, flatness));
            drop(samplers);
            kept.push(reuse.kept_terms().unwrap());
        }
        let [first, deeper, other, again, repeated] = kept.try_into().unwrap();
        assert!(first.1.iter().all(|count| *count > 250), "{:?}", first.1);
        // A deeper body has the same face, whose kept terms it extends where its deeper sweep
        // reaches more of the edge band.
        assert_eq!(deeper.0, first.0);
        assert!(
            deeper
                .1
                .iter()
                .zip(first.1)
                .all(|(deeper, first)| *deeper >= first)
        );
        // Another face is kept beside the first, whose terms are complete when it returns.
        assert_ne!(other.0, first.0);
        assert_eq!(again, deeper);
        assert_eq!(repeated, again);
        assert_eq!(reuse.kept_faces(), [other.0.clone(), first.0.clone()]);
    }

    #[test]
    fn reuse_keeps_the_most_recently_rendered_faces() {
        let reuse = ReliefReuse::default();
        let point = PhysicalVector { x: 3.1, y: 2.7 };
        let mut identities = Vec::new();
        for radius in [10.0, 12.0, 14.0, 16.0, 18.0, 10.0] {
            let body = body(0.5, [radius; 4], "ltr");
            let samplers = body.samplers(&reuse);
            assert_eq!(
                samplers.sampler().sample_paint(point).unwrap(),
                body.sample_paint(point).unwrap()
            );
            drop(samplers);
            identities.push(reuse.kept_terms().unwrap().0);
        }
        // The fifth face forgot the first, the least recently rendered; rendering it again keeps
        // it as the most recent and forgets the second.
        assert_ne!(identities[0], identities[1]);
        assert_eq!(identities[5], identities[0]);
        assert_eq!(reuse.kept_faces(), identities[2..].to_vec());
    }

    #[test]
    fn reuse_forgets_older_faces_to_stay_within_its_bound() {
        let reuse = ReliefReuse::default();
        let mut identities = Vec::new();
        for radii in [[20.0, 12.0, 16.0, 10.0], [24.0; 4]] {
            let body = body(0.5, radii, "ltr");
            let samplers = body.samplers(&reuse);
            let mut flatness = samplers.flatness();
            for iy in 0..400 {
                for ix in 0..400 {
                    let min = PhysicalVector {
                        x: f64::from(ix) * 0.24 + 0.0137,
                        y: f64::from(iy) * 0.12 + 0.0071,
                    };
                    let max = PhysicalVector {
                        x: min.x + 0.1,
                        y: min.y + 0.1,
                    };
                    assert_eq!(
                        flatness.flat_face_over(min, max),
                        body.flat_face_over(min, max)
                    );
                }
            }
            drop(flatness);
            drop(samplers);
            identities.push(reuse.kept_terms().unwrap());
        }
        let [first, second] = identities.try_into().unwrap();
        // Together the two faces' certifications exceed the bound, so keeping the second's
        // forgets the first, the least recently rendered.
        const { assert!(2 * 160_000 > MAX_KEPT_TERMS && 160_000 < MAX_KEPT_TERMS) };
        assert_eq!(first.1[4], 160_000);
        assert_eq!(second.1[4], 160_000);
        assert_eq!(reuse.kept_faces(), [second.0]);
    }

    #[test]
    fn circular_edge_interior_equals_the_unswept_inset_contour() {
        for (radii, layout) in [
            ([24.0; 4], "ltr"),
            ([20.0, 12.0, 16.0, 10.0], "ltr"),
            ([20.0, 12.0, 16.0, 10.0], "rtl"),
            ([0.0, 12.0, 0.0, 10.0], "rtl"),
        ] {
            let body = body(0.0, radii, layout);
            let mut inside = 0;
            for iy in -5..=245 {
                for ix in -5..=485 {
                    let point = PhysicalVector {
                        x: f64::from(ix) * 0.2 + 0.0137,
                        y: f64::from(iy) * 0.2 + 0.0071,
                    };
                    let face = body.face.edge_interior_contains(point);
                    assert_eq!(
                        face,
                        placed_contains(&body.geometry.edge_interior, point).unwrap(),
                        "{radii:?} {layout} {point:?}"
                    );
                    inside += usize::from(face);
                }
            }
            assert!(inside > 80_000);
        }
    }

    #[test]
    fn swept_regions_contain_the_face_and_its_projection() {
        let lit = body(2.0, [20.0, 12.0, 16.0, 10.0], "rtl");
        let offset = lit.lighting.side_offset();
        for iy in -10..=260 {
            for ix in -10..=500 {
                let point = PhysicalVector {
                    x: f64::from(ix) * 0.2 + 0.0113,
                    y: f64::from(iy) * 0.2 + 0.0097,
                };
                let shifted = PhysicalVector {
                    x: point.x + offset.x,
                    y: point.y + offset.y,
                };
                if lit.face.surface().outline().contains(point).unwrap() {
                    assert!(contains(&lit.geometry.silhouette, point).unwrap());
                    assert!(contains(&lit.geometry.silhouette, shifted).unwrap());
                }
                if lit.face.edge_interior_contains(point) {
                    assert!(placed_contains(&lit.geometry.edge_interior, point).unwrap());
                    assert!(placed_contains(&lit.geometry.edge_interior, shifted).unwrap());
                }
            }
        }
    }
}
