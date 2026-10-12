use num_bigint::{BigInt, BigUint, Sign};
use resina_model::PhysicalVector;
use std::cmp::Ordering;

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

pub(super) fn positive_dot(a: PhysicalVector, b: PhysicalVector) -> bool {
    let product = |x: f64, y: f64| {
        let (mx, ex) = dyadic(x);
        let (my, ey) = dyadic(y);
        let sign = if x.is_sign_negative() == y.is_sign_negative() {
            Sign::Plus
        } else {
            Sign::Minus
        };
        (
            BigInt::from_biguint(sign, BigUint::from(mx) * BigUint::from(my)),
            ex + ey,
        )
    };
    let (x, ex) = product(a.x, b.x);
    let (y, ey) = product(a.y, b.y);
    let exponent = ex.min(ey);
    ((x << (ex - exponent) as usize) + (y << (ey - exponent) as usize)).sign() == Sign::Plus
}

fn compare_cube(root: f64, value: f64) -> Ordering {
    let (m, e) = dyadic(root);
    let (mv, ev) = dyadic(value);
    let exponent = (3 * e).min(ev);
    (BigUint::from(m).pow(3) << (3 * e - exponent) as usize)
        .cmp(&(BigUint::from(mv) << (ev - exponent) as usize))
}

// Select the lower adjacent float with exact integer cubes; libc precision is unspecified.
pub(super) fn cube_root(value: f64) -> f64 {
    debug_assert!(value.is_finite() && value >= 0.0);
    if value == 0.0 || value == 1.0 {
        return value;
    }
    let seed = value.cbrt();
    if seed.is_finite() && seed > 0.0 && seed.next_up().is_finite() {
        let lower = if compare_cube(seed, value) == Ordering::Greater {
            seed.next_down()
        } else {
            seed
        };
        if compare_cube(lower, value) != Ordering::Greater
            && compare_cube(lower.next_up(), value) == Ordering::Greater
        {
            return lower;
        }
    }
    let mut low = 0_u64;
    let mut high = value.max(1.0).to_bits();
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if compare_cube(f64::from_bits(middle), value) == Ordering::Greater {
            high = middle;
        } else {
            low = middle;
        }
    }
    f64::from_bits(low)
}

pub(super) fn cube_root_bounds(value: f64) -> (f64, f64) {
    let lower = cube_root(value);
    let upper = if compare_cube(lower, value) == Ordering::Equal {
        lower
    } else {
        lower.next_up()
    };
    (lower, upper)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_are_bracketed_even_for_subnormal_inputs() {
        for value in [
            f64::from_bits(1),
            1e-300,
            1e-100,
            0.125,
            0.5,
            1.0_f64.next_down(),
            1.0_f64.next_up(),
            8.0,
            1e300,
        ] {
            let lower = cube_root(value);
            assert!(compare_cube(lower, value) != Ordering::Greater);
            assert_eq!(compare_cube(lower.next_up(), value), Ordering::Greater);
        }
    }

    #[test]
    fn projection_sign_does_not_drop_cancelled_products() {
        let x = 1.0_f64.next_up();
        let y = 1.0_f64.next_down();
        assert_eq!(x * y - 1.0, 0.0);
        assert!(positive_dot(
            PhysicalVector { x, y: -1.0 },
            PhysicalVector { x: y, y: 1.0 }
        ));
        assert!(!positive_dot(
            PhysicalVector { x: 1.0, y: -1.0 },
            PhysicalVector { x: 1.0, y: 1.0 }
        ));
    }
}
