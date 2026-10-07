use crate::{CornerGeometryError, normalize_corner_radii};
use resina_environment::LayoutDirection;
use resina_model::{
    ContourSegment, CornerRadius, LogicalCornerRadii, PhysicalBounds, PhysicalVector, SurfaceSize,
};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExtrudedContourRequest {
    schema_version: String,
    size: SurfaceSize,
    radii: LogicalCornerRadii,
    layout_direction: LayoutDirection,
    offset: PhysicalVector,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtrudedContourResult {
    schema_version: &'static str,
    bounds: Option<PhysicalBounds>,
    segments: Vec<ContourSegment>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacedContour {
    offset: PhysicalVector,
    contour: ExtrudedContourResult,
}

impl PlacedContour {
    pub(crate) fn new(offset: PhysicalVector, contour: ExtrudedContourResult) -> Self {
        Self { offset, contour }
    }
    pub fn offset(&self) -> PhysicalVector {
        self.offset
    }
    pub fn contour(&self) -> &ExtrudedContourResult {
        &self.contour
    }
}

impl ExtrudedContourResult {
    pub fn bounds(&self) -> Option<PhysicalBounds> {
        self.bounds
    }
    pub fn segments(&self) -> &[ContourSegment] {
        &self.segments
    }
}

#[derive(Debug)]
pub enum ExtrudedContourError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    Geometry(CornerGeometryError),
    InvalidOffset,
    BoundsOverflow,
}

impl fmt::Display for ExtrudedContourError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "extruded contour parse failed: {error}"),
            Self::Request(error) => write!(formatter, "invalid extruded contour request: {error}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::Geometry(error) => write!(formatter, "invalid extrusion geometry: {error}"),
            Self::InvalidOffset => formatter.write_str("offset must be finite"),
            Self::BoundsOverflow => {
                formatter.write_str("extruded bounds exceed finite coordinates")
            }
        }
    }
}

impl std::error::Error for ExtrudedContourError {}

pub fn resolve_extruded_contour_source(
    source: &str,
) -> Result<ExtrudedContourResult, ExtrudedContourError> {
    let document = parse_token_document(source).map_err(ExtrudedContourError::Parse)?;
    let request: ExtrudedContourRequest =
        serde_json::from_value(document).map_err(ExtrudedContourError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(ExtrudedContourError::UnsupportedVersion);
    }
    resolve_extruded_contour(
        request.size,
        request.radii,
        request.layout_direction,
        request.offset,
    )
}

pub fn resolve_extruded_contour(
    size: SurfaceSize,
    radii: LogicalCornerRadii,
    layout_direction: LayoutDirection,
    offset: PhysicalVector,
) -> Result<ExtrudedContourResult, ExtrudedContourError> {
    let radii = normalize_corner_radii(size, radii).map_err(ExtrudedContourError::Geometry)?;
    if !offset.x.is_finite() || !offset.y.is_finite() {
        return Err(ExtrudedContourError::InvalidOffset);
    }
    if size.width == 0.0 || size.height == 0.0 {
        return Ok(ExtrudedContourResult {
            schema_version: "0.1.0",
            bounds: None,
            segments: Vec::new(),
        });
    }
    let bounds = PhysicalBounds {
        x: offset.x.min(0.0),
        y: offset.y.min(0.0),
        width: size.width + offset.x.abs(),
        height: size.height + offset.y.abs(),
    };
    if !bounds.width.is_finite() || !bounds.height.is_finite() {
        return Err(ExtrudedContourError::BoundsOverflow);
    }
    let physical = match layout_direction {
        LayoutDirection::Ltr => [
            radii.top_start,
            radii.top_end,
            radii.bottom_end,
            radii.bottom_start,
        ],
        LayoutDirection::Rtl => [
            radii.top_end,
            radii.top_start,
            radii.bottom_start,
            radii.bottom_end,
        ],
    }
    .map(|r| {
        if r.x == 0.0 || r.y == 0.0 {
            CornerRadius { x: 0.0, y: 0.0 }
        } else {
            r
        }
    });
    let unit = |x, y| PhysicalVector { x, y };
    let radials = [
        unit(-1.0, 0.0),
        unit(0.0, -1.0),
        unit(1.0, 0.0),
        unit(0.0, 1.0),
    ];
    let mut segments = Vec::new();
    let mut first = None;
    let mut last = None;
    for (index, radius) in physical.into_iter().enumerate() {
        let center = corner_center(size, radius, index);
        let start = radials[index];
        let end = radials[(index + 1) % 4];
        if radius.x > 0.0 && radius.y > 0.0 {
            let (a, b) = scaled_products(offset.x, radius.y, offset.y, radius.x);
            let mut endpoints = vec![start];
            if a != 0.0 && b != 0.0 {
                let length = (a * a + b * b).sqrt();
                for radial in [unit(b / length, -a / length), unit(-b / length, a / length)] {
                    if radial.x * (start.x + end.x) > 0.0 && radial.y * (start.y + end.y) > 0.0 {
                        endpoints.push(radial);
                    }
                }
            }
            endpoints.push(end);
            for pair in endpoints.windows(2) {
                let shifted = a * (pair[0].x + pair[1].x) + b * (pair[0].y + pair[1].y) > 0.0;
                let center = translated(center, if shifted { offset } else { unit(0.0, 0.0) });
                let segment = ContourSegment::Arc {
                    center,
                    radii: radius,
                    start: pair[0],
                    end: pair[1],
                };
                let (from, to) = endpoints_of(&segment);
                append(&mut segments, &mut first, &mut last, from, to, segment);
            }
        }
        let next_index = (index + 1) % 4;
        let next_radius = physical[next_index];
        let next_center = corner_center(size, next_radius, next_index);
        let normal = end;
        let shift = if offset.x * normal.x + offset.y * normal.y > 0.0 {
            offset
        } else {
            unit(0.0, 0.0)
        };
        let from = point(translated(center, shift), radius, end);
        let to = point(translated(next_center, shift), next_radius, end);
        if from != to {
            append(
                &mut segments,
                &mut first,
                &mut last,
                from,
                to,
                ContourSegment::Line { from, to },
            );
        }
    }
    if let (Some(from), Some(to)) = (last, first)
        && from != to
    {
        segments.push(ContourSegment::Line { from, to });
    }
    for segment in &segments {
        let (from, to) = endpoints_of(segment);
        if [from.x, from.y, to.x, to.y]
            .into_iter()
            .any(|value| !value.is_finite())
        {
            return Err(ExtrudedContourError::BoundsOverflow);
        }
    }
    Ok(ExtrudedContourResult {
        schema_version: "0.1.0",
        bounds: Some(bounds),
        segments,
    })
}

fn corner_center(size: SurfaceSize, radius: CornerRadius, index: usize) -> PhysicalVector {
    PhysicalVector {
        x: if index == 0 || index == 3 {
            radius.x
        } else {
            size.width - radius.x
        },
        y: if index < 2 {
            radius.y
        } else {
            size.height - radius.y
        },
    }
}

fn translated(point: PhysicalVector, offset: PhysicalVector) -> PhysicalVector {
    PhysicalVector {
        x: point.x + offset.x,
        y: point.y + offset.y,
    }
}

fn point(center: PhysicalVector, radii: CornerRadius, radial: PhysicalVector) -> PhysicalVector {
    PhysicalVector {
        x: center.x + radii.x * radial.x,
        y: center.y + radii.y * radial.y,
    }
}

fn endpoints_of(segment: &ContourSegment) -> (PhysicalVector, PhysicalVector) {
    match segment {
        ContourSegment::Line { from, to } => (*from, *to),
        ContourSegment::Arc {
            center,
            radii,
            start,
            end,
        } => (point(*center, *radii, *start), point(*center, *radii, *end)),
    }
}

fn append(
    segments: &mut Vec<ContourSegment>,
    first: &mut Option<PhysicalVector>,
    last: &mut Option<PhysicalVector>,
    from: PhysicalVector,
    to: PhysicalVector,
    segment: ContourSegment,
) {
    if let Some(previous) = *last {
        if previous != from {
            segments.push(ContourSegment::Line {
                from: previous,
                to: from,
            });
        }
    } else {
        *first = Some(from);
    }
    segments.push(segment);
    *last = Some(to);
}

fn scaled_products(x: f64, y: f64, z: f64, w: f64) -> (f64, f64) {
    // Scale the products together: scaling each factor separately can erase both
    // products when a very large component is paired with a subnormal radius.
    fn parts(value: f64) -> (f64, i32) {
        let bits = value.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i32;
        if exponent == 0 {
            let (mantissa, exponent) = parts(value * 4503599627370496.0);
            return (mantissa, exponent - 52);
        }
        (
            f64::from_bits((bits & ((1u64 << 52) - 1)) | (1023u64 << 52)),
            exponent - 1023,
        )
    }
    fn product(x: f64, y: f64) -> Option<(f64, i32)> {
        if x == 0.0 || y == 0.0 {
            return None;
        }
        let (a, e) = parts(x.abs());
        let (b, f) = parts(y.abs());
        Some((x.signum() * y.signum() * a * b, e + f))
    }
    let a = product(x, y);
    let b = product(z, w);
    let exponent = a
        .map(|(_, e)| e)
        .into_iter()
        .chain(b.map(|(_, e)| e))
        .max()
        .unwrap_or(0);
    let scale = |product: Option<(f64, i32)>| match product {
        None => 0.0,
        Some((mantissa, e)) => {
            let difference = e - exponent;
            let power = if difference >= -1022 {
                f64::from_bits(((difference + 1023) as u64) << 52)
            } else if difference >= -1074 {
                f64::from_bits(1u64 << (difference + 1074))
            } else {
                0.0
            };
            mantissa * power
        }
    };
    (scale(a), scale(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compare_json(actual: &serde_json::Value, expected: &serde_json::Value, radial: bool) {
        match expected {
            serde_json::Value::Object(fields) => {
                let actual = actual.as_object().unwrap();
                assert_eq!(actual.len(), fields.len());
                for (name, value) in fields {
                    compare_json(
                        &actual[name],
                        value,
                        radial || name == "start" || name == "end",
                    );
                }
            }
            serde_json::Value::Array(values) => {
                let actual = actual.as_array().unwrap();
                assert_eq!(actual.len(), values.len());
                for (actual, expected) in actual.iter().zip(values) {
                    compare_json(actual, expected, radial);
                }
            }
            serde_json::Value::Number(value) => {
                let expected = value.as_f64().unwrap();
                let actual = actual.as_f64().unwrap();
                let tolerance = if radial {
                    1e-12
                } else {
                    (expected.abs() * 1e-12).max(1e-12)
                };
                assert!(
                    (actual - expected).abs() <= tolerance,
                    "{actual} != {expected}"
                );
            }
            _ => assert_eq!(actual, expected),
        }
    }

    #[test]
    fn public_extruded_contour_vectors() {
        let vectors: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../../../../conformance/geometry/extruded-contour-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = resolve_extruded_contour_source(&vector["request"].to_string());
            if let Some(expected) = vector.get("expected") {
                compare_json(
                    &serde_json::to_value(result.unwrap()).unwrap(),
                    expected,
                    false,
                );
            } else {
                let error = result.unwrap_err().to_string();
                assert!(
                    error.contains(vector["errorContains"].as_str().unwrap()),
                    "{}: {error}",
                    vector["name"]
                );
            }
        }
    }

    fn uniform(x: f64, y: f64) -> LogicalCornerRadii {
        let radius = CornerRadius { x, y };
        LogicalCornerRadii {
            top_start: radius,
            top_end: radius,
            bottom_end: radius,
            bottom_start: radius,
        }
    }

    fn dot(a: PhysicalVector, b: PhysicalVector) -> f64 {
        a.x * b.x + a.y * b.y
    }
    fn cross(a: PhysicalVector, b: PhysicalVector) -> f64 {
        a.x * b.y - a.y * b.x
    }

    fn arc_support(
        center: PhysicalVector,
        radius: CornerRadius,
        start: PhysicalVector,
        end: PhysicalVector,
        normal: PhysicalVector,
    ) -> f64 {
        let mut support =
            dot(point(center, radius, start), normal).max(dot(point(center, radius, end), normal));
        let x = radius.x * normal.x;
        let y = radius.y * normal.y;
        let length = (x * x + y * y).sqrt();
        if length > 0.0 {
            let radial = PhysicalVector {
                x: x / length,
                y: y / length,
            };
            if cross(start, radial) >= 0.0 && cross(radial, end) >= 0.0 {
                support = support.max(dot(center, normal) + length);
            }
        }
        support
    }

    #[test]
    fn swept_boundary_matches_independent_support_function() {
        let size = SurfaceSize {
            width: 100.0,
            height: 60.0,
        };
        let asymmetric = LogicalCornerRadii {
            top_start: CornerRadius { x: 8.0, y: 19.0 },
            top_end: CornerRadius { x: 22.0, y: 7.0 },
            bottom_end: CornerRadius { x: 12.0, y: 18.0 },
            bottom_start: CornerRadius { x: 0.0, y: 11.0 },
        };
        let axes = [
            PhysicalVector { x: -1.0, y: 0.0 },
            PhysicalVector { x: 0.0, y: -1.0 },
            PhysicalVector { x: 1.0, y: 0.0 },
            PhysicalVector { x: 0.0, y: 1.0 },
        ];
        for radii in [
            uniform(0.0, 0.0),
            uniform(30.0, 30.0),
            uniform(400.0, 200.0),
            asymmetric,
        ] {
            let normalized = normalize_corner_radii(size, radii).unwrap();
            for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
                let physical = match direction {
                    LayoutDirection::Ltr => [
                        normalized.top_start,
                        normalized.top_end,
                        normalized.bottom_end,
                        normalized.bottom_start,
                    ],
                    LayoutDirection::Rtl => [
                        normalized.top_end,
                        normalized.top_start,
                        normalized.bottom_start,
                        normalized.bottom_end,
                    ],
                };
                for offset in [
                    PhysicalVector { x: 0.0, y: 0.0 },
                    PhysicalVector { x: 12.0, y: 0.0 },
                    PhysicalVector { x: 0.0, y: -14.0 },
                    PhysicalVector { x: 400.0, y: 500.0 },
                    PhysicalVector { x: -23.0, y: 9.0 },
                    PhysicalVector { x: -7.0, y: -11.0 },
                ] {
                    let result = resolve_extruded_contour(size, radii, direction, offset).unwrap();
                    let endpoints: Vec<_> = result.segments().iter().map(endpoints_of).collect();
                    for i in 0..endpoints.len() {
                        assert_eq!(endpoints[i].1, endpoints[(i + 1) % endpoints.len()].0);
                    }
                    for angle in 0..360 {
                        let angle = f64::from(angle).to_radians();
                        let normal = PhysicalVector {
                            x: angle.cos(),
                            y: angle.sin(),
                        };
                        let mut front_support = f64::NEG_INFINITY;
                        for (index, radius) in physical.iter().enumerate() {
                            let radius = if radius.x == 0.0 || radius.y == 0.0 {
                                CornerRadius { x: 0.0, y: 0.0 }
                            } else {
                                *radius
                            };
                            let center = PhysicalVector {
                                x: if index == 0 || index == 3 {
                                    radius.x
                                } else {
                                    size.width - radius.x
                                },
                                y: if index < 2 {
                                    radius.y
                                } else {
                                    size.height - radius.y
                                },
                            };
                            front_support = front_support.max(arc_support(
                                center,
                                radius,
                                axes[index],
                                axes[(index + 1) % 4],
                                normal,
                            ));
                        }
                        let expected = front_support + dot(offset, normal).max(0.0);
                        let actual = result
                            .segments()
                            .iter()
                            .map(|segment| match segment {
                                ContourSegment::Line { from, to } => {
                                    dot(*from, normal).max(dot(*to, normal))
                                }
                                ContourSegment::Arc {
                                    center,
                                    radii,
                                    start,
                                    end,
                                } => arc_support(*center, *radii, *start, *end, normal),
                            })
                            .fold(f64::NEG_INFINITY, f64::max);
                        assert!(
                            (actual - expected).abs() <= 1e-10,
                            "{direction:?} {radii:?} {offset:?}: {actual} != {expected}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn product_scaling_preserves_crossed_extreme_components() {
        let tiny = f64::from_bits(1);
        for (x, y) in [
            (8e307, tiny),
            (tiny, 8e307),
            (f64::MAX, f64::MAX),
            (tiny, tiny),
        ] {
            let (a, b) = scaled_products(x, y, -y, x);
            assert!(a.is_finite() && a > 0.0);
            assert_eq!(a, -b);
        }
    }

    #[test]
    fn extreme_anisotropic_ellipse_keeps_its_support_switches() {
        let tiny = f64::from_bits(1);
        let contour = resolve_extruded_contour(
            SurfaceSize {
                width: 8e307,
                height: 1.0,
            },
            uniform(4e307, tiny),
            LayoutDirection::Ltr,
            PhysicalVector { x: 4e307, y: tiny },
        )
        .unwrap();
        let mut split_endpoints = 0;
        for segment in contour.segments() {
            if let ContourSegment::Arc { start, end, .. } = segment {
                for radial in [start, end] {
                    if radial.x != 0.0 && radial.y != 0.0 {
                        assert!((radial.x.abs() - std::f64::consts::FRAC_1_SQRT_2).abs() <= 1e-12);
                        assert!((radial.y.abs() - std::f64::consts::FRAC_1_SQRT_2).abs() <= 1e-12);
                        split_endpoints += 1;
                    }
                }
            }
        }
        assert_eq!(split_endpoints, 4);
        assert!(
            !serde_json::to_value(contour)
                .unwrap()
                .to_string()
                .contains("null")
        );
    }

    #[test]
    fn authored_shapes_compose_with_light_and_nested_insets() {
        use crate::{resolve_inset_contour, resolve_key_light, resolve_shape_fallback};
        use resina_model::{KeyLight, ShapeFallbackAssignments, ShapeIntent};
        let assignments: ShapeFallbackAssignments =
            serde_json::from_str(include_str!("../../../../definitions/tier0-shapes.json"))
                .unwrap();
        let tokens =
            resina_tokens::resolve_token_source(include_str!("../../../../tokens/foundation.json"))
                .unwrap();
        let size = SurfaceSize {
            width: 200.0,
            height: 80.0,
        };
        for shape in [
            ShapeIntent::Structural,
            ShapeIntent::Soft,
            ShapeIntent::Rounded,
            ShapeIntent::Capsule,
            ShapeIntent::Organic,
        ] {
            let radii = resolve_shape_fallback(shape, size, &assignments, &tokens).unwrap();
            let inset = resolve_inset_contour(size, radii, 2.0).unwrap();
            for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
                for light in [
                    PhysicalVector { x: -1.0, y: -1.0 },
                    PhysicalVector { x: 1.0, y: -2.0 },
                ] {
                    let light = KeyLight::try_new(light).unwrap();
                    let offset = resolve_key_light(&light, 4.0, &[]).unwrap().side_offset();
                    let outer = resolve_extruded_contour(size, radii, direction, offset).unwrap();
                    let inner =
                        resolve_extruded_contour(inset.size(), inset.radii(), direction, offset)
                            .unwrap();
                    for angle in 0..360 {
                        let angle = f64::from(angle).to_radians();
                        let normal = PhysicalVector {
                            x: angle.cos(),
                            y: angle.sin(),
                        };
                        let support = |contour: &ExtrudedContourResult| {
                            contour
                                .segments()
                                .iter()
                                .map(|segment| match segment {
                                    ContourSegment::Line { from, to } => {
                                        dot(*from, normal).max(dot(*to, normal))
                                    }
                                    ContourSegment::Arc {
                                        center,
                                        radii,
                                        start,
                                        end,
                                    } => arc_support(*center, *radii, *start, *end, normal),
                                })
                                .fold(f64::NEG_INFINITY, f64::max)
                        };
                        let inner_support = support(&inner) + 2.0 * (normal.x + normal.y);
                        assert!(
                            inner_support <= support(&outer) + 1e-10,
                            "{shape:?} {direction:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn typed_calls_reject_invalid_geometry_before_empty_result() {
        let size = SurfaceSize {
            width: 0.0,
            height: 10.0,
        };
        for offset in [
            PhysicalVector {
                x: f64::NAN,
                y: 0.0,
            },
            PhysicalVector {
                x: 0.0,
                y: f64::INFINITY,
            },
        ] {
            assert!(matches!(
                resolve_extruded_contour(size, uniform(0.0, 0.0), LayoutDirection::Ltr, offset),
                Err(ExtrudedContourError::InvalidOffset)
            ));
        }
        assert!(matches!(
            resolve_extruded_contour(
                size,
                uniform(-1.0, 0.0),
                LayoutDirection::Ltr,
                PhysicalVector { x: 0.0, y: 0.0 }
            ),
            Err(ExtrudedContourError::Geometry(_))
        ));
        assert!(matches!(
            resolve_extruded_contour(
                SurfaceSize {
                    width: f64::MAX,
                    height: 1.0
                },
                uniform(0.0, 0.0),
                LayoutDirection::Ltr,
                PhysicalVector {
                    x: f64::MAX,
                    y: 0.0
                }
            ),
            Err(ExtrudedContourError::BoundsOverflow)
        ));
    }
}
