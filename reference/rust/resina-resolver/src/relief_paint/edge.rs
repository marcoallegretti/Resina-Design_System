use crate::{OutlineCurvature, SurfaceOutline};
use num_bigint::BigUint;
use resina_model::PhysicalVector;

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

fn dyadic(value: f64) -> (u64, i32) {
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
