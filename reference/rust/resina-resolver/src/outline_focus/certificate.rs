use super::{OutlineFocusGeometry, OutlineFocusSupport, arithmetic};
use crate::OutlineCurvature;
use resina_model::PhysicalVector;

#[derive(Clone, Copy)]
struct Bounds(f64, f64);
impl Bounds {
    fn exact(value: f64) -> Self {
        Self(value, value)
    }
    fn is(self, value: f64) -> bool {
        self.0 == value && self.1 == value
    }
    fn neg(self) -> Self {
        Self(-self.1, -self.0)
    }
    fn add(self, other: Self) -> Self {
        if self.is(0.0) {
            return other;
        }
        if other.is(0.0) {
            return self;
        }
        if self.0 == self.1 && other.is(-self.0) {
            return Self::exact(0.0);
        }
        Self((self.0 + other.0).next_down(), (self.1 + other.1).next_up())
    }
    fn mul(self, other: Self) -> Self {
        if self.is(0.0) || other.is(0.0) {
            return Self::exact(0.0);
        }
        if self.is(1.0) {
            return other;
        }
        if other.is(1.0) {
            return self;
        }
        if self.is(-1.0) {
            return other.neg();
        }
        if other.is(-1.0) {
            return self.neg();
        }
        let products = [
            self.0 * other.0,
            self.0 * other.1,
            self.1 * other.0,
            self.1 * other.1,
        ];
        Self(
            products
                .into_iter()
                .fold(f64::INFINITY, f64::min)
                .next_down(),
            products
                .into_iter()
                .fold(f64::NEG_INFINITY, f64::max)
                .next_up(),
        )
    }
    fn div(self, other: Self) -> Option<Self> {
        if other.0 <= 0.0 || !other.1.is_finite() {
            return None;
        }
        if self.is(0.0) {
            return Some(self);
        }
        if other.is(1.0) {
            return Some(self);
        }
        if self.0 == self.1 && other.is(self.0) {
            return Some(Self::exact(1.0));
        }
        let quotients = [
            self.0 / other.0,
            self.0 / other.1,
            self.1 / other.0,
            self.1 / other.1,
        ];
        Some(Self(
            quotients
                .into_iter()
                .fold(f64::INFINITY, f64::min)
                .next_down(),
            quotients
                .into_iter()
                .fold(f64::NEG_INFINITY, f64::max)
                .next_up(),
        ))
    }
    fn nonnegative(self) -> Self {
        Self(self.0.max(0.0), self.1.max(0.0))
    }
    fn sqrt(self) -> Self {
        if self.is(0.0) || self.is(1.0) {
            return self;
        }
        Self(
            self.0.max(0.0).sqrt().next_down().max(0.0),
            self.1.sqrt().next_up(),
        )
    }
    fn cube_root(self) -> Option<Self> {
        if !self.0.is_finite() || !self.1.is_finite() || self.0 < 0.0 {
            return None;
        }
        Some(Self(
            arithmetic::cube_root_bounds(self.0).0,
            arithmetic::cube_root_bounds(self.1).1,
        ))
    }
    fn agrees(self, value: f64) -> bool {
        let minimum_magnitude = if self.0 <= 0.0 && self.1 >= 0.0 {
            0.0
        } else {
            self.0.abs().min(self.1.abs())
        };
        let tolerance = 1e-12_f64.max(minimum_magnitude * 1e-12).next_down();
        value.is_finite()
            && self.0.is_finite()
            && self.1.is_finite()
            && (value - self.0).next_up() <= tolerance
            && (self.1 - value).next_up() <= tolerance
    }
}

pub(super) fn certifies(
    geometry: &OutlineFocusGeometry,
    direction: PhysicalVector,
    result: &OutlineFocusSupport,
) -> bool {
    certify(geometry, direction, result).is_some()
}

// Bounds enclose real arithmetic from represented inputs, including normalization error.
fn certify(
    geometry: &OutlineFocusGeometry,
    direction: PhysicalVector,
    result: &OutlineFocusSupport,
) -> Option<()> {
    let exact = Bounds::exact;
    let maximum = exact(direction.x.abs().max(direction.y.abs()));
    let x = exact(direction.x.abs()).div(maximum)?.nonnegative();
    let y = exact(direction.y.abs()).div(maximum)?.nonnegative();
    let magnitude = x.mul(x).add(y.mul(y)).nonnegative().sqrt();
    let mut nx = x.div(magnitude)?.nonnegative();
    let mut ny = y.div(magnitude)?.nonnegative();
    let right = direction.x >= 0.0;
    let bottom = direction.y >= 0.0;
    let corner = geometry.outline.corners()[match (right, bottom) {
        (false, false) => 0,
        (true, false) => 1,
        (true, true) => 2,
        (false, true) => 3,
    }];
    let size = geometry.outline.size();
    let cx = if right {
        exact(size.width).add(exact(corner.x).neg())
    } else {
        exact(corner.x)
    };
    let cy = if bottom {
        exact(size.height).add(exact(corner.y).neg())
    } else {
        exact(corner.y)
    };
    let scale = (corner.x * result.normal.x.abs()).max(corner.y * result.normal.y.abs());
    let (length, u, v) = if scale == 0.0 {
        (exact(0.0), exact(0.0), exact(0.0))
    } else {
        let wx = nx
            .mul(exact(corner.x))
            .nonnegative()
            .div(exact(scale))?
            .nonnegative();
        let wy = ny
            .mul(exact(corner.y))
            .nonnegative()
            .div(exact(scale))?
            .nonnegative();
        match geometry.outline.curvature() {
            OutlineCurvature::Circular => {
                let factor = wx.mul(wx).add(wy.mul(wy)).nonnegative().sqrt();
                (exact(scale).mul(factor), wx.div(factor)?, wy.div(factor)?)
            }
            OutlineCurvature::Quartic => {
                let sum = wx
                    .mul(wx.cube_root()?)
                    .add(wy.mul(wy.cube_root()?))
                    .nonnegative();
                let factor = sum.sqrt().mul(sum.sqrt().sqrt());
                (
                    exact(scale).mul(factor),
                    wx.div(factor)?.nonnegative().cube_root()?,
                    wy.div(factor)?.nonnegative().cube_root()?,
                )
            }
        }
    };
    if !right {
        nx = nx.neg();
    }
    if !bottom {
        ny = ny.neg();
    }
    let dx = exact(corner.x).mul(u);
    let dy = exact(corner.y).mul(v);
    let mut px = cx.add(if right { dx } else { dx.neg() });
    let mut py = cy.add(if bottom { dy } else { dy.neg() });
    let mut support = cx.mul(nx).add(cy.mul(ny)).add(length);
    if arithmetic::positive_dot(direction, geometry.side_offset) {
        px = px.add(exact(geometry.side_offset.x));
        py = py.add(exact(geometry.side_offset.y));
        support = support.add(
            exact(geometry.side_offset.x)
                .mul(nx)
                .add(exact(geometry.side_offset.y).mul(ny)),
        );
    }
    if !nx.agrees(result.normal.x) || !ny.agrees(result.normal.y) {
        return None;
    }
    for (boundary, outset) in [
        (&result.silhouette, 0.0),
        (&result.inner, geometry.inner_outset),
        (&result.outer, geometry.outer_outset),
    ] {
        if !px.add(exact(outset).mul(nx)).agrees(boundary.point.x)
            || !py.add(exact(outset).mul(ny)).agrees(boundary.point.y)
            || !support.add(exact(outset)).agrees(boundary.support)
        {
            return None;
        }
    }
    Some(())
}
