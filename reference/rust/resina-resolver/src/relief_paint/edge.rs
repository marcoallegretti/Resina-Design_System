use super::Shared;
use crate::{OutlineCurvature, SurfaceOutline, surface_relief::Recalled};
use num_bigint::BigUint;
use resina_model::PhysicalVector;
use std::sync::PoisonError;

#[derive(Clone, Copy)]
struct Bounds(f64, f64);
impl Bounds {
    fn exact(value: f64) -> Self {
        Self(value, value)
    }
    fn subtract(self, other: Self) -> Self {
        Self((self.0 - other.1).next_down(), (self.1 - other.0).next_up())
    }
    fn positive(self) -> Self {
        Self(self.0.max(0.0), self.1.max(0.0))
    }
    fn divide(self, other: Self) -> Option<Self> {
        (other.0 > 0.0).then(|| {
            Self(
                (self.0 / other.1).next_down().max(0.0),
                (self.1 / other.0).next_up(),
            )
        })
    }
    fn power(self, exponent: u32) -> Self {
        let mut result = self;
        for _ in 1..exponent {
            result = Self(
                (result.0 * self.0).next_down().max(0.0),
                (result.1 * self.1).next_up(),
            );
        }
        result
    }
}

pub(super) fn contains(outline: &SurfaceOutline, inset: f64, point: PhysicalVector) -> bool {
    decide(
        outline,
        inset,
        point,
        &axis(outline, inset, point.x, true),
        &axis(outline, inset, point.y, false),
    )
}

#[derive(Clone, Copy)]
pub(crate) struct EdgeAxis {
    outside: bool,
    uncertain: bool,
    ratios: [Option<Bounds>; 4],
}

fn exponent(outline: &SurfaceOutline) -> u32 {
    match outline.curvature() {
        OutlineCurvature::Circular => 2,
        OutlineCurvature::Quartic => 4,
    }
}

fn axis(outline: &SurfaceOutline, inset: f64, coordinate: f64, horizontal: bool) -> EdgeAxis {
    let size = outline.size();
    let near = Bounds::exact(coordinate);
    let far = Bounds::exact(if horizontal { size.width } else { size.height }).subtract(near);
    let distances = if horizontal {
        [near, far, far, near]
    } else {
        [near, near, far, far]
    };
    let mut ratios = [None; 4];
    for ((ratio, corner), distance) in ratios.iter_mut().zip(outline.corners()).zip(distances) {
        if corner.x <= inset || corner.y <= inset {
            continue;
        }
        let radius = if horizontal { corner.x } else { corner.y };
        *ratio = Bounds::exact(radius)
            .subtract(distance)
            .positive()
            .divide(Bounds::exact(radius).subtract(Bounds::exact(inset)))
            .map(|value| value.power(exponent(outline)));
    }
    EdgeAxis {
        outside: coordinate < inset || far.1 < inset,
        uncertain: far.0 < inset,
        ratios,
    }
}

fn decide(
    outline: &SurfaceOutline,
    inset: f64,
    point: PhysicalVector,
    x: &EdgeAxis,
    y: &EdgeAxis,
) -> bool {
    if x.outside || y.outside {
        return false;
    }
    let mut uncertain = x.uncertain || y.uncertain;
    for (i, corner) in outline.corners().into_iter().enumerate() {
        if corner.x <= inset || corner.y <= inset {
            continue;
        }
        if let (Some(u), Some(v)) = (x.ratios[i], y.ratios[i]) {
            if (u.0 + v.0).next_down() > 1.0 {
                return false;
            }
            uncertain |= (u.1 + v.1).next_up() > 1.0;
        } else {
            uncertain = true;
        }
    }
    !uncertain || exact_contains(outline, inset, point, exponent(outline))
}

/// Evaluates [`contains`] for many points, reusing the terms of recently seen coordinates.
pub(super) struct EdgeSampler<'a> {
    outline: &'a SurfaceOutline,
    inset: f64,
    shared: Option<Shared<'a>>,
    axes: [Recalled<'a, EdgeAxis>; 2],
}

impl<'a> EdgeSampler<'a> {
    /// A sampler that reuses the axes `shared` keeps for this outline and inset, and adds the ones
    /// it evaluates.
    pub(super) fn new(outline: &'a SurfaceOutline, inset: f64, shared: Option<Shared<'a>>) -> Self {
        let kept = shared.map(|shared| &shared.kept.edge_axes);
        Self {
            outline,
            inset,
            shared,
            axes: [
                Recalled::new(kept.map(|axes| &axes[0])),
                Recalled::new(kept.map(|axes| &axes[1])),
            ],
        }
    }

    pub(super) fn contains(&mut self, point: PhysicalVector) -> bool {
        let (outline, inset) = (self.outline, self.inset);
        let [x_axes, y_axes] = &mut self.axes;
        let x = x_axes.get(point.x, || axis(outline, inset, point.x, true));
        let y = y_axes.get(point.y, || axis(outline, inset, point.y, false));
        decide(outline, inset, point, x, y)
    }
}

impl Drop for EdgeSampler<'_> {
    fn drop(&mut self) {
        let [x, y] = &mut self.axes;
        let axes = [x.take_added(), y.take_added()];
        if let Some(shared) = self.shared
            && axes.iter().any(|added| !added.is_empty())
        {
            let mut added = shared.added.lock().unwrap_or_else(PoisonError::into_inner);
            for (kept, mut axis) in added.edge_axes.iter_mut().zip(axes) {
                kept.append(&mut axis);
            }
        }
    }
}

pub(super) fn dyadic(value: f64) -> (u64, i32) {
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let significand = bits & ((1_u64 << 52) - 1);
    if exponent == 0 {
        (significand, -1074)
    } else {
        (significand | (1_u64 << 52), exponent - 1075)
    }
}

fn exact_contains(
    outline: &SurfaceOutline,
    inset: f64,
    point: PhysicalVector,
    exponent: u32,
) -> bool {
    let size = outline.size();
    let corners = outline.corners();
    let values = [
        size.width,
        size.height,
        inset,
        point.x,
        point.y,
        corners[0].x,
        corners[0].y,
        corners[1].x,
        corners[1].y,
        corners[2].x,
        corners[2].y,
        corners[3].x,
        corners[3].y,
    ];
    let parts = values.map(dyadic);
    let scale = parts
        .iter()
        .filter(|(mantissa, _)| *mantissa != 0)
        .map(|(_, exponent)| *exponent)
        .min()
        .expect("positive outline dimensions");
    let integers = parts.map(|(mantissa, exponent)| {
        if mantissa == 0 {
            BigUint::ZERO
        } else {
            BigUint::from(mantissa) << (exponent - scale) as usize
        }
    });
    let [width, height, inset, x, y, ref rest @ ..] = integers;
    // All callers first establish outer containment and finite nonnegative inputs.
    let right = &width - &x;
    let bottom = &height - &y;
    if x < inset || y < inset || right < inset || bottom < inset {
        return false;
    }
    let distances = [(&x, &y), (&right, &y), (&right, &bottom), (&x, &bottom)];
    for (axes, (dx, dy)) in rest.as_chunks::<2>().0.iter().zip(distances) {
        let [a, b] = axes;
        if a <= &inset || b <= &inset {
            continue;
        }
        let u = if a > dx { a - dx } else { BigUint::ZERO };
        let v = if b > dy { b - dy } else { BigUint::ZERO };
        let ax = (a - &inset).pow(exponent);
        let by = (b - &inset).pow(exponent);
        if u.pow(exponent) * &by + v.pow(exponent) * &ax > ax * by {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve_surface_outline;
    use resina_environment::LayoutDirection;
    use resina_model::{CornerRadius, LogicalCornerRadii, SurfaceSize};

    fn unsplit_contains(outline: &SurfaceOutline, inset: f64, point: PhysicalVector) -> bool {
        let size = outline.size();
        if point.x < inset || point.y < inset {
            return false;
        }
        let right = Bounds::exact(size.width).subtract(Bounds::exact(point.x));
        let bottom = Bounds::exact(size.height).subtract(Bounds::exact(point.y));
        if right.1 < inset || bottom.1 < inset {
            return false;
        }
        let mut uncertain = right.0 < inset || bottom.0 < inset;
        let exponent = match outline.curvature() {
            OutlineCurvature::Circular => 2,
            OutlineCurvature::Quartic => 4,
        };
        let distances = [
            (Bounds::exact(point.x), Bounds::exact(point.y)),
            (right, Bounds::exact(point.y)),
            (right, bottom),
            (Bounds::exact(point.x), bottom),
        ];
        for (corner, (dx, dy)) in outline.corners().into_iter().zip(distances) {
            if corner.x <= inset || corner.y <= inset {
                continue;
            }
            let ratio = |axis, distance| {
                Bounds::exact(axis)
                    .subtract(distance)
                    .positive()
                    .divide(Bounds::exact(axis).subtract(Bounds::exact(inset)))
                    .map(|value| value.power(exponent))
            };
            if let (Some(u), Some(v)) = (ratio(corner.x, dx), ratio(corner.y, dy)) {
                if (u.0 + v.0).next_down() > 1.0 {
                    return false;
                }
                uncertain |= (u.1 + v.1).next_up() > 1.0;
            } else {
                uncertain = true;
            }
        }
        !uncertain || exact_contains(outline, inset, point, exponent)
    }

    #[test]
    fn split_and_sampled_terms_reproduce_unsplit_containment() {
        let radius = |r: f64| CornerRadius { x: r, y: r };
        let mut compared = 0_usize;
        let mut inside = 0_usize;
        for (size, radii, curvature, inset) in [
            ((166.0, 80.0), [24.0; 4], OutlineCurvature::Circular, 1.0),
            (
                (120.0, 60.0),
                [20.0, 0.0, 1.0, 6.0],
                OutlineCurvature::Circular,
                1.0,
            ),
            ((48.0, 48.0), [24.0; 4], OutlineCurvature::Circular, 2.0),
            (
                (90.0, 44.0),
                [16.0, 12.0, 16.0, 12.0],
                OutlineCurvature::Quartic,
                1.0,
            ),
            ((31.0, 13.0), [6.5; 4], OutlineCurvature::Circular, 0.5),
        ] {
            let outline = resolve_surface_outline(
                SurfaceSize {
                    width: size.0,
                    height: size.1,
                },
                LogicalCornerRadii {
                    top_start: radius(radii[0]),
                    top_end: radius(radii[1]),
                    bottom_end: radius(radii[2]),
                    bottom_start: radius(radii[3]),
                },
                LayoutDirection::Ltr,
                curvature,
                inset,
            )
            .unwrap();
            let mut points = Vec::new();
            for (scale, origin) in [(1.25, -1.3), (2.0, -0.75)] {
                let columns = ((size.0 - 2.0 * origin) * scale).ceil() as u32;
                let rows = ((size.1 - 2.0 * origin) * scale).ceil() as u32;
                for y in 0..rows {
                    for x in 0..columns {
                        points.push(PhysicalVector {
                            x: origin + (f64::from(x) + 0.5) / scale,
                            y: origin + (f64::from(y) + 0.5) / scale,
                        });
                    }
                }
            }
            for along in [0.25, 3.0, size.0 / 2.0, size.0 - 0.5] {
                for edge in [inset, size.1 - inset] {
                    for y in [edge.next_down(), edge, edge.next_up()] {
                        points.push(PhysicalVector { x: along, y });
                        points.push(PhysicalVector {
                            x: y,
                            y: along.min(size.1),
                        });
                    }
                }
            }
            let mut sampler = EdgeSampler::new(&outline, inset, None);
            for point in points {
                let expected = unsplit_contains(&outline, inset, point);
                assert_eq!(contains(&outline, inset, point), expected, "{point:?}");
                assert_eq!(sampler.contains(point), expected, "{point:?}");
                compared += 1;
                inside += usize::from(expected);
            }
        }
        assert!(compared > 40_000 && inside > 20_000, "{compared} {inside}");
    }
}
