use crate::{
    ExtrudedContourResult, FocusIndicatorIr, OpaqueSurfaceIr, PlacedContour, SrgbFallback,
};
use resina_model::{ContourSegment, PhysicalVector};
use std::{fmt, sync::OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfacePaintError {
    InvalidPoint,
    NumericRange,
    UnsupportedContour,
}

impl fmt::Display for SurfacePaintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPoint => "paint point must be finite",
            Self::NumericRange => "paint geometry exceeds finite sampling arithmetic",
            Self::UnsupportedContour => "opaque paint sampling requires nonempty circular contours",
        })
    }
}

impl std::error::Error for SurfacePaintError {}

impl OpaqueSurfaceIr {
    pub fn sample_paint(
        &self,
        point: PhysicalVector,
    ) -> Result<Option<SrgbFallback>, SurfacePaintError> {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(SurfacePaintError::InvalidPoint);
        }
        let direction = self.lighting().direction();
        if !contains(self.geometry().silhouette(), point)? {
            return Ok(None);
        }
        if !placed_contains(self.geometry().edge_interior(), point)? {
            return Ok(Some(self.edge().color().clone()));
        }
        if !contains(self.geometry().front(), point)? {
            return Ok(Some(self.pigment().side().clone()));
        }
        if self.highlight_width() > 0.0
            && placed_contains(self.geometry().highlight_outer(), point)?
            && !placed_contains(self.geometry().content(), point)?
        {
            let highlight = placed_boundary(self.geometry().highlight_outer(), point, direction)?;
            let lift = self.pigment().profile().highlight_lift() * highlight.weight;
            static WHITE: OnceLock<SrgbFallback> = OnceLock::new();
            let white = WHITE.get_or_init(|| {
                resina_color::resolve_srgb_fallback(&serde_json::json!({
                    "colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1
                }))
                .expect("constant opaque sRGB white")
            });
            let color = resina_color::composite_srgb_over_opaque(
                &white.with_alpha(lift).expect("bounded highlight weight"),
                self.pigment().body(),
            )
            .expect("opaque surface body");
            return Ok(Some(color));
        }
        Ok(Some(self.pigment().body().clone()))
    }
}

impl FocusIndicatorIr {
    pub fn sample_paint(
        &self,
        point: PhysicalVector,
    ) -> Result<Option<SrgbFallback>, SurfacePaintError> {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(SurfacePaintError::InvalidPoint);
        }
        if placed_contains(self.geometry().outer(), point)?
            && !placed_contains(self.geometry().inner(), point)?
        {
            Ok(Some(self.indicator().color().clone()))
        } else {
            Ok(None)
        }
    }
}

#[derive(Clone, Copy)]
struct Boundary {
    inside: bool,
    weight: f64,
}

#[derive(Clone, Copy)]
struct Nearest {
    distance: f64,
    signed_distance: f64,
    weight: f64,
}

fn placed_contains(
    contour: &PlacedContour,
    point: PhysicalVector,
) -> Result<bool, SurfacePaintError> {
    contains(contour.contour(), subtract(point, contour.offset()))
}

fn contains(
    contour: &ExtrudedContourResult,
    point: PhysicalVector,
) -> Result<bool, SurfacePaintError> {
    if let Some(inside) = convex_contains(contour, point) {
        return Ok(inside);
    }
    Ok(boundary(contour, point, PhysicalVector { x: 0.0, y: 0.0 })?.inside)
}

fn convex_contains(contour: &ExtrudedContourResult, point: PhysicalVector) -> Option<bool> {
    let bounds = contour.bounds()?;
    if point.x < bounds.x
        || point.y < bounds.y
        || point.x - bounds.x > bounds.width
        || point.y - bounds.y > bounds.height
    {
        return Some(false);
    }
    let magnitude = bounds
        .x
        .abs()
        .max(bounds.y.abs())
        .max(bounds.width)
        .max(bounds.height);
    // Keep cross products and their error bounds away from overflow and underflow.
    if !(1e-100..=1e100).contains(&magnitude) || contour.segments().is_empty() {
        return None;
    }
    let tolerance = 64.0 * f64::EPSILON * magnitude;
    let mut outside = false;
    // Canonical convex contours are intersections of their supporting half-planes.
    // Circular arcs add radial constraints in their outward sectors. Ambiguous
    // arithmetic falls back to the nearest-boundary evaluator's closed-set rule.
    for segment in contour.segments() {
        match *segment {
            ContourSegment::Line { from, to } => {
                let tangent = subtract(to, from);
                let signed = cross(tangent, subtract(point, from));
                let uncertainty = tolerance * (tangent.x.abs() + tangent.y.abs());
                if signed.abs() <= uncertainty {
                    return None;
                }
                outside |= signed < 0.0;
            }
            ContourSegment::Arc {
                center,
                radii,
                start,
                end,
            } => {
                if radii.x != radii.y || radii.x <= 0.0 {
                    return None;
                }
                let offset = subtract(point, center);
                for radial in [start, end] {
                    let signed = dot(offset, radial) - radii.x;
                    if signed.abs() <= tolerance {
                        return None;
                    }
                    outside |= signed > 0.0;
                }
                let start_side = cross(start, offset);
                let end_side = cross(offset, end);
                if start_side > tolerance && end_side > tolerance {
                    let signed = offset.x.hypot(offset.y) - radii.x;
                    if signed.abs() <= tolerance {
                        return None;
                    }
                    outside |= signed > 0.0;
                } else if start_side >= -tolerance && end_side >= -tolerance {
                    return None;
                }
            }
        }
    }
    Some(!outside)
}

fn placed_boundary(
    contour: &PlacedContour,
    point: PhysicalVector,
    direction: PhysicalVector,
) -> Result<Boundary, SurfacePaintError> {
    boundary(
        contour.contour(),
        subtract(point, contour.offset()),
        direction,
    )
}

fn boundary(
    contour: &ExtrudedContourResult,
    point: PhysicalVector,
    direction: PhysicalVector,
) -> Result<Boundary, SurfacePaintError> {
    let bounds = contour
        .bounds()
        .ok_or(SurfacePaintError::UnsupportedContour)?;
    if point.x < bounds.x
        || point.y < bounds.y
        || point.x - bounds.x > bounds.width
        || point.y - bounds.y > bounds.height
    {
        return Ok(Boundary {
            inside: false,
            weight: 0.0,
        });
    }
    let mut nearest = None;
    for segment in contour.segments() {
        match *segment {
            ContourSegment::Line { from, to } => {
                let tangent = subtract(to, from);
                let length = finite_length(tangent)?;
                if length == 0.0 {
                    return Err(SurfacePaintError::UnsupportedContour);
                }
                let tangent = PhysicalVector {
                    x: tangent.x / length,
                    y: tangent.y / length,
                };
                let offset = subtract(point, from);
                let along = dot(offset, tangent).clamp(0.0, length);
                let closest = add(from, scale(tangent, along));
                let normal = PhysicalVector {
                    x: tangent.y,
                    y: -tangent.x,
                };
                merge(&mut nearest, candidate(point, closest, normal, direction)?);
            }
            ContourSegment::Arc {
                center,
                radii,
                start,
                end,
            } => {
                if radii.x != radii.y || radii.x <= 0.0 {
                    return Err(SurfacePaintError::UnsupportedContour);
                }
                let offset = subtract(point, center);
                let length = finite_length(offset)?;
                if length == 0.0 {
                    let weight = if on_arc(direction, start, end) {
                        1.0
                    } else {
                        dot(start, direction)
                            .max(dot(end, direction))
                            .clamp(0.0, 1.0)
                    };
                    merge(
                        &mut nearest,
                        Nearest {
                            distance: radii.x,
                            signed_distance: -radii.x,
                            weight,
                        },
                    );
                } else {
                    let radial = PhysicalVector {
                        x: offset.x / length,
                        y: offset.y / length,
                    };
                    if on_arc(radial, start, end) {
                        merge(
                            &mut nearest,
                            candidate(
                                point,
                                add(center, scale(radial, radii.x)),
                                radial,
                                direction,
                            )?,
                        );
                    } else {
                        for radial in [start, end] {
                            merge(
                                &mut nearest,
                                candidate(
                                    point,
                                    add(center, scale(radial, radii.x)),
                                    radial,
                                    direction,
                                )?,
                            );
                        }
                    }
                }
            }
        }
    }
    let nearest = nearest.ok_or(SurfacePaintError::UnsupportedContour)?;
    Ok(Boundary {
        inside: nearest.signed_distance <= 0.0,
        weight: nearest.weight,
    })
}

fn candidate(
    point: PhysicalVector,
    closest: PhysicalVector,
    normal: PhysicalVector,
    direction: PhysicalVector,
) -> Result<Nearest, SurfacePaintError> {
    let delta = subtract(point, closest);
    Ok(Nearest {
        distance: finite_length(delta)?,
        signed_distance: dot(delta, normal),
        weight: dot(normal, direction).clamp(0.0, 1.0),
    })
}

fn merge(nearest: &mut Option<Nearest>, candidate: Nearest) {
    let Some(current) = nearest else {
        *nearest = Some(candidate);
        return;
    };
    // Arithmetic ties at shared endpoints must retain both incident normals.
    let tolerance = 32.0 * f64::EPSILON * current.distance.max(candidate.distance);
    if (candidate.distance - current.distance).abs() <= tolerance {
        current.distance = current.distance.min(candidate.distance);
        current.signed_distance = current.signed_distance.max(candidate.signed_distance);
        current.weight = current.weight.max(candidate.weight);
    } else if candidate.distance < current.distance {
        *current = candidate;
    }
}

fn finite_length(vector: PhysicalVector) -> Result<f64, SurfacePaintError> {
    let length = vector.x.hypot(vector.y);
    if length.is_finite() {
        Ok(length)
    } else {
        Err(SurfacePaintError::NumericRange)
    }
}
fn on_arc(normal: PhysicalVector, start: PhysicalVector, end: PhysicalVector) -> bool {
    cross(start, normal) >= 0.0 && cross(normal, end) >= 0.0
}
fn cross(a: PhysicalVector, b: PhysicalVector) -> f64 {
    a.x * b.y - a.y * b.x
}
fn dot(a: PhysicalVector, b: PhysicalVector) -> f64 {
    a.x * b.x + a.y * b.y
}
fn subtract(a: PhysicalVector, b: PhysicalVector) -> PhysicalVector {
    PhysicalVector {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}
fn add(a: PhysicalVector, b: PhysicalVector) -> PhysicalVector {
    PhysicalVector {
        x: a.x + b.x,
        y: a.y + b.y,
    }
}
fn scale(a: PhysicalVector, factor: f64) -> PhysicalVector {
    PhysicalVector {
        x: a.x * factor,
        y: a.y * factor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use resina_environment::LayoutDirection;
    use resina_model::{CornerRadius, LogicalCornerRadii, SurfaceSize};

    fn circle(offset: PhysicalVector) -> ExtrudedContourResult {
        let radius = CornerRadius { x: 5.0, y: 5.0 };
        crate::resolve_extruded_contour(
            SurfaceSize {
                width: 10.0,
                height: 10.0,
            },
            LogicalCornerRadii {
                top_start: radius,
                top_end: radius,
                bottom_end: radius,
                bottom_start: radius,
            },
            LayoutDirection::Ltr,
            offset,
        )
        .unwrap()
    }

    #[test]
    fn swept_circle_containment_matches_distance_to_center_segment() {
        let direction = PhysicalVector { x: 0.0, y: -1.0 };
        for offset in [
            PhysicalVector { x: 0.0, y: 0.0 },
            PhysicalVector { x: 4.0, y: 3.0 },
            PhysicalVector { x: -4.0, y: 3.0 },
            PhysicalVector { x: 0.0, y: -4.0 },
        ] {
            let contour = circle(offset);
            for y in -40..160 {
                for x in -40..160 {
                    let point = PhysicalVector {
                        x: x as f64 * 0.1 + 0.037,
                        y: y as f64 * 0.1 + 0.029,
                    };
                    let delta = subtract(point, PhysicalVector { x: 5.0, y: 5.0 });
                    let squared = dot(offset, offset);
                    let t = if squared == 0.0 {
                        0.0
                    } else {
                        (dot(delta, offset) / squared).clamp(0.0, 1.0)
                    };
                    let distance = subtract(delta, scale(offset, t))
                        .x
                        .hypot(subtract(delta, scale(offset, t)).y);
                    assert_eq!(
                        boundary(&contour, point, direction).unwrap().inside,
                        distance <= 5.0,
                        "{point:?} offset {offset:?}"
                    );
                    assert_eq!(contains(&contour, point).unwrap(), distance <= 5.0);
                }
            }
        }
    }

    fn compare_containment(contour: &ExtrudedContourResult, point: PhysicalVector) {
        let nearest =
            boundary(contour, point, PhysicalVector { x: 0.0, y: 0.0 }).map(|result| result.inside);
        assert_eq!(contains(contour, point), nearest, "{point:?} {contour:?}");
    }

    #[test]
    fn scaled_convex_containment_matches_analytic_swept_circle() {
        for magnitude in [1e-90, 1e90] {
            let radius = CornerRadius {
                x: 5.0 * magnitude,
                y: 5.0 * magnitude,
            };
            let contour = crate::resolve_extruded_contour(
                SurfaceSize {
                    width: 10.0 * magnitude,
                    height: 10.0 * magnitude,
                },
                LogicalCornerRadii {
                    top_start: radius,
                    top_end: radius,
                    bottom_end: radius,
                    bottom_start: radius,
                },
                LayoutDirection::Rtl,
                PhysicalVector {
                    x: 4.0 * magnitude,
                    y: 3.0 * magnitude,
                },
            )
            .unwrap();
            let mut classified = 0;
            for y in 0..100 {
                for x in 0..100 {
                    let unscaled = PhysicalVector {
                        x: x as f64 * 0.2 - 2.037,
                        y: y as f64 * 0.2 - 2.029,
                    };
                    let delta = subtract(unscaled, PhysicalVector { x: 5.0, y: 5.0 });
                    let t = ((delta.x * 4.0 + delta.y * 3.0) / 25.0).clamp(0.0, 1.0);
                    let nearest = subtract(
                        delta,
                        PhysicalVector {
                            x: 4.0 * t,
                            y: 3.0 * t,
                        },
                    );
                    let expected = nearest.x.hypot(nearest.y) <= 5.0;
                    let point = scale(unscaled, magnitude);
                    assert_eq!(
                        contains(&contour, point).unwrap(),
                        expected,
                        "{magnitude} {point:?}"
                    );
                    classified += usize::from(convex_contains(&contour, point) == Some(true));
                }
            }
            assert!(classified > 100);
        }
    }

    #[test]
    fn convex_containment_preserves_asymmetric_sweeps_and_boundary_rounding() {
        for radii in [
            [0.0; 4],
            [4.0; 4],
            [0.0, 2.0, 5.0, 1.0],
            [8.0, 1.0, 3.0, 6.0],
        ] {
            for layout in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
                for offset in [
                    PhysicalVector { x: 0.0, y: 0.0 },
                    PhysicalVector { x: 4.0, y: 3.0 },
                    PhysicalVector { x: -4.0, y: 3.0 },
                    PhysicalVector { x: 4.0, y: -3.0 },
                    PhysicalVector { x: -4.0, y: -3.0 },
                    PhysicalVector { x: 0.0, y: -4.0 },
                    PhysicalVector { x: -4.0, y: 0.0 },
                ] {
                    let [top_start, top_end, bottom_end, bottom_start] =
                        radii.map(|radius| CornerRadius {
                            x: radius,
                            y: radius,
                        });
                    let contour = crate::resolve_extruded_contour(
                        SurfaceSize {
                            width: 10.0,
                            height: 12.0,
                        },
                        LogicalCornerRadii {
                            top_start,
                            top_end,
                            bottom_end,
                            bottom_start,
                        },
                        layout,
                        offset,
                    )
                    .unwrap();
                    let bounds = contour.bounds().unwrap();
                    for y in 0..23 {
                        for x in 0..17 {
                            compare_containment(
                                &contour,
                                PhysicalVector {
                                    x: bounds.x - 1.0
                                        + (bounds.width + 2.0) * (x as f64 + 0.37) / 17.0,
                                    y: bounds.y - 1.0
                                        + (bounds.height + 2.0) * (y as f64 + 0.29) / 23.0,
                                },
                            );
                        }
                    }
                    for segment in contour.segments() {
                        for fraction in [0.0, 0.5, 1.0] {
                            let point = match *segment {
                                ContourSegment::Line { from, to } => {
                                    add(from, scale(subtract(to, from), fraction))
                                }
                                ContourSegment::Arc {
                                    center,
                                    radii,
                                    start,
                                    end,
                                } => {
                                    let radial =
                                        add(scale(start, 1.0 - fraction), scale(end, fraction));
                                    let length = radial.x.hypot(radial.y);
                                    add(center, scale(radial, radii.x / length))
                                }
                            };
                            for x in [point.x.next_down(), point.x, point.x.next_up()] {
                                for y in [point.y.next_down(), point.y, point.y.next_up()] {
                                    compare_containment(&contour, PhysicalVector { x, y });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fast_containment_defers_extremes_and_exact_boundaries() {
        let contour = circle(PhysicalVector { x: 0.0, y: 0.0 });
        assert_eq!(
            convex_contains(&contour, PhysicalVector { x: 5.125, y: 5.25 }),
            Some(true)
        );
        assert_eq!(
            convex_contains(&contour, PhysicalVector { x: 0.125, y: 0.25 }),
            Some(false)
        );
        assert_eq!(
            convex_contains(&contour, PhysicalVector { x: 5.0, y: 0.0 }),
            None
        );
        for magnitude in [1e-120, 1e120] {
            let radius = CornerRadius {
                x: 4.0 * magnitude,
                y: 4.0 * magnitude,
            };
            let contour = crate::resolve_extruded_contour(
                SurfaceSize {
                    width: 10.0 * magnitude,
                    height: 12.0 * magnitude,
                },
                LogicalCornerRadii {
                    top_start: radius,
                    top_end: radius,
                    bottom_end: radius,
                    bottom_start: radius,
                },
                LayoutDirection::Ltr,
                PhysicalVector {
                    x: magnitude,
                    y: -2.0 * magnitude,
                },
            )
            .unwrap();
            let point = PhysicalVector {
                x: 5.125 * magnitude,
                y: 5.25 * magnitude,
            };
            assert_eq!(convex_contains(&contour, point), None);
            compare_containment(&contour, point);
        }
    }

    #[test]
    fn circle_center_retains_all_equidistant_normals() {
        let contour = circle(PhysicalVector { x: 0.0, y: 0.0 });
        for degree in 0..360 {
            let angle = (degree as f64).to_radians();
            let direction = PhysicalVector {
                x: angle.cos(),
                y: angle.sin(),
            };
            let result = boundary(&contour, PhysicalVector { x: 5.0, y: 5.0 }, direction).unwrap();
            assert!(result.inside);
            assert!((result.weight - 1.0).abs() <= 1e-12);
        }
    }

    #[test]
    fn circle_weight_matches_radial_normal_at_every_angle() {
        let contour = circle(PhysicalVector { x: 0.0, y: 0.0 });
        let direction = PhysicalVector { x: 0.0, y: -1.0 };
        for degree in 0..360 {
            let angle = (degree as f64).to_radians();
            let radial = PhysicalVector {
                x: angle.cos(),
                y: angle.sin(),
            };
            let point = add(PhysicalVector { x: 5.0, y: 5.0 }, scale(radial, 4.5));
            let result = boundary(&contour, point, direction).unwrap();
            assert!(result.inside);
            assert!((result.weight - (-radial.y).max(0.0)).abs() <= 1e-12);
        }
    }

    #[test]
    fn unsupported_ellipses_and_overflow_fail_explicitly() {
        let radius = CornerRadius { x: 5.0, y: 4.0 };
        let contour = crate::resolve_extruded_contour(
            SurfaceSize {
                width: 10.0,
                height: 10.0,
            },
            LogicalCornerRadii {
                top_start: radius,
                top_end: radius,
                bottom_end: radius,
                bottom_start: radius,
            },
            LayoutDirection::Ltr,
            PhysicalVector { x: 0.0, y: 0.0 },
        )
        .unwrap();
        assert!(matches!(
            boundary(
                &contour,
                PhysicalVector { x: 5.0, y: 5.0 },
                PhysicalVector { x: 0.0, y: -1.0 }
            ),
            Err(SurfacePaintError::UnsupportedContour)
        ));
        assert_eq!(
            contains(&contour, PhysicalVector { x: 5.0, y: 5.0 }),
            Err(SurfacePaintError::UnsupportedContour)
        );
        let contour = circle(PhysicalVector {
            x: f64::MAX,
            y: f64::MAX,
        });
        assert!(matches!(
            boundary(
                &contour,
                PhysicalVector {
                    x: f64::MAX / 2.0,
                    y: f64::MAX / 2.0
                },
                PhysicalVector { x: 0.0, y: -1.0 }
            ),
            Err(SurfacePaintError::NumericRange)
        ));
        assert_eq!(
            contains(
                &contour,
                PhysicalVector {
                    x: f64::MAX / 2.0,
                    y: f64::MAX / 2.0
                }
            ),
            Err(SurfacePaintError::NumericRange)
        );
    }
}
