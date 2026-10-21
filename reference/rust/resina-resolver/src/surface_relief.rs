use crate::{
    OutlineCurvature, SurfaceOutline,
    relief_paint::{KeptRow, KeptSamples, Shared},
    resolve_surface_outline_source,
};
use resina_model::{PhysicalBounds, PhysicalVector};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt,
    hash::{BuildHasherDefault, Hash, Hasher},
    sync::PoisonError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReliefKind {
    Flat,
    Convex,
    Recessed,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceReliefProfile {
    pub kind: ReliefKind,
    pub body_thickness: f64,
    pub amplitude: f64,
    pub shoulder_width: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct FaceNormal {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ReliefSample {
    pub height: f64,
    pub normal: FaceNormal,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceRelief {
    outline: SurfaceOutline,
    profile: SurfaceReliefProfile,
    content_bounds: PhysicalBounds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceReliefError(String);

impl fmt::Display for SurfaceReliefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "surface relief: {}", self.0)
    }
}
impl std::error::Error for SurfaceReliefError {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReliefRequest<'a> {
    schema_version: String,
    #[serde(borrow)]
    outline: &'a serde_json::value::RawValue,
    profile: SurfaceReliefProfile,
    points: Vec<PhysicalVector>,
}

#[derive(Debug, Serialize)]
pub struct ReliefPointSample {
    pub point: PhysicalVector,
    pub sample: Option<ReliefSample>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceReliefResult {
    pub schema_version: &'static str,
    pub surface: SurfaceRelief,
    pub samples: Vec<ReliefPointSample>,
}

pub fn resolve_surface_relief_source(
    source: &str,
) -> Result<SurfaceReliefResult, SurfaceReliefError> {
    let document = parse_token_document(source).map_err(|e| SurfaceReliefError(e.to_string()))?;
    for pointer in ["", "/outline", "/profile"] {
        if !document
            .pointer(pointer)
            .is_some_and(serde_json::Value::is_object)
        {
            return Err(SurfaceReliefError(format!("{pointer} must be an object")));
        }
    }
    let points = document
        .get("points")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| SurfaceReliefError("points must be an array".into()))?;
    if points.iter().any(|p| !p.is_object()) {
        return Err(SurfaceReliefError(
            "points must contain vector objects".into(),
        ));
    }
    let request: ReliefRequest<'_> =
        serde_json::from_str(source).map_err(|e| SurfaceReliefError(e.to_string()))?;
    if request.schema_version != "0.1.0" {
        return Err(SurfaceReliefError("schemaVersion must be 0.1.0".into()));
    }
    let outline = resolve_surface_outline_source(request.outline.get())
        .map_err(|e| SurfaceReliefError(e.to_string()))?;
    let surface = resolve_surface_relief(outline, request.profile)?;
    let samples = request
        .points
        .into_iter()
        .map(|point| {
            Ok(ReliefPointSample {
                point,
                sample: surface.sample(point)?,
            })
        })
        .collect::<Result<Vec<_>, SurfaceReliefError>>()?;
    Ok(SurfaceReliefResult {
        schema_version: "0.1.0",
        surface,
        samples,
    })
}

pub(crate) fn validate_relief_profile(
    profile: SurfaceReliefProfile,
) -> Result<(), SurfaceReliefError> {
    let fail = |s: &str| SurfaceReliefError(s.into());
    if !profile.body_thickness.is_finite() || profile.body_thickness <= 0.0 {
        return Err(fail("bodyThickness must be finite and positive"));
    }
    if !profile.amplitude.is_finite()
        || profile.amplitude < 0.0
        || !profile.shoulder_width.is_finite()
        || profile.shoulder_width < 0.0
    {
        return Err(fail(
            "amplitude and shoulderWidth must be finite and nonnegative",
        ));
    }
    if profile.kind == ReliefKind::Flat {
        if profile.amplitude != 0.0 || profile.shoulder_width != 0.0 {
            return Err(fail("flat requires zero amplitude and shoulderWidth"));
        }
        return Ok(());
    }
    if profile.amplitude == 0.0
        || profile.shoulder_width == 0.0
        || profile.amplitude * 2.0 > profile.shoulder_width
    {
        return Err(fail(
            "relief requires positive amplitude no greater than shoulderWidth/2",
        ));
    }
    if profile.amplitude / profile.shoulder_width == 0.0 {
        return Err(fail("amplitude/shoulderWidth must remain representable"));
    }
    let thickness = match profile.kind {
        ReliefKind::Convex => profile.body_thickness + profile.amplitude,
        ReliefKind::Recessed => profile.body_thickness - profile.amplitude,
        ReliefKind::Flat => unreachable!(),
    };
    if !thickness.is_finite() || thickness <= 0.0 || thickness == profile.body_thickness {
        return Err(fail(
            "relieved thickness must be representable and positive",
        ));
    }
    Ok(())
}

pub fn resolve_surface_relief(
    outline: SurfaceOutline,
    profile: SurfaceReliefProfile,
) -> Result<SurfaceRelief, SurfaceReliefError> {
    let fail = |s: &str| SurfaceReliefError(s.into());
    validate_relief_profile(profile)?;
    let mut result = SurfaceRelief {
        content_bounds: outline.content_bounds(),
        outline,
        profile,
    };
    if profile.kind == ReliefKind::Flat {
        return Ok(result);
    }
    let content = result.content_bounds;
    let size = result.outline.size();
    if size.width - profile.shoulder_width == size.width
        || size.height - profile.shoulder_width == size.height
    {
        return Err(fail(
            "shoulderWidth must remain representable in surface coordinates",
        ));
    }
    let mut pads = [
        content.x,
        (size.width - content.x) - content.width,
        content.y,
        (size.height - content.y) - content.height,
    ]
    .map(|p| p.max(profile.shoulder_width));
    for (i, r) in result.outline.corners().into_iter().enumerate() {
        if r.x == 0.0 {
            continue;
        }
        let (_, _, tau) = result.corner_ratios(r.x, r.y)?;
        let base = ((1.0 - tau) * 0.5).sqrt();
        let diagonal =
            1.0 - match result.outline.curvature() {
                OutlineCurvature::Circular => base,
                OutlineCurvature::Quartic => base.sqrt(),
            } + 8.0 * f64::EPSILON;
        let (side_x, side_y) = match i {
            0 => (0, 2),
            1 => (1, 2),
            2 => (1, 3),
            _ => (0, 3),
        };
        pads[side_x] = pads[side_x].max((r.x * diagonal).next_up());
        pads[side_y] = pads[side_y].max((r.y * diagonal).next_up());
    }
    let [left, right, top, bottom] = pads;
    let width = (size.width - right) - left;
    let height = (size.height - bottom) - top;
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || size.width - left == size.width
        || size.width - right == size.width
        || size.height - top == size.height
        || size.height - bottom == size.height
    {
        return Err(fail(
            "content plateau must have representable positive area",
        ));
    }
    result.content_bounds = PhysicalBounds {
        x: left,
        y: top,
        width,
        height,
    };
    for x in [left, left + width] {
        for y in [top, top + height] {
            let sample = result.sample(PhysicalVector { x, y })?;
            if sample
                != Some(ReliefSample {
                    height: result.signed_amplitude(),
                    normal: FaceNormal {
                        x: 0.0,
                        y: 0.0,
                        z: 1.0,
                    },
                })
            {
                return Err(fail("content plateau containment is unrepresentable"));
            }
        }
    }
    Ok(result)
}

impl SurfaceRelief {
    pub fn outline(&self) -> &SurfaceOutline {
        &self.outline
    }
    pub fn profile(&self) -> SurfaceReliefProfile {
        self.profile
    }
    pub fn content_bounds(&self) -> PhysicalBounds {
        self.content_bounds
    }

    fn signed_amplitude(&self) -> f64 {
        if self.profile.kind == ReliefKind::Recessed {
            -self.profile.amplitude
        } else {
            self.profile.amplitude
        }
    }

    fn exponent(&self) -> f64 {
        match self.outline.curvature() {
            OutlineCurvature::Circular => 2.0,
            OutlineCurvature::Quartic => 4.0,
        }
    }

    fn corner_ratios(&self, a: f64, b: f64) -> Result<(f64, f64, f64), SurfaceReliefError> {
        let rx = self.profile.shoulder_width / a;
        let ry = self.profile.shoulder_width / b;
        let tau = self.exponent() * (rx + ry);
        if rx <= 0.0 || ry <= 0.0 || !tau.is_finite() || tau <= 0.0 || tau >= 1.0 {
            return Err(SurfaceReliefError(
                "corner shoulder ratios must be representable with 0 < tau < 1".into(),
            ));
        }
        Ok((rx, ry, tau))
    }

    /// Whether every point of the closed box from `min` to `max` is certified to have the exactly
    /// flat normal, by evaluating every gate over the box with outward-rounded intervals.
    pub(crate) fn flat_over(&self, min: PhysicalVector, max: PhysicalVector) -> bool {
        if self.profile.kind == ReliefKind::Flat {
            return true;
        }
        if ![min.x, min.y, max.x, max.y].iter().all(|v| v.is_finite())
            || min.x > max.x
            || min.y > max.y
        {
            return false;
        }
        let span = |lower: f64, upper: f64| SampleNumber {
            value: lower,
            lower,
            upper,
        };
        let size = self.outline.size();
        let w = SampleNumber::exact(self.profile.shoulder_width);
        let px = span(min.x, max.x);
        let py = span(min.y, max.y);
        let right = SampleNumber::exact(size.width) - px;
        let bottom = SampleNumber::exact(size.height) - py;
        if [px, right, py, bottom].iter().any(|d| (*d / w).lower < 1.0) {
            return false;
        }
        let one = SampleNumber::exact(1.0);
        let corners = [(px, py), (right, py), (right, bottom), (px, bottom)];
        self.outline
            .corners()
            .into_iter()
            .zip(corners)
            .all(|(r, (x, y))| {
                if r.x == 0.0 {
                    return true;
                }
                let a = SampleNumber::exact(r.x);
                let b = SampleNumber::exact(r.y);
                let tau = SampleNumber::exact(self.exponent()) * (w / a + w / b);
                let u = ((a - x) / a).clamp(0.0, 1.0);
                let v = ((b - y) / b).clamp(0.0, 1.0);
                let (up, vp) = match self.outline.curvature() {
                    OutlineCurvature::Circular => (u * u, v * v),
                    OutlineCurvature::Quartic => (u * u * u * u, v * v * v * v),
                };
                (((one - up) - vp) / tau).lower >= 1.0
            })
    }

    pub fn sample(
        &self,
        point: PhysicalVector,
    ) -> Result<Option<ReliefSample>, SurfaceReliefError> {
        if !self
            .outline
            .contains(point)
            .map_err(|e| SurfaceReliefError(e.to_string()))?
        {
            return Ok(None);
        }
        if self.profile.kind == ReliefKind::Flat {
            return Ok(Some(ReliefSample {
                height: 0.0,
                normal: FaceNormal {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
            }));
        }
        self.relief(
            &self.axis(point.x, Axis::X),
            &self.axis(point.y, Axis::Y),
            |corner, u, v| self.corner_gate(corner, u, v),
        )
        .map(Some)
    }

    /// A sampler that reuses the axes and corner samples `shared` keeps for this relief, and adds
    /// the ones it evaluates.
    pub(crate) fn sampler_keeping<'a>(&'a self, shared: Option<Shared<'a>>) -> ReliefSampler<'a> {
        let size = self.outline.size();
        let kept = shared.map(|shared| &shared.kept.relief_axes);
        ReliefSampler {
            relief: self,
            shared,
            kept_rows: Recent::default(),
            added: Vec::new(),
            neutral: (self.profile.kind != ReliefKind::Flat).then(|| {
                [
                    self.axis(size.width / 2.0, Axis::X),
                    self.axis(size.height / 2.0, Axis::Y),
                ]
            }),
            axes: [
                Recalled::new(kept.map(|axes| &axes[0])),
                Recalled::new(kept.map(|axes| &axes[1])),
            ],
            corner_gates: Default::default(),
            lines: Default::default(),
        }
    }

    fn axis(&self, coordinate: f64, axis: Axis) -> ReliefAxis {
        let size = self.outline.size();
        let w = SampleNumber::exact(self.profile.shoulder_width);
        let near = SampleNumber::exact(coordinate);
        let straight = |distance: SampleNumber, x: f64, y: f64| {
            let (value, derivative) = (distance / w).smooth();
            (value, derivative * x.into(), derivative * y.into())
        };
        let (gates, distances) = match axis {
            Axis::X => {
                let far = SampleNumber::exact(size.width) - near;
                (
                    [straight(near, 1.0, 0.0), straight(far, -1.0, 0.0)],
                    [near, far, far, near],
                )
            }
            Axis::Y => {
                let far = SampleNumber::exact(size.height) - near;
                (
                    [straight(near, 0.0, 1.0), straight(far, 0.0, -1.0)],
                    [near, near, far, far],
                )
            }
        };
        let mut corners = [SampleNumber::exact(0.0); 4];
        for ((corner, r), distance) in corners
            .iter_mut()
            .zip(self.outline.corners())
            .zip(distances)
        {
            if r.x == 0.0 {
                continue;
            }
            let a = SampleNumber::exact(match axis {
                Axis::X => r.x,
                Axis::Y => r.y,
            });
            *corner = ((a - distance) / a).clamp(0.0, 1.0);
        }
        ReliefAxis { gates, corners }
    }

    fn corner_gate(&self, corner: usize, u: SampleNumber, v: SampleNumber) -> Gate {
        let r = self.outline.corners()[corner];
        let (sx, sy) = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)][corner];
        let w = SampleNumber::exact(self.profile.shoulder_width);
        let one = SampleNumber::exact(1.0);
        let a = SampleNumber::exact(r.x);
        let b = SampleNumber::exact(r.y);
        let rx = w / a;
        let ry = w / b;
        let n = SampleNumber::exact(self.exponent());
        let tau = n * (rx + ry);
        let (up, vp, du, dv) = match self.outline.curvature() {
            OutlineCurvature::Circular => (u * u, v * v, u, v),
            OutlineCurvature::Quartic => (u * u * u * u, v * v * v * v, u * u * u, v * v * v),
        };
        let f = (one - up) - vp;
        let (value, derivative) = (f / tau).smooth();
        (
            value,
            derivative * n * du * (rx / tau) * sx.into(),
            derivative * n * dv * (ry / tau) * sy.into(),
        )
    }

    fn relief(
        &self,
        x: &ReliefAxis,
        y: &ReliefAxis,
        mut corner_gate: impl FnMut(usize, SampleNumber, SampleNumber) -> Gate,
    ) -> Result<ReliefSample, SurfaceReliefError> {
        let w = SampleNumber::exact(self.profile.shoulder_width);
        let one = SampleNumber::exact(1.0);
        let zero = SampleNumber::exact(0.0);
        let mut gates = [
            x.gates[0],
            x.gates[1],
            y.gates[0],
            y.gates[1],
            (one, zero, zero),
            (one, zero, zero),
            (one, zero, zero),
            (one, zero, zero),
        ];
        for (i, r) in self.outline.corners().into_iter().enumerate() {
            if r.x != 0.0 {
                gates[i + 4] = corner_gate(i, x.corners[i], y.corners[i]);
            }
        }
        let value = gates.iter().fold(one, |product, g| product * g.0);
        let (mut dx, mut dy) = (zero, zero);
        for (i, gate) in gates.iter().enumerate() {
            // Skipping an exact-zero derivative is bit-identical: its product is an exact zero,
            // which the `is_exact(0.0)` short-circuits of `+` and `-` below discard.
            let (x_term, y_term) = (!gate.1.is_exact(0.0), !gate.2.is_exact(0.0));
            if !x_term && !y_term {
                continue;
            }
            let product = gates
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .fold(one, |product, (_, g)| product * g.0);
            if x_term {
                dx = dx + gate.1 * product;
            }
            if y_term {
                dy = dy + gate.2 * product;
            }
        }
        let amplitude = SampleNumber::exact(self.signed_amplitude());
        let ratio = amplitude / w;
        dx = dx * ratio;
        dy = dy * ratio;
        let length = (dx * dx + dy * dy + one).sqrt();
        Ok(ReliefSample {
            height: (amplitude * value).certify()?,
            normal: FaceNormal {
                x: ((zero - dx) / length).certify()?,
                y: ((zero - dy) / length).certify()?,
                z: (one / length).certify()?,
            },
        })
    }
}

#[derive(Clone, Copy)]
#[repr(usize)]
enum Axis {
    X,
    Y,
}

type Gate = (SampleNumber, SampleNumber, SampleNumber);

#[derive(Clone, Copy)]
pub(crate) struct ReliefAxis {
    gates: [Gate; 2],
    corners: [SampleNumber; 4],
}

impl ReliefAxis {
    fn identical(&self, other: &Self) -> bool {
        self.gates
            .iter()
            .zip(&other.gates)
            .all(|(a, b)| a.0.identical(b.0) && a.1.identical(b.1) && a.2.identical(b.2))
            && self
                .corners
                .iter()
                .zip(&other.corners)
                .all(|(a, b)| a.identical(*b))
    }
}

const RECENT: usize = 8;

/// Evaluates [`SurfaceRelief::sample`] for many points. A sample whose terms along one axis are
/// bit-identical to those at the surface midline is a function of its other coordinate alone, so
/// it is evaluated once per such coordinate.
pub(crate) struct ReliefSampler<'a> {
    relief: &'a SurfaceRelief,
    shared: Option<Shared<'a>>,
    kept_rows: Recent<(u64, Option<&'a KeptRow>)>,
    added: Vec<([u64; 2], ReliefSample)>,
    neutral: Option<[ReliefAxis; 2]>,
    axes: [Recalled<'a, ReliefAxis>; 2],
    corner_gates: [Recent<(SampleNumber, SampleNumber, Gate)>; 4],
    lines: [CoordinateMap<Result<ReliefSample, SurfaceReliefError>>; 2],
}

impl ReliefSampler<'_> {
    pub(crate) fn sample(
        &mut self,
        point: PhysicalVector,
    ) -> Result<Option<ReliefSample>, SurfaceReliefError> {
        let relief = self.relief;
        let Some(neutral) = &self.neutral else {
            return relief.sample(point);
        };
        if !relief
            .outline
            .contains(point)
            .map_err(|e| SurfaceReliefError(e.to_string()))?
        {
            return Ok(None);
        }
        let [x_axes, y_axes] = &mut self.axes;
        let x = x_axes.get(point.x, || relief.axis(point.x, Axis::X));
        let y = y_axes.get(point.y, || relief.axis(point.y, Axis::Y));
        let (line, coordinate) = if x.identical(&neutral[0]) {
            (1, point.y)
        } else if y.identical(&neutral[1]) {
            (0, point.x)
        } else {
            let key = [point.x.to_bits(), point.y.to_bits()];
            if let Some(shared) = self.shared {
                let samples: &KeptSamples = &shared.kept.samples;
                let (_, row) = self.kept_rows.recall(
                    |(y, _)| *y == key[1],
                    || (key[1], samples.get(&key[1]).map(Vec::as_slice)),
                );
                if let Some(row) = row
                    && let Ok(found) = row.binary_search_by_key(&key[0], |(x, _)| *x)
                {
                    return Ok(Some(row[found].1));
                }
            }
            let gates = &mut self.corner_gates;
            let sample = relief.relief(x, y, |corner, u, v| {
                gates[corner]
                    .recall(
                        |(cu, cv, _)| cu.identical(u) && cv.identical(v),
                        || (u, v, relief.corner_gate(corner, u, v)),
                    )
                    .2
            })?;
            if self.shared.is_some() {
                self.added.push((key, sample));
            }
            return Ok(Some(sample));
        };
        let bits = coordinate.to_bits();
        if let Some(sample) = self
            .shared
            .and_then(|shared| shared.kept.lines[line].get(&bits))
        {
            return Ok(Some(*sample));
        }
        self.lines[line]
            .entry(bits)
            .or_insert_with(|| relief.relief(x, y, |corner, u, v| relief.corner_gate(corner, u, v)))
            .clone()
            .map(Some)
    }
}

impl Drop for ReliefSampler<'_> {
    fn drop(&mut self) {
        let Some(shared) = self.shared else {
            return;
        };
        let [x, y] = &mut self.axes;
        let axes = [x.take_added(), y.take_added()];
        // Only evaluated samples are kept, so a failing one is diagnosed again by a later render.
        let lines = self.lines.each_mut().map(|line| {
            line.drain()
                .filter_map(|(bits, sample)| Some((bits, sample.ok()?)))
                .collect::<Vec<_>>()
        });
        if !self.added.is_empty()
            || axes.iter().any(|added| !added.is_empty())
            || lines.iter().any(|added| !added.is_empty())
        {
            let mut added = shared.added.lock().unwrap_or_else(PoisonError::into_inner);
            added.samples.append(&mut self.added);
            for (kept, mut axis) in added.relief_axes.iter_mut().zip(axes) {
                kept.append(&mut axis);
            }
            for (kept, mut line) in added.lines.iter_mut().zip(lines) {
                kept.append(&mut line);
            }
        }
    }
}

/// The exact bits of a sample box's `min.x`, `min.y`, `max.x` and `max.y`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct BoxKey([u64; 4]);

impl Hash for BoxKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let [a, b, c, d] = self.0;
        state.write_u64(
            a.rotate_left(48) ^ b.rotate_left(32) ^ c.rotate_left(16) ^ d ^ (a ^ c).rotate_left(5),
        );
    }
}

/// Flat-box certifications by the exact bits of their boxes.
pub(crate) type FlatBoxes = HashMap<BoxKey, bool, BuildHasherDefault<CoordinateHasher>>;

/// Evaluates [`SurfaceRelief::flat_over`] for many boxes, reusing the certifications `shared`
/// keeps for this relief and adding the ones it evaluates.
pub(crate) struct FlatnessSampler<'a> {
    relief: &'a SurfaceRelief,
    shared: Option<Shared<'a>>,
    added: Vec<(BoxKey, bool)>,
}

impl SurfaceRelief {
    pub(crate) fn flatness_keeping<'a>(
        &'a self,
        shared: Option<Shared<'a>>,
    ) -> FlatnessSampler<'a> {
        FlatnessSampler {
            relief: self,
            shared,
            added: Vec::new(),
        }
    }
}

impl FlatnessSampler<'_> {
    pub(crate) fn flat_over(&mut self, min: PhysicalVector, max: PhysicalVector) -> bool {
        if self.relief.profile.kind == ReliefKind::Flat {
            return true;
        }
        let Some(shared) = self.shared else {
            return self.relief.flat_over(min, max);
        };
        let key = BoxKey([min.x, min.y, max.x, max.y].map(f64::to_bits));
        if let Some(flat) = shared.kept.flat_boxes.get(&key) {
            return *flat;
        }
        let flat = self.relief.flat_over(min, max);
        self.added.push((key, flat));
        flat
    }
}

impl Drop for FlatnessSampler<'_> {
    fn drop(&mut self) {
        if let Some(shared) = self.shared
            && !self.added.is_empty()
        {
            let mut added = shared.added.lock().unwrap_or_else(PoisonError::into_inner);
            added.flat_boxes.append(&mut self.added);
        }
    }
}

/// A map keyed by the bits of a sample coordinate.
pub(crate) type CoordinateMap<V> = HashMap<u64, V, BuildHasherDefault<CoordinateHasher>>;

/// Mixes the bits of a sample coordinate (the MurmurHash3 finalizer). Sample coordinates are not
/// adversarial, and SipHash would cost more than the evaluations some of these maps replace.
#[derive(Default)]
pub(crate) struct CoordinateHasher(u64);

impl Hasher for CoordinateHasher {
    fn finish(&self) -> u64 {
        let mut bits = self.0;
        bits ^= bits >> 33;
        bits = bits.wrapping_mul(0xff51_afd7_ed55_8ccd);
        bits ^= bits >> 33;
        bits = bits.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        bits ^ (bits >> 33)
    }
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = self.0.rotate_left(8) ^ u64::from(*byte);
        }
    }
    fn write_u64(&mut self, bits: u64) {
        self.0 ^= bits;
    }
}

/// Evaluations of a function of one coordinate, keyed by the coordinate's bits: those kept from
/// earlier renders, then those of this sampler. The rows of a raster revisit the same sample
/// coordinates, so every evaluation is kept, behind a window of the most recent ones that serves
/// the coordinates of the current pixel.
pub(crate) struct Recalled<'a, T> {
    recent: Recent<(u64, Slot<'a, T>)>,
    kept: Option<&'a CoordinateMap<T>>,
    slots: CoordinateMap<usize>,
    values: Vec<(u64, T)>,
}

enum Slot<'a, T> {
    Kept(&'a T),
    Evaluated(usize),
}

impl<T> Clone for Slot<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Slot<'_, T> {}

impl<'a, T> Recalled<'a, T> {
    pub(crate) fn new(kept: Option<&'a CoordinateMap<T>>) -> Self {
        Self {
            recent: Recent::default(),
            kept,
            slots: CoordinateMap::default(),
            values: Vec::new(),
        }
    }

    pub(crate) fn get(&mut self, coordinate: f64, evaluate: impl FnOnce() -> T) -> &T {
        let key = coordinate.to_bits();
        let (kept, slots, values) = (self.kept, &mut self.slots, &mut self.values);
        let (_, slot) = self.recent.recall(
            |(bits, _)| *bits == key,
            || {
                if let Some(value) = kept.and_then(|kept| kept.get(&key)) {
                    return (key, Slot::Kept(value));
                }
                let slot = *slots.entry(key).or_insert_with(|| {
                    values.push((key, evaluate()));
                    values.len() - 1
                });
                (key, Slot::Evaluated(slot))
            },
        );
        match slot {
            Slot::Kept(value) => value,
            Slot::Evaluated(slot) => &self.values[slot].1,
        }
    }

    /// The evaluations this sampler added, which a render keeps.
    pub(crate) fn take_added(&mut self) -> Vec<(u64, T)> {
        self.recent = Recent::default();
        self.slots.clear();
        std::mem::take(&mut self.values)
    }
}

/// The [`RECENT`] most recent evaluations, of which the oldest is replaced by a new one.
pub(crate) struct Recent<T> {
    entries: Vec<T>,
    oldest: usize,
}

impl<T> Default for Recent<T> {
    fn default() -> Self {
        Self {
            entries: Vec::with_capacity(RECENT),
            oldest: 0,
        }
    }
}

impl<T: Copy> Recent<T> {
    /// The entry that `matches`, evaluating and keeping a new one when none does.
    pub(crate) fn recall(
        &mut self,
        matches: impl Fn(&T) -> bool,
        evaluate: impl FnOnce() -> T,
    ) -> T {
        if let Some(found) = self.entries.iter().find(|entry| matches(entry)) {
            return *found;
        }
        let entry = evaluate();
        if self.entries.len() < RECENT {
            self.entries.push(entry);
        } else {
            self.entries[self.oldest] = entry;
            self.oldest = (self.oldest + 1) % RECENT;
        }
        entry
    }
}

fn smooth(t: f64) -> (f64, f64) {
    if t <= 0.0 {
        (0.0, 0.0)
    } else if t >= 1.0 {
        (1.0, 0.0)
    } else {
        (
            (t * t * t * (10.0 + t * (-15.0 + 6.0 * t))).clamp(0.0, 1.0),
            30.0 * t * t * (1.0 - t) * (1.0 - t),
        )
    }
}

// Outward rounding encloses the analytic result for the represented inputs.
#[derive(Clone, Copy)]
struct SampleNumber {
    value: f64,
    lower: f64,
    upper: f64,
}

impl From<f64> for SampleNumber {
    fn from(value: f64) -> Self {
        Self::exact(value)
    }
}

impl SampleNumber {
    fn exact(value: f64) -> Self {
        Self {
            value,
            lower: value,
            upper: value,
        }
    }

    fn rounded(value: f64, lower: f64, upper: f64) -> Self {
        if lower.is_nan() || upper.is_nan() {
            return Self {
                value,
                lower: f64::NEG_INFINITY,
                upper: f64::INFINITY,
            };
        }
        Self {
            value,
            lower: lower.next_down(),
            upper: upper.next_up(),
        }
    }

    fn identical(self, other: Self) -> bool {
        self.value.to_bits() == other.value.to_bits()
            && self.lower.to_bits() == other.lower.to_bits()
            && self.upper.to_bits() == other.upper.to_bits()
    }

    fn is_exact(self, value: f64) -> bool {
        self.lower == value && self.upper == value
    }

    fn clamp(self, lower: f64, upper: f64) -> Self {
        Self {
            value: self.value.clamp(lower, upper),
            lower: self.lower.clamp(lower, upper),
            upper: self.upper.clamp(lower, upper),
        }
    }

    fn sqrt(self) -> Self {
        Self::rounded(
            self.value.sqrt(),
            self.lower.max(0.0).sqrt(),
            self.upper.sqrt(),
        )
    }

    fn smooth(self) -> (Self, Self) {
        let (value, derivative) = smooth(self.value);
        let t = self.clamp(0.0, 1.0);
        let polynomial = |x: f64| {
            if x == 0.0 || x == 1.0 {
                return Self::exact(x);
            }
            let x = Self::exact(x);
            x * x * x * (Self::exact(10.0) + x * (Self::exact(-15.0) + Self::exact(6.0) * x))
        };
        let derivative_at = |x: f64| {
            if x == 0.0 || x == 1.0 {
                return Self::exact(0.0);
            }
            let x = Self::exact(x);
            let rest = Self::exact(1.0) - x;
            Self::exact(30.0) * x * x * rest * rest
        };
        let low_value = polynomial(t.lower).clamp(0.0, 1.0);
        let high_value = polynomial(t.upper).clamp(0.0, 1.0);
        let low_derivative = derivative_at(t.lower).clamp(0.0, 15.0 / 8.0);
        let high_derivative = derivative_at(t.upper).clamp(0.0, 15.0 / 8.0);
        (
            Self {
                value,
                lower: low_value.lower,
                upper: high_value.upper,
            },
            Self {
                value: derivative,
                lower: low_derivative.lower.min(high_derivative.lower),
                upper: if t.lower <= 0.5 && t.upper >= 0.5 {
                    15.0 / 8.0
                } else {
                    low_derivative.upper.max(high_derivative.upper)
                },
            },
        )
    }

    fn certify(self) -> Result<f64, SurfaceReliefError> {
        let minimum_magnitude = if self.lower <= 0.0 && self.upper >= 0.0 {
            0.0
        } else {
            self.lower.abs().min(self.upper.abs())
        };
        let tolerance = 1e-12_f64.max(minimum_magnitude * 1e-12).next_down();
        if !self.value.is_finite()
            || !self.lower.is_finite()
            || !self.upper.is_finite()
            || (self.value - self.lower).next_up() > tolerance
            || (self.upper - self.value).next_up() > tolerance
        {
            return Err(SurfaceReliefError(
                "sample precision cannot certify height and normal within 1e-12".into(),
            ));
        }
        Ok(self.value)
    }
}

impl std::ops::Add for SampleNumber {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        if self.is_exact(0.0) {
            return rhs;
        }
        if rhs.is_exact(0.0) {
            return self;
        }
        Self::rounded(
            self.value + rhs.value,
            self.lower + rhs.lower,
            self.upper + rhs.upper,
        )
    }
}
impl std::ops::Sub for SampleNumber {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        if rhs.is_exact(0.0) {
            return self;
        }
        if self.lower == self.upper && rhs.lower == rhs.upper && self.value == rhs.value {
            return Self::exact(0.0);
        }
        let bound = |a: f64, b: f64, lower: bool| {
            let value = a - b;
            // Subnormal differences and Sterbenz subtraction are exact.
            if (a.abs() < f64::MIN_POSITIVE && b.abs() < f64::MIN_POSITIVE)
                || (a >= 0.0 && b >= 0.0 && a <= 2.0 * b && b <= 2.0 * a)
            {
                value
            } else if lower {
                value.next_down()
            } else {
                value.next_up()
            }
        };
        Self {
            value: self.value - rhs.value,
            lower: bound(self.lower, rhs.upper, true),
            upper: bound(self.upper, rhs.lower, false),
        }
    }
}
impl std::ops::Mul for SampleNumber {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        if self.is_exact(0.0) || rhs.is_exact(0.0) {
            return Self::exact(self.value * rhs.value);
        }
        if self.is_exact(1.0) {
            return rhs;
        }
        if rhs.is_exact(1.0) {
            return self;
        }
        let products = [
            self.lower * rhs.lower,
            self.lower * rhs.upper,
            self.upper * rhs.lower,
            self.upper * rhs.upper,
        ];
        if products.iter().any(|p| p.is_nan()) {
            return Self::rounded(self.value * rhs.value, f64::NEG_INFINITY, f64::INFINITY);
        }
        Self::rounded(
            self.value * rhs.value,
            products.into_iter().fold(f64::INFINITY, f64::min),
            products.into_iter().fold(f64::NEG_INFINITY, f64::max),
        )
    }
}
impl std::ops::Div for SampleNumber {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        if rhs.is_exact(1.0) {
            return self;
        }
        if rhs.lower <= 0.0 && rhs.upper >= 0.0 {
            return Self::rounded(self.value / rhs.value, f64::NEG_INFINITY, f64::INFINITY);
        }
        let quotients = [
            self.lower / rhs.lower,
            self.lower / rhs.upper,
            self.upper / rhs.lower,
            self.upper / rhs.upper,
        ];
        if quotients.iter().any(|p| p.is_nan()) {
            return Self::rounded(self.value / rhs.value, f64::NEG_INFINITY, f64::INFINITY);
        }
        Self::rounded(
            self.value / rhs.value,
            quotients.into_iter().fold(f64::INFINITY, f64::min),
            quotients.into_iter().fold(f64::NEG_INFINITY, f64::max),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve_surface_outline;
    use resina_environment::LayoutDirection;
    use resina_model::{CornerRadius, LogicalCornerRadii, SurfaceSize};

    fn relief(
        kind: ReliefKind,
        curvature: OutlineCurvature,
        radius: CornerRadius,
    ) -> SurfaceRelief {
        let outline = resolve_surface_outline(
            SurfaceSize {
                width: 120.0,
                height: 60.0,
            },
            LogicalCornerRadii {
                top_start: radius,
                top_end: radius,
                bottom_end: radius,
                bottom_start: radius,
            },
            LayoutDirection::Ltr,
            curvature,
            2.0,
        )
        .unwrap();
        resolve_surface_relief(
            outline,
            SurfaceReliefProfile {
                kind,
                body_thickness: 4.0,
                amplitude: 0.5,
                shoulder_width: 1.0,
            },
        )
        .unwrap()
    }

    fn unsplit_sample(
        relief: &SurfaceRelief,
        point: PhysicalVector,
    ) -> Result<Option<ReliefSample>, SurfaceReliefError> {
        if !relief
            .outline
            .contains(point)
            .map_err(|e| SurfaceReliefError(e.to_string()))?
        {
            return Ok(None);
        }
        if relief.profile.kind == ReliefKind::Flat {
            return Ok(Some(ReliefSample {
                height: 0.0,
                normal: FaceNormal {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
            }));
        }
        let size = relief.outline.size();
        let w = SampleNumber::exact(relief.profile.shoulder_width);
        let px = SampleNumber::exact(point.x);
        let py = SampleNumber::exact(point.y);
        let right = SampleNumber::exact(size.width) - px;
        let bottom = SampleNumber::exact(size.height) - py;
        let straight = |distance: SampleNumber, x: f64, y: f64| {
            let (value, derivative) = (distance / w).smooth();
            (value, derivative * x.into(), derivative * y.into())
        };
        let one = SampleNumber::exact(1.0);
        let zero = SampleNumber::exact(0.0);
        let mut gates = [
            straight(px, 1.0, 0.0),
            straight(right, -1.0, 0.0),
            straight(py, 0.0, 1.0),
            straight(bottom, 0.0, -1.0),
            (one, zero, zero),
            (one, zero, zero),
            (one, zero, zero),
            (one, zero, zero),
        ];
        let corners = [
            (px, py, 1.0, 1.0),
            (right, py, -1.0, 1.0),
            (right, bottom, -1.0, -1.0),
            (px, bottom, 1.0, -1.0),
        ];
        for (i, (r, (x, y, sx, sy))) in relief
            .outline
            .corners()
            .into_iter()
            .zip(corners)
            .enumerate()
        {
            if r.x == 0.0 {
                continue;
            }
            let a = SampleNumber::exact(r.x);
            let b = SampleNumber::exact(r.y);
            let rx = w / a;
            let ry = w / b;
            let n = SampleNumber::exact(relief.exponent());
            let tau = n * (rx + ry);
            let u = ((a - x) / a).clamp(0.0, 1.0);
            let v = ((b - y) / b).clamp(0.0, 1.0);
            let (up, vp, du, dv) = match relief.outline.curvature() {
                OutlineCurvature::Circular => (u * u, v * v, u, v),
                OutlineCurvature::Quartic => (u * u * u * u, v * v * v * v, u * u * u, v * v * v),
            };
            let f = (one - up) - vp;
            let (value, derivative) = (f / tau).smooth();
            gates[i + 4] = (
                value,
                derivative * n * du * (rx / tau) * sx.into(),
                derivative * n * dv * (ry / tau) * sy.into(),
            );
        }
        let value = gates.iter().fold(one, |product, g| product * g.0);
        let (mut dx, mut dy) = (zero, zero);
        for (i, gate) in gates.iter().enumerate() {
            let product = gates
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .fold(one, |product, (_, g)| product * g.0);
            dx = dx + gate.1 * product;
            dy = dy + gate.2 * product;
        }
        let amplitude = SampleNumber::exact(relief.signed_amplitude());
        let ratio = amplitude / w;
        dx = dx * ratio;
        dy = dy * ratio;
        let length = (dx * dx + dy * dy + one).sqrt();
        Ok(Some(ReliefSample {
            height: (amplitude * value).certify()?,
            normal: FaceNormal {
                x: ((zero - dx) / length).certify()?,
                y: ((zero - dy) / length).certify()?,
                z: (one / length).certify()?,
            },
        }))
    }

    fn bits(
        sample: Result<Option<ReliefSample>, SurfaceReliefError>,
    ) -> Result<Option<[u64; 4]>, SurfaceReliefError> {
        sample.map(|sample| {
            sample.map(|sample| {
                [
                    sample.height,
                    sample.normal.x,
                    sample.normal.y,
                    sample.normal.z,
                ]
                .map(f64::to_bits)
            })
        })
    }

    #[test]
    fn sampler_and_split_terms_reproduce_unsplit_relief_bits() {
        let radius = |x: f64, y: f64| CornerRadius { x, y };
        let mut compared = 0_usize;
        let mut reused = 0_usize;
        for (size, radii, curvature) in [
            (
                (166.0, 80.0),
                [radius(24.0, 24.0); 4],
                OutlineCurvature::Circular,
            ),
            (
                (120.0, 60.0),
                [
                    radius(20.0, 12.0),
                    radius(0.0, 0.0),
                    radius(12.0, 12.0),
                    radius(6.0, 6.0),
                ],
                OutlineCurvature::Circular,
            ),
            (
                (120.0, 60.0),
                [radius(0.0, 0.0); 4],
                OutlineCurvature::Circular,
            ),
            (
                (48.0, 48.0),
                [radius(24.0, 24.0); 4],
                OutlineCurvature::Circular,
            ),
            (
                (96.0, 40.0),
                [radius(20.0, 20.0); 4],
                OutlineCurvature::Circular,
            ),
            (
                (90.0, 44.0),
                [radius(16.0, 12.0); 4],
                OutlineCurvature::Quartic,
            ),
            (
                (31.0, 13.0),
                [radius(6.5, 6.5); 4],
                OutlineCurvature::Circular,
            ),
        ] {
            let outline = resolve_surface_outline(
                SurfaceSize {
                    width: size.0,
                    height: size.1,
                },
                LogicalCornerRadii {
                    top_start: radii[0],
                    top_end: radii[1],
                    bottom_end: radii[2],
                    bottom_start: radii[3],
                },
                LayoutDirection::Ltr,
                curvature,
                1.0,
            )
            .unwrap();
            for (kind, amplitude, shoulder_width) in [
                (ReliefKind::Convex, 1.0, 2.0),
                (ReliefKind::Recessed, 0.75, 1.5),
                (ReliefKind::Convex, 0.5, 1.0),
                (ReliefKind::Flat, 0.0, 0.0),
            ] {
                let Ok(relief) = resolve_surface_relief(
                    outline.clone(),
                    SurfaceReliefProfile {
                        kind,
                        body_thickness: 2.0,
                        amplitude,
                        shoulder_width,
                    },
                ) else {
                    continue;
                };
                let mut points = Vec::new();
                for (scale, samples, origin) in [(1.25, 1, -2.4)] {
                    let grid = f64::from(samples);
                    let columns = ((size.0 - 2.0 * origin) * scale).ceil() as u32;
                    let rows = ((size.1 - 2.0 * origin) * scale).ceil() as u32;
                    for y in 0..rows {
                        for x in 0..columns {
                            for sy in 0..samples {
                                for sx in 0..samples {
                                    let point = PhysicalVector {
                                        x: origin
                                            + (f64::from(x) + (f64::from(sx) + 0.5) / grid) / scale,
                                        y: origin
                                            + (f64::from(y) + (f64::from(sy) + 0.5) / grid) / scale,
                                    };
                                    let corner = (point.x.min(size.0 - point.x) < 28.0)
                                        && (point.y.min(size.1 - point.y) < 28.0);
                                    if corner
                                        || point
                                            .x
                                            .min(size.0 - point.x)
                                            .min(point.y)
                                            .min(size.1 - point.y)
                                            < 6.0
                                    {
                                        points.push(point);
                                    }
                                }
                            }
                        }
                    }
                }
                let mut state = 0x9e37_79b9_7f4a_7c15_u64;
                for _ in 0..500 {
                    let mut next = || {
                        state ^= state << 13;
                        state ^= state >> 7;
                        state ^= state << 17;
                        (state >> 11) as f64 / (1_u64 << 53) as f64
                    };
                    let x = next() * (size.0 + 2.0) - 1.0;
                    let y = next() * (size.1 + 2.0) - 1.0;
                    points.extend([
                        PhysicalVector { x, y },
                        PhysicalVector { x: size.0 - x, y },
                        PhysicalVector { x, y: size.1 / 2.0 },
                        PhysicalVector { x: size.0 / 2.0, y },
                    ]);
                }
                let mut sampler = relief.sampler_keeping(None);
                for point in points {
                    let expected = bits(unsplit_sample(&relief, point));
                    assert_eq!(bits(relief.sample(point)), expected, "{kind:?} {point:?}");
                    assert_eq!(bits(sampler.sample(point)), expected, "{kind:?} {point:?}");
                    compared += 1;
                }
                reused += sampler.lines.iter().map(HashMap::len).sum::<usize>();
            }
        }
        assert!(compared > 40_000 && reused > 3_000, "{compared} {reused}");
    }

    #[test]
    fn certified_flat_boxes_contain_only_exactly_flat_samples() {
        for kind in [ReliefKind::Convex, ReliefKind::Recessed] {
            for (curvature, radius, minimum_rejected) in [
                (
                    OutlineCurvature::Circular,
                    CornerRadius { x: 20.0, y: 12.0 },
                    500,
                ),
                (
                    OutlineCurvature::Quartic,
                    CornerRadius { x: 20.0, y: 12.0 },
                    500,
                ),
                (
                    OutlineCurvature::Circular,
                    CornerRadius { x: 0.0, y: 0.0 },
                    300,
                ),
            ] {
                let relief = relief(kind, curvature, radius);
                let (mut certified, mut rejected) = (0, 0);
                for iy in 0..60 {
                    for ix in 0..120 {
                        let min = PhysicalVector {
                            x: f64::from(ix) * 0.995,
                            y: f64::from(iy) * 0.995,
                        };
                        let max = PhysicalVector {
                            x: min.x + 0.75,
                            y: min.y + 0.75,
                        };
                        if !relief.flat_over(min, max) {
                            rejected += 1;
                            continue;
                        }
                        certified += 1;
                        for (fx, fy) in
                            [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (0.37, 0.61)]
                        {
                            let point = PhysicalVector {
                                x: min.x + (max.x - min.x) * fx,
                                y: min.y + (max.y - min.y) * fy,
                            };
                            let sample = relief.sample(point).unwrap().unwrap();
                            assert_eq!(
                                (sample.normal.x, sample.normal.y, sample.normal.z),
                                (0.0, 0.0, 1.0),
                                "{kind:?} {curvature:?} {point:?}"
                            );
                        }
                    }
                }
                assert!(
                    certified > 4000 && rejected > minimum_rejected,
                    "{certified} {rejected}"
                );
                let near_shoulder = PhysicalVector { x: 0.5, y: 30.0 };
                assert!(!relief.flat_over(near_shoulder, near_shoulder));
            }
        }
    }
}
