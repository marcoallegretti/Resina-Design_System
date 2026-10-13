use crate::{OutlineCurvature, SurfaceOutline, resolve_surface_outline_source};
use resina_model::{PhysicalBounds, PhysicalVector};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use std::fmt;

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

pub fn resolve_surface_relief(
    outline: SurfaceOutline,
    profile: SurfaceReliefProfile,
) -> Result<SurfaceRelief, SurfaceReliefError> {
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
    let mut result = SurfaceRelief {
        content_bounds: outline.content_bounds(),
        outline,
        profile,
    };
    if profile.kind == ReliefKind::Flat {
        if profile.amplitude != 0.0 || profile.shoulder_width != 0.0 {
            return Err(fail("flat requires zero amplitude and shoulderWidth"));
        }
        return Ok(result);
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
        let size = self.outline.size();
        let w = SampleNumber::exact(self.profile.shoulder_width);
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
        for (i, (r, (x, y, sx, sy))) in self.outline.corners().into_iter().zip(corners).enumerate()
        {
            if r.x == 0.0 {
                continue;
            }
            let a = SampleNumber::exact(r.x);
            let b = SampleNumber::exact(r.y);
            let rx = w / a;
            let ry = w / b;
            let n = SampleNumber::exact(self.exponent());
            let tau = n * (rx + ry);
            let u = ((a - x) / a).clamp(0.0, 1.0);
            let v = ((b - y) / b).clamp(0.0, 1.0);
            let (up, vp, du, dv) = match self.outline.curvature() {
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
        let amplitude = SampleNumber::exact(self.signed_amplitude());
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
