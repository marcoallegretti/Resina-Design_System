use super::edge::dyadic;
use crate::{OutlineCurvature, ReliefKind, SurfaceRelief};
use num_bigint::BigUint;
use resina_model::PhysicalBounds;

/// Whether all four vertices of the closed box lie in the edge interior at `inset` and in the
/// uniform relief plateau, where every relief gate equals one, decided with exact rationals of the
/// represented inputs. Both regions are convex, so the vertices decide the whole box.
pub(super) fn contains_box(surface: &SurfaceRelief, inset: f64, bounds: PhysicalBounds) -> bool {
    let values = [bounds.x, bounds.y, bounds.width, bounds.height];
    if values
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return false;
    }
    let outline = surface.outline();
    let size = outline.size();
    let profile = surface.profile();
    let corners = outline.corners();
    let lattice = [
        size.width,
        size.height,
        inset,
        profile.shoulder_width,
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        corners[0].x,
        corners[0].y,
        corners[1].x,
        corners[1].y,
        corners[2].x,
        corners[2].y,
        corners[3].x,
        corners[3].y,
    ];
    let parts = lattice.map(dyadic);
    let Some(scale) = parts
        .iter()
        .filter(|(mantissa, _)| *mantissa != 0)
        .map(|(_, exponent)| *exponent)
        .min()
    else {
        return false;
    };
    let [
        width,
        height,
        inset,
        shoulder,
        x,
        y,
        box_width,
        box_height,
        radii @ ..,
    ] = parts.map(|(mantissa, exponent)| {
        if mantissa == 0 {
            BigUint::ZERO
        } else {
            BigUint::from(mantissa) << (exponent - scale) as usize
        }
    });
    let face = Face {
        width,
        height,
        inset,
        shoulder: (profile.kind != ReliefKind::Flat).then_some(shoulder),
        exponent: match outline.curvature() {
            OutlineCurvature::Circular => 2,
            OutlineCurvature::Quartic => 4,
        },
        radii,
    };
    let right = &x + box_width;
    let bottom = &y + box_height;
    [(&x, &y), (&right, &y), (&right, &bottom), (&x, &bottom)]
        .into_iter()
        .all(|(px, py)| face.contains(px, py))
}

struct Face {
    width: BigUint,
    height: BigUint,
    inset: BigUint,
    shoulder: Option<BigUint>,
    exponent: u32,
    radii: [BigUint; 8],
}

impl Face {
    fn contains(&self, x: &BigUint, y: &BigUint) -> bool {
        if x > &self.width || y > &self.height {
            return false;
        }
        let right = &self.width - x;
        let bottom = &self.height - y;
        let margin = self
            .shoulder
            .as_ref()
            .map_or(&self.inset, |shoulder| shoulder.max(&self.inset));
        if [x, y, &right, &bottom]
            .into_iter()
            .any(|distance| distance < margin)
        {
            return false;
        }
        let n = self.exponent;
        let distances = [(x, y), (&right, y), (&right, &bottom), (x, &bottom)];
        for (axes, (dx, dy)) in self.radii.as_chunks::<2>().0.iter().zip(distances) {
            let [a, b] = axes;
            if a.bits() == 0 || b.bits() == 0 {
                continue;
            }
            let u = if a > dx {
                (a - dx).pow(n)
            } else {
                BigUint::ZERO
            };
            let v = if b > dy {
                (b - dy).pow(n)
            } else {
                BigUint::ZERO
            };
            if a > &self.inset && b > &self.inset {
                let ai = (a - &self.inset).pow(n);
                let bi = (b - &self.inset).pow(n);
                if &u * &bi + &v * &ai > ai * bi {
                    return false;
                }
            }
            // The spec/82 plateau F >= tau, with tau = n*(w/a + w/b), multiplied by a^n*b^n.
            if let Some(w) = &self.shoulder {
                let tau = BigUint::from(n) * w * (a + b) * a.pow(n - 1) * b.pow(n - 1);
                let (an, bn) = (a.pow(n), b.pow(n));
                if &u * &bn + &v * &an + tau > an * bn {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SurfaceReliefProfile, resolve_surface_outline, resolve_surface_relief};
    use resina_environment::LayoutDirection;
    use resina_model::{CornerRadius, LogicalCornerRadii, SurfaceSize};

    #[test]
    fn right_and_bottom_vertices_are_exact_sums() {
        let square = CornerRadius { x: 0.0, y: 0.0 };
        let outline = resolve_surface_outline(
            SurfaceSize {
                width: 96.0,
                height: 96.0,
            },
            LogicalCornerRadii {
                top_start: square,
                top_end: square,
                bottom_end: square,
                bottom_start: square,
            },
            LayoutDirection::Ltr,
            OutlineCurvature::Circular,
            1.0,
        )
        .unwrap();
        let surface = resolve_surface_relief(
            outline,
            SurfaceReliefProfile {
                kind: ReliefKind::Convex,
                body_thickness: 2.0,
                amplitude: 0.75,
                shoulder_width: 1.5,
            },
        )
        .unwrap();
        let past = 1.5_f64.next_up();
        assert_eq!(past + 93.0, 94.5);
        let cases = [
            (past, 1.5, 93.0, 93.0, false),
            (1.5, past, 93.0, 93.0, false),
            (1.5, 1.5, 93.0, 93.0, true),
            (
                past,
                past,
                93.0 - 2.0_f64.powi(-46),
                93.0 - 2.0_f64.powi(-46),
                true,
            ),
        ];
        for (x, y, width, height, contained) in cases {
            let bounds = PhysicalBounds {
                x,
                y,
                width,
                height,
            };
            assert_eq!(contains_box(&surface, 1.0, bounds), contained, "{bounds:?}");
        }
    }
}
