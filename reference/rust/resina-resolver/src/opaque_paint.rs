use crate::{ExtrudedContourResult, OpaqueSurfaceIr, PlacedContour, SrgbFallback};
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
        if !boundary(self.geometry().silhouette(), point, direction)?.inside {
            return Ok(None);
        }
        if !placed_boundary(self.geometry().edge_interior(), point, direction)?.inside {
            return Ok(Some(self.edge().color().clone()));
        }
        if !boundary(self.geometry().front(), point, direction)?.inside {
            return Ok(Some(self.pigment().side().clone()));
        }
        let highlight = placed_boundary(self.geometry().highlight_outer(), point, direction)?;
        if self.highlight_width() > 0.0
            && highlight.inside
            && !placed_boundary(self.geometry().content(), point, direction)?.inside
        {
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
                }
            }
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
    }
}
