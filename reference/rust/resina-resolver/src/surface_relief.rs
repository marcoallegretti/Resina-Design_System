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
struct ReliefRequest {
    schema_version: String,
    outline: serde_json::Value,
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
    let request: ReliefRequest =
        serde_json::from_value(document).map_err(|e| SurfaceReliefError(e.to_string()))?;
    if request.schema_version != "0.1.0" {
        return Err(SurfaceReliefError("schemaVersion must be 0.1.0".into()));
    }
    let outline = resolve_surface_outline_source(&request.outline.to_string())
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
        || profile.amplitude > profile.shoulder_width / 2.0
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
        let w = self.profile.shoulder_width;
        let straight = |distance: f64, x: f64, y: f64| {
            let (value, derivative) = smooth(if distance >= w { 1.0 } else { distance / w });
            (value, derivative * x, derivative * y)
        };
        let mut gates = [
            straight(point.x, 1.0, 0.0),
            straight(size.width - point.x, -1.0, 0.0),
            straight(point.y, 0.0, 1.0),
            straight(size.height - point.y, 0.0, -1.0),
            (1.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
        ];
        let corners = [
            (point.x, point.y, 1.0, 1.0),
            (size.width - point.x, point.y, -1.0, 1.0),
            (size.width - point.x, size.height - point.y, -1.0, -1.0),
            (point.x, size.height - point.y, 1.0, -1.0),
        ];
        for (i, (r, (x, y, sx, sy))) in self.outline.corners().into_iter().zip(corners).enumerate()
        {
            if r.x == 0.0 {
                continue;
            }
            let (rx, ry, tau) = self.corner_ratios(r.x, r.y)?;
            let u = if x >= r.x { 0.0 } else { (r.x - x) / r.x };
            let v = if y >= r.y { 0.0 } else { (r.y - y) / r.y };
            let (up, vp, du, dv) = match self.outline.curvature() {
                OutlineCurvature::Circular => (u * u, v * v, u, v),
                OutlineCurvature::Quartic => (u * u * u * u, v * v * v * v, u * u * u, v * v * v),
            };
            let f = (1.0 - up) - vp;
            let (value, derivative) = smooth(if f >= tau { 1.0 } else { f / tau });
            gates[i + 4] = (
                value,
                derivative * self.exponent() * du * (rx / tau) * sx,
                derivative * self.exponent() * dv * (ry / tau) * sy,
            );
        }
        let value: f64 = gates.iter().map(|g| g.0).product();
        let (mut dx, mut dy) = (0.0, 0.0);
        for (i, gate) in gates.iter().enumerate() {
            let product: f64 = gates
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, g)| g.0)
                .product();
            dx += gate.1 * product;
            dy += gate.2 * product;
        }
        let ratio = self.signed_amplitude() / w;
        dx *= ratio;
        dy *= ratio;
        let length = (dx * dx + dy * dy + 1.0).sqrt();
        Ok(Some(ReliefSample {
            height: self.signed_amplitude() * value,
            normal: FaceNormal {
                x: -dx / length,
                y: -dy / length,
                z: 1.0 / length,
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
