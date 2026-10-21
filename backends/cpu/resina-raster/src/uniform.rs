use resina_model::{ContourSegment, PhysicalVector};
use resina_resolver::{ExtrudedContourResult, PlacedContour};

#[derive(Clone, Copy)]
pub(crate) struct SampleBox {
    pub min: PhysicalVector,
    pub max: PhysicalVector,
}

impl SampleBox {
    pub(crate) fn contains(self, other: Self) -> bool {
        self.min.x <= other.min.x
            && self.min.y <= other.min.y
            && other.max.x <= self.max.x
            && other.max.y <= self.max.y
    }

    fn translated(self, offset: PhysicalVector) -> Self {
        Self {
            min: subtract(self.min, offset),
            max: subtract(self.max, offset),
        }
    }

    fn support(self, normal: PhysicalVector) -> PhysicalVector {
        PhysicalVector {
            x: if normal.x < 0.0 {
                self.min.x
            } else {
                self.max.x
            },
            y: if normal.y < 0.0 {
                self.min.y
            } else {
                self.max.y
            },
        }
    }

    fn magnitude(self) -> f64 {
        self.min
            .x
            .abs()
            .max(self.min.y.abs())
            .max(self.max.x.abs())
            .max(self.max.y.abs())
    }
}

pub(crate) fn supported(contour: &ExtrudedContourResult) -> bool {
    let Some(bounds) = contour.bounds() else {
        return false;
    };
    let magnitude = bounds
        .x
        .abs()
        .max(bounds.y.abs())
        .max(bounds.width)
        .max(bounds.height);
    (1e-100..=1e100).contains(&magnitude)
        && !contour.segments().is_empty()
        && contour.segments().iter().all(|segment| match *segment {
            ContourSegment::Line { from, to } => from != to,
            ContourSegment::Arc {
                radii, start, end, ..
            } => {
                radii.x == radii.y
                    && radii.x > 0.0
                    && start.x * end.x >= 0.0
                    && start.y * end.y >= 0.0
                    && cross(start, end) > 0.0
            }
        })
}

pub(crate) fn placed_supported(contour: &PlacedContour) -> bool {
    contour.offset().x.abs().max(contour.offset().y.abs()) <= 1e100 && supported(contour.contour())
}

pub(crate) fn disjoint(region: SampleBox, contour: &ExtrudedContourResult) -> bool {
    let Some(bounds) = contour.bounds() else {
        return false;
    };
    let magnitude = region
        .magnitude()
        .max(bounds.x.abs())
        .max(bounds.y.abs())
        .max(bounds.width)
        .max(bounds.height);
    if magnitude > 1e100 {
        return false;
    }
    let tolerance = 64.0 * f64::EPSILON * magnitude;
    bounds.x - region.max.x > tolerance
        || bounds.y - region.max.y > tolerance
        || (region.min.x - bounds.x) - bounds.width > tolerance
        || (region.min.y - bounds.y) - bounds.height > tolerance
}

pub(crate) fn placed_disjoint(region: SampleBox, contour: &PlacedContour) -> bool {
    disjoint(region.translated(contour.offset()), contour.contour())
}

pub(crate) fn placed_inside(region: SampleBox, contour: &PlacedContour) -> bool {
    let region = region.translated(contour.offset());
    let Some(bounds) = contour.contour().bounds() else {
        return false;
    };
    let magnitude = region
        .magnitude()
        .max(bounds.x.abs())
        .max(bounds.y.abs())
        .max(bounds.width)
        .max(bounds.height);
    if !(1e-100..=1e100).contains(&magnitude) {
        return false;
    }
    let tolerance = 64.0 * f64::EPSILON * magnitude;
    // A box's support is attained at a vertex. Each canonical quarter arc
    // stays in one normal quadrant, so that same vertex maximizes every
    // supporting normal on the arc. Ambiguous predicates require point sampling.
    for segment in contour.contour().segments() {
        match *segment {
            ContourSegment::Line { from, to } => {
                let normal = PhysicalVector {
                    x: to.y - from.y,
                    y: from.x - to.x,
                };
                let offset = subtract(region.support(normal), from);
                if dot(offset, normal) >= -tolerance * (normal.x.abs() + normal.y.abs()) {
                    return false;
                }
            }
            ContourSegment::Arc {
                center,
                radii,
                start,
                end,
            } => {
                let normal = PhysicalVector {
                    x: start.x + end.x,
                    y: start.y + end.y,
                };
                let offset = subtract(region.support(normal), center);
                if dot(offset, start) - radii.x >= -tolerance
                    || dot(offset, end) - radii.x >= -tolerance
                {
                    return false;
                }
                let a = cross(start, offset);
                let b = cross(offset, end);
                if a > tolerance && b > tolerance {
                    if offset.x.hypot(offset.y) - radii.x >= -tolerance {
                        return false;
                    }
                } else if a >= -tolerance && b >= -tolerance {
                    return false;
                }
            }
        }
    }
    true
}

/// Whether every point of the box lies outside the convex contour by more than the tolerance:
/// beyond its bounds, beyond the supporting line of one of its line segments, or beyond the
/// circle of one of its arcs inside that arc's sector, where the arc is the contour's boundary.
/// Each separation exceeds the arithmetic error of point evaluation, so every point of the box is
/// evaluated as outside.
pub(crate) fn outside(region: SampleBox, contour: &ExtrudedContourResult) -> bool {
    if disjoint(region, contour) {
        return true;
    }
    let Some(bounds) = contour.bounds() else {
        return false;
    };
    let magnitude = region
        .magnitude()
        .max(bounds.x.abs())
        .max(bounds.y.abs())
        .max(bounds.width)
        .max(bounds.height);
    if !(1e-100..=1e100).contains(&magnitude) {
        return false;
    }
    let tolerance = 64.0 * f64::EPSILON * magnitude;
    let vertices = [
        region.min,
        PhysicalVector {
            x: region.max.x,
            y: region.min.y,
        },
        region.max,
        PhysicalVector {
            x: region.min.x,
            y: region.max.y,
        },
    ];
    contour.segments().iter().any(|segment| match *segment {
        ContourSegment::Line { from, to } => {
            let normal = PhysicalVector {
                x: to.y - from.y,
                y: from.x - to.x,
            };
            let nearest = region.support(PhysicalVector {
                x: -normal.x,
                y: -normal.y,
            });
            dot(subtract(nearest, from), normal) > tolerance * (normal.x.abs() + normal.y.abs())
        }
        ContourSegment::Arc {
            center,
            radii,
            start,
            end,
        } => {
            let nearest = PhysicalVector {
                x: center.x.clamp(region.min.x, region.max.x),
                y: center.y.clamp(region.min.y, region.max.y),
            };
            let offset = subtract(nearest, center);
            vertices.iter().all(|vertex| {
                let offset = subtract(*vertex, center);
                cross(start, offset) > tolerance && cross(offset, end) > tolerance
            }) && offset.x.hypot(offset.y) - radii.x > tolerance
        }
    })
}

pub(crate) fn placed_outside(region: SampleBox, contour: &PlacedContour) -> bool {
    outside(region.translated(contour.offset()), contour.contour())
}

fn subtract(a: PhysicalVector, b: PhysicalVector) -> PhysicalVector {
    PhysicalVector {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}
fn dot(a: PhysicalVector, b: PhysicalVector) -> f64 {
    a.x * b.x + a.y * b.y
}
fn cross(a: PhysicalVector, b: PhysicalVector) -> f64 {
    a.x * b.y - a.y * b.x
}
