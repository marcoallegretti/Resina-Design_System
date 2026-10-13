use crate::{RasterError, RasterImage, Viewport, validate};
use num_bigint::BigInt;
use num_rational::Ratio;
use num_traits::float::FloatCore;
use num_traits::{One, Signed, ToPrimitive, Zero};
use resina_model::PhysicalVector;
use resina_resolver::{OutlineFocusGeometry, OutlineFocusIr, OutlineFocusSupport};
use std::{cmp::Ordering, collections::VecDeque};

type Rational = Ratio<BigInt>;
const MAX_VERTICES: usize = 4096;
const MAX_WORK: u64 = 134_217_728;

struct Budget(u64);
impl Budget {
    fn spend(&mut self, amount: usize) -> Result<(), RasterError> {
        self.0 = self
            .0
            .checked_sub(amount as u64)
            .ok_or(RasterError::ResourceLimit)?;
        Ok(())
    }
}

fn rational(value: f64) -> Rational {
    assert!(value.is_finite(), "validated finite geometry and viewport");
    let (mantissa, exponent, sign) = FloatCore::integer_decode(value);
    let numerator = BigInt::from(mantissa) * BigInt::from(sign);
    if exponent >= 0 {
        Rational::from_integer(numerator << exponent as usize)
    } else {
        Rational::new(numerator, BigInt::one() << (-exponent) as usize)
    }
}
fn integer(value: u32) -> Rational {
    Rational::from_integer(value.into())
}

#[derive(Clone, Copy)]
struct Interval {
    low: f64,
    high: f64,
}
impl Interval {
    fn enclose(value: &Rational) -> Option<Self> {
        let approximation = value.numer().to_f64()? / value.denom().to_f64()?;
        if !approximation.is_finite() {
            return None;
        }
        if rational(approximation) == *value {
            return Some(Self {
                low: approximation,
                high: approximation,
            });
        }
        let low = approximation.next_down();
        let high = approximation.next_up();
        if !low.is_finite()
            || !high.is_finite()
            || rational(low) > *value
            || rational(high) < *value
        {
            return None;
        }
        Some(Self { low, high })
    }
    fn subtract(self, other: Self) -> Option<Self> {
        Self::finite(
            (self.low - other.high).next_down(),
            (self.high - other.low).next_up(),
        )
    }
    fn multiply(self, other: Self) -> Option<Self> {
        let values = [
            self.low * other.low,
            self.low * other.high,
            self.high * other.low,
            self.high * other.high,
        ];
        if values.iter().any(|value| !value.is_finite()) {
            return None;
        }
        Self::finite(
            values.into_iter().fold(f64::INFINITY, f64::min).next_down(),
            values
                .into_iter()
                .fold(f64::NEG_INFINITY, f64::max)
                .next_up(),
        )
    }
    fn finite(low: f64, high: f64) -> Option<Self> {
        (low.is_finite() && high.is_finite()).then_some(Self { low, high })
    }
}

#[derive(Clone)]
struct Point {
    x: Rational,
    y: Rational,
    enclosure: Option<(Interval, Interval)>,
}
impl PartialEq for Point {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}
impl Eq for Point {}
impl Point {
    fn new(x: Rational, y: Rational) -> Self {
        let enclosure = Interval::enclose(&x).zip(Interval::enclose(&y));
        Self { x, y, enclosure }
    }
    fn device(point: PhysicalVector, viewport: Viewport) -> Self {
        let scale = rational(viewport.pixels_per_unit);
        Self::new(
            (rational(point.x) - rational(viewport.origin.x)) * &scale,
            (rational(point.y) - rational(viewport.origin.y)) * scale,
        )
    }
    fn coordinate(&self, axis: usize) -> &Rational {
        if axis == 0 { &self.x } else { &self.y }
    }
}

fn orientation(
    a: &Point,
    b: &Point,
    p: &Point,
    budget: &mut Budget,
) -> Result<Ordering, RasterError> {
    budget.spend(1)?;
    if let Some(value) = filtered_orientation(a, b, p) {
        if value.low > 0.0 {
            return Ok(Ordering::Greater);
        }
        if value.high < 0.0 {
            return Ok(Ordering::Less);
        }
    }
    Ok(((&b.x - &a.x) * (&p.y - &a.y) - (&b.y - &a.y) * (&p.x - &a.x)).cmp(&Rational::zero()))
}
fn filtered_orientation(a: &Point, b: &Point, p: &Point) -> Option<Interval> {
    let (ax, ay) = a.enclosure?;
    let (bx, by) = b.enclosure?;
    let (px, py) = p.enclosure?;
    bx.subtract(ax)?
        .multiply(py.subtract(ay)?)?
        .subtract(by.subtract(ay)?.multiply(px.subtract(ax)?)?)
}

struct Plane {
    x: Rational,
    y: Rational,
    support: Rational,
}
impl Plane {
    fn intersection(&self, other: &Self, budget: &mut Budget) -> Result<Point, RasterError> {
        budget.spend(1)?;
        let determinant = &self.x * &other.y - &self.y * &other.x;
        if determinant.is_zero() {
            return Err(RasterError::FocusPrecision("parallel support planes"));
        }
        Ok(Point::new(
            (&self.support * &other.y - &self.y * &other.support) / &determinant,
            (&self.x * &other.support - &self.support * &other.x) / determinant,
        ))
    }
    fn excludes_intersection(
        &self,
        a: &Self,
        b: &Self,
        budget: &mut Budget,
    ) -> Result<bool, RasterError> {
        budget.spend(1)?;
        let determinant = &a.x * &b.y - &a.y * &b.x;
        if determinant.is_zero() {
            return Err(RasterError::FocusPrecision("parallel support planes"));
        }
        let x = &a.support * &b.y - &a.y * &b.support;
        let y = &a.x * &b.support - &a.support * &b.x;
        let value = &self.x * x + &self.y * y - &self.support * &determinant;
        Ok(if determinant.is_positive() {
            value.is_positive()
        } else {
            value.is_negative()
        })
    }
}

struct Polygon(Vec<Point>);

struct PixelRow {
    polygon: Polygon,
    full_columns: Option<(u32, u32)>,
    y: u32,
}

impl PixelRow {
    fn new(polygon: Polygon, y: u32) -> Self {
        let extrema = |line: Rational| {
            let mut values = polygon
                .0
                .iter()
                .filter(|point| point.y == line)
                .map(|point| &point.x);
            let first = values.next()?;
            let (mut low, mut high) = (first, first);
            for value in values {
                low = low.min(value);
                high = high.max(value);
            }
            Some((low, high))
        };
        // Convexity keeps the endpoint overlap inside the complete unit-height strip.
        let full_columns = extrema(integer(y))
            .zip(extrema(integer(y) + Rational::one()))
            .and_then(|((top_left, top_right), (bottom_left, bottom_right))| {
                let left = top_left
                    .max(bottom_left)
                    .ceil()
                    .max(Rational::zero())
                    .min(integer(u32::MAX))
                    .to_integer()
                    .to_u32()?;
                let right = top_right
                    .min(bottom_right)
                    .floor()
                    .max(Rational::zero())
                    .min(integer(u32::MAX))
                    .to_integer()
                    .to_u32()?;
                (left < right).then_some((left, right))
            });
        Self {
            polygon,
            full_columns,
            y,
        }
    }

    fn area(&self, x: u32, y: u32, budget: &mut Budget) -> Result<Rational, RasterError> {
        if y == self.y
            && self
                .full_columns
                .is_some_and(|(left, right)| x >= left && x < right)
        {
            return Ok(Rational::one());
        }
        self.polygon.area(x, y, budget)
    }
}

fn clip_axis(
    points: &[Point],
    axis: usize,
    bound: &Rational,
    keep_greater: bool,
    budget: &mut Budget,
) -> Result<Vec<Point>, RasterError> {
    let mut clipped = Vec::new();
    clipped
        .try_reserve(points.len() + 2)
        .map_err(RasterError::Allocation)?;
    if let Some(mut previous) = points.last() {
        for current in points {
            budget.spend(1)?;
            let before = if keep_greater {
                previous.coordinate(axis) >= bound
            } else {
                previous.coordinate(axis) <= bound
            };
            let after = if keep_greater {
                current.coordinate(axis) >= bound
            } else {
                current.coordinate(axis) <= bound
            };
            if before != after {
                let fraction = (previous.coordinate(axis) - bound)
                    / (previous.coordinate(axis) - current.coordinate(axis));
                let other = previous.coordinate(1 - axis)
                    + fraction * (current.coordinate(1 - axis) - previous.coordinate(1 - axis));
                clipped.push(if axis == 0 {
                    Point::new(bound.clone(), other)
                } else {
                    Point::new(other, bound.clone())
                });
            }
            if after {
                clipped.push(current.clone());
            }
            previous = current;
        }
    }
    Ok(clipped)
}

impl Polygon {
    fn clipped_row(&self, y: u32, budget: &mut Budget) -> Result<Option<PixelRow>, RasterError> {
        let top = integer(y);
        let bottom = &top + Rational::one();
        let points = clip_axis(&self.0, 1, &top, true, budget)?;
        let points = clip_axis(&points, 1, &bottom, false, budget)?;
        if points.len() < 3 {
            return Ok(None);
        }
        let mut positive_area = false;
        for pair in points[1..].windows(2) {
            if orientation(&points[0], &pair[0], &pair[1], budget)? != Ordering::Equal {
                positive_area = true;
                break;
            }
        }
        if !positive_area {
            return Ok(None);
        }
        Self::hull(points, budget).map(|polygon| Some(PixelRow::new(polygon, y)))
    }

    fn hull(mut points: Vec<Point>, budget: &mut Budget) -> Result<Self, RasterError> {
        points.sort_by(|a, b| a.x.cmp(&b.x).then(a.y.cmp(&b.y)));
        points.dedup();
        let mut hull = Vec::new();
        hull.try_reserve(points.len() * 2)
            .map_err(RasterError::Allocation)?;
        for point in &points {
            while hull.len() >= 2
                && !orientation(&hull[hull.len() - 2], &hull[hull.len() - 1], point, budget)?
                    .is_gt()
            {
                hull.pop();
            }
            hull.push(point.clone());
        }
        let lower = hull.len();
        for point in points.iter().rev().skip(1) {
            while hull.len() > lower
                && !orientation(&hull[hull.len() - 2], &hull[hull.len() - 1], point, budget)?
                    .is_gt()
            {
                hull.pop();
            }
            hull.push(point.clone());
        }
        hull.pop();
        if hull.len() < 3 {
            return Err(RasterError::FocusPrecision("degenerate contact hull"));
        }
        Ok(Self(hull))
    }

    fn halfplanes(planes: Vec<Plane>, budget: &mut Budget) -> Result<Self, RasterError> {
        let mut active: VecDeque<Plane> = VecDeque::new();
        active
            .try_reserve(planes.len())
            .map_err(RasterError::Allocation)?;
        for plane in planes {
            while active.len() > 1 {
                if !plane.excludes_intersection(
                    &active[active.len() - 2],
                    &active[active.len() - 1],
                    budget,
                )? {
                    break;
                }
                active.pop_back();
            }
            while active.len() > 1 {
                if !plane.excludes_intersection(&active[0], &active[1], budget)? {
                    break;
                }
                active.pop_front();
            }
            active.push_back(plane);
        }
        while active.len() > 2 {
            if !active[0].excludes_intersection(
                &active[active.len() - 2],
                &active[active.len() - 1],
                budget,
            )? {
                break;
            }
            active.pop_back();
        }
        while active.len() > 2 {
            if !active[active.len() - 1].excludes_intersection(&active[0], &active[1], budget)? {
                break;
            }
            active.pop_front();
        }
        if active.len() < 3 {
            return Err(RasterError::FocusPrecision(
                "unbounded support intersection",
            ));
        }
        let mut points = Vec::new();
        points
            .try_reserve(active.len())
            .map_err(RasterError::Allocation)?;
        for index in 0..active.len() {
            points.push(active[index].intersection(&active[(index + 1) % active.len()], budget)?);
        }
        Ok(Self(points))
    }

    fn separating_edge(
        &self,
        point: &Point,
        budget: &mut Budget,
    ) -> Result<Option<usize>, RasterError> {
        let points = &self.0;
        if orientation(&points[0], &points[1], point, budget)?.is_lt() {
            return Ok(Some(0));
        }
        if orientation(&points[0], points.last().unwrap(), point, budget)?.is_gt() {
            return Ok(Some(points.len() - 1));
        }
        let (mut low, mut high) = (1, points.len() - 1);
        while high - low > 1 {
            let middle = (low + high) / 2;
            if orientation(&points[0], &points[middle], point, budget)?.is_lt() {
                high = middle;
            } else {
                low = middle;
            }
        }
        Ok(orientation(&points[low], &points[high], point, budget)?
            .is_lt()
            .then_some(low))
    }

    fn area(&self, x: u32, y: u32, budget: &mut Budget) -> Result<Rational, RasterError> {
        let left = integer(x);
        let top = integer(y);
        let right = &left + Rational::one();
        let bottom = &top + Rational::one();
        let corners = [
            Point::new(left.clone(), top.clone()),
            Point::new(right.clone(), top.clone()),
            Point::new(right.clone(), bottom.clone()),
            Point::new(left.clone(), bottom.clone()),
        ];
        let mut contained = true;
        for corner in &corners {
            if let Some(edge) = self.separating_edge(corner, budget)? {
                contained = false;
                let a = &self.0[edge];
                let b = &self.0[(edge + 1) % self.0.len()];
                let mut disjoint = true;
                for corner in &corners {
                    if !orientation(a, b, corner, budget)?.is_lt() {
                        disjoint = false;
                        break;
                    }
                }
                if disjoint {
                    return Ok(Rational::zero());
                }
            }
        }
        if contained {
            return Ok(Rational::one());
        }
        let mut polygon = Vec::new();
        polygon
            .try_reserve(self.0.len() + 4)
            .map_err(RasterError::Allocation)?;
        polygon.extend(self.0.iter().cloned());
        for (axis, bound, keep_greater) in [
            (0, &left, true),
            (0, &right, false),
            (1, &top, true),
            (1, &bottom, false),
        ] {
            polygon = clip_axis(&polygon, axis, bound, keep_greater, budget)?;
        }
        let mut twice = Rational::zero();
        if let Some(mut previous) = polygon.last() {
            for current in &polygon {
                budget.spend(1)?;
                twice += (&previous.x - &left) * (&current.y - &top)
                    - (&previous.y - &top) * (&current.x - &left);
                previous = current;
            }
        }
        let area = twice.abs() / integer(2);
        if area > Rational::one() {
            return Err(RasterError::FocusPrecision("invalid clipped polygon area"));
        }
        Ok(area)
    }
}

fn guidance(a: &OutlineFocusSupport, b: &OutlineFocusSupport) -> Result<f64, RasterError> {
    let determinant = a.normal.x * b.normal.y - a.normal.y * b.normal.x;
    if determinant <= 0.0 || !determinant.is_finite() {
        return Err(RasterError::FocusPrecision("normal subdivision lost order"));
    }
    let mut error = 0.0_f64;
    for (first, second) in [(a.inner, b.inner), (a.outer, b.outer)] {
        let x = (first.support * b.normal.y - a.normal.y * second.support) / determinant;
        let y = (a.normal.x * second.support - first.support * b.normal.x) / determinant;
        let dx = second.point.x - first.point.x;
        let dy = second.point.y - first.point.y;
        let length = dx * dx + dy * dy;
        let parameter = if length > 0.0 {
            ((x - first.point.x) * dx + (y - first.point.y) * dy) / length
        } else {
            0.0
        };
        let parameter = parameter.clamp(0.0, 1.0);
        let dx = x - first.point.x - parameter * dx;
        let dy = y - first.point.y - parameter * dy;
        let distance = (dx * dx + dy * dy).sqrt();
        if !distance.is_finite() {
            return Err(RasterError::FocusPrecision(
                "nonfinite subdivision guidance",
            ));
        }
        error = error.max(distance);
    }
    Ok(error)
}

fn samples(
    geometry: &OutlineFocusGeometry,
    scale: f64,
    threshold: f64,
    budget: &mut Budget,
) -> Result<Vec<OutlineFocusSupport>, RasterError> {
    let mut queries = Vec::new();
    queries
        .try_reserve(MAX_VERTICES)
        .map_err(RasterError::Allocation)?;
    for (x, y) in [
        (1.0, 0.0),
        (1.0, 1.0),
        (0.0, 1.0),
        (-1.0, 1.0),
        (-1.0, 0.0),
        (-1.0, -1.0),
        (0.0, -1.0),
        (1.0, -1.0),
    ] {
        queries.push(
            geometry
                .support_at(PhysicalVector { x, y })
                .map_err(RasterError::OutlineFocus)?,
        );
    }
    for _ in 0..20 {
        let mut refined = Vec::new();
        refined
            .try_reserve(MAX_VERTICES)
            .map_err(RasterError::Allocation)?;
        let mut changed = false;
        for index in 0..queries.len() {
            budget.spend(1)?;
            let a = &queries[index];
            let b = &queries[(index + 1) % queries.len()];
            if refined.len() == MAX_VERTICES {
                return Err(RasterError::ResourceLimit);
            }
            refined.push(a.clone());
            if guidance(a, b)? * scale > threshold {
                if refined.len() == MAX_VERTICES {
                    return Err(RasterError::ResourceLimit);
                }
                changed = true;
                let direction = PhysicalVector {
                    x: a.normal.x + b.normal.x,
                    y: a.normal.y + b.normal.y,
                };
                refined.push(
                    geometry
                        .support_at(direction)
                        .map_err(RasterError::OutlineFocus)?,
                );
            }
        }
        queries = refined;
        if !changed {
            return Ok(queries);
        }
    }
    Err(RasterError::FocusPrecision(
        "normal subdivision did not converge",
    ))
}

struct Boundary {
    contacts: Polygon,
    containing: Polygon,
}
impl Boundary {
    fn new(
        queries: &[OutlineFocusSupport],
        outer: bool,
        magnitude: &Rational,
        viewport: Viewport,
        budget: &mut Budget,
    ) -> Result<Self, RasterError> {
        let tolerance = Rational::new(1.into(), 1_000_000_000_000_u64.into());
        let scale = rational(viewport.pixels_per_unit);
        let mut points = Vec::new();
        let mut planes = Vec::new();
        points
            .try_reserve(queries.len())
            .map_err(RasterError::Allocation)?;
        planes
            .try_reserve(queries.len())
            .map_err(RasterError::Allocation)?;
        for query in queries {
            let boundary = if outer { query.outer } else { query.inner };
            points.push(Point::device(boundary.point, viewport));
            let x = rational(query.normal.x);
            let y = rational(query.normal.y);
            let support = rational(boundary.support);
            let margin = integer(2) * &tolerance * support.abs().max(Rational::one())
                + integer(4) * &tolerance * magnitude;
            let support = (support + margin
                - &x * rational(viewport.origin.x)
                - &y * rational(viewport.origin.y))
                * &scale;
            planes.push(Plane { x, y, support });
        }
        Ok(Self {
            contacts: Polygon::hull(points, budget)?,
            containing: Polygon::halfplanes(planes, budget)?,
        })
    }
}

/// Bounds analytic ring coverage before rounding each pixel to RGBA8.
pub fn render_outline_focus(
    ir: &OutlineFocusIr,
    viewport: Viewport,
) -> Result<RasterImage, RasterError> {
    let bytes = validate(viewport, 1)?;
    let geometry = ir.geometry();
    let size = geometry.outline().size();
    let offset = geometry.side_offset();
    let magnitude = (rational(size.width)
        + rational(offset.x).abs()
        + rational(geometry.outer_outset()))
    .max(rational(size.height) + rational(offset.y).abs() + rational(geometry.outer_outset()))
    .max(Rational::one());
    let epsilon = integer(4)
        * Rational::new(1.into(), 1_000_000_000_000_u64.into())
        * &magnitude
        * rational(viewport.pixels_per_unit);
    let allowance = integer(4) * &epsilon;
    let tolerance = Rational::new(1.into(), 255.into());
    if &allowance * integer(2) > tolerance.clone() / integer(4) {
        return Err(RasterError::FocusPrecision(
            "support uncertainty exceeds the pixel budget",
        ));
    }
    let rgb = ir
        .indicator()
        .color()
        .components()
        .map(|channel| (channel * 255.0).round() as u8);
    let mut budget = Budget(MAX_WORK);
    for attempt in 0..4 {
        let queries = samples(
            geometry,
            viewport.pixels_per_unit,
            1.0 / (256.0 * f64::from(1_u32 << attempt)),
            &mut budget,
        )?;
        let inner = Boundary::new(&queries, false, &magnitude, viewport, &mut budget)?;
        let outer = Boundary::new(&queries, true, &magnitude, viewport, &mut budget)?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(bytes)
            .map_err(RasterError::Allocation)?;
        rgba.resize(bytes, 0);
        let mut complete = true;
        'pixels: for y in 0..viewport.height {
            let Some(outer_containing_row) = outer.containing.clipped_row(y, &mut budget)? else {
                continue;
            };
            let inner_contacts_row = inner.contacts.clipped_row(y, &mut budget)?;
            let outer_contacts_row = outer.contacts.clipped_row(y, &mut budget)?;
            let inner_containing_row = inner.containing.clipped_row(y, &mut budget)?;
            for x in 0..viewport.width {
                let inner_contacts = match &inner_contacts_row {
                    Some(row) => row.area(x, y, &mut budget)?,
                    None => Rational::zero(),
                };
                // For convex P within A + D(e), P eroded by e is within A.
                // Eroding P intersect a unit pixel loses at most 4e area.
                if inner_contacts.is_one() {
                    continue;
                }
                let outer_containing = outer_containing_row.area(x, y, &mut budget)?;
                if outer_containing.is_zero() {
                    continue;
                }
                let outer_contacts = match &outer_contacts_row {
                    Some(row) => row.area(x, y, &mut budget)?,
                    None => Rational::zero(),
                };
                let inner_containing = match &inner_containing_row {
                    Some(row) => row.area(x, y, &mut budget)?,
                    None => Rational::zero(),
                };
                let low = (outer_contacts - &allowance - inner_containing).max(Rational::zero());
                let high = (outer_containing - inner_contacts + &allowance).min(Rational::one());
                if high < low {
                    return Err(RasterError::FocusPrecision(
                        "inverted pixel coverage bounds",
                    ));
                }
                if &high - &low > tolerance {
                    complete = false;
                    break 'pixels;
                }
                let scaled = (low + high) * integer(255) / integer(2);
                let rounded = (scaled.numer() * 2_u8 + scaled.denom()) / (scaled.denom() * 2_u8);
                let alpha = rounded
                    .to_u8()
                    .ok_or(RasterError::FocusPrecision("invalid quantized coverage"))?;
                if alpha != 0 {
                    let index = ((y * viewport.width + x) * 4) as usize;
                    rgba[index..index + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], alpha]);
                }
            }
        }
        if complete {
            return Ok(RasterImage {
                width: viewport.width,
                height: viewport.height,
                rgba,
            });
        }
    }
    Err(RasterError::FocusPrecision(
        "pixel coverage did not converge",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_conversion_retains_exact_ieee_normal_subnormal_and_signed_values() {
        assert_eq!(
            rational(0.1),
            Rational::new(
                3_602_879_701_896_397_u64.into(),
                36_028_797_018_963_968_u64.into()
            )
        );
        assert_eq!(rational(-0.1), -rational(0.1));
        assert_eq!(rational(-0.0), Rational::zero());
        assert_eq!(
            rational(f64::from_bits(1)),
            Rational::new(BigInt::one(), BigInt::one() << 1074_usize)
        );
        assert_eq!(
            rational(f64::MIN_POSITIVE),
            Rational::new(BigInt::one(), BigInt::one() << 1022_usize)
        );
        assert_eq!(
            rational(f64::MAX),
            Rational::from_integer(((BigInt::one() << 53_usize) - BigInt::one()) << 971_usize)
        );
    }

    #[test]
    fn float_enclosures_contain_exact_values_including_underflow() {
        let values = [
            rational(0.0),
            rational(-f64::MAX),
            rational(f64::from_bits(1)),
            Rational::new(1.into(), 3.into()),
            Rational::new((-1).into(), 3.into()),
            rational(f64::from_bits(1)) / integer(2),
        ];
        for value in values {
            let interval = Interval::enclose(&value).unwrap();
            assert!(rational(interval.low) <= value);
            assert!(rational(interval.high) >= value);
        }
        assert!(Interval::enclose(&(rational(f64::MAX) * integer(2))).is_none());
    }

    #[test]
    fn filtered_orientation_matches_exact_sign_for_cancellation_and_overflow() {
        let tiny = rational(f64::from_bits(1)) / integer(2);
        let triples = [
            [
                (rational(0.0), rational(0.0)),
                (rational(1.0), rational(1.0)),
                (rational(2.0), rational(2.0) + &tiny),
            ],
            [
                (rational(0.0), rational(0.0)),
                (rational(1.0), rational(1.0)),
                (rational(2.0), rational(2.0) - &tiny),
            ],
            [
                (rational(-f64::MAX), rational(0.0)),
                (rational(f64::MAX), rational(1.0)),
                (rational(0.0), rational(0.0)),
            ],
            [
                (rational(0.0), rational(0.0)),
                (rational(1.0), rational(1.0)),
                (rational(2.0), rational(2.0)),
            ],
        ];
        for coordinates in triples {
            let [a, b, p] = coordinates.map(|(x, y)| Point::new(x, y));
            let expected = ((&b.x - &a.x) * (&p.y - &a.y) - (&b.y - &a.y) * (&p.x - &a.x))
                .cmp(&Rational::zero());
            assert_eq!(orientation(&a, &b, &p, &mut Budget(1)).unwrap(), expected);
        }
    }

    #[test]
    fn homogeneous_plane_predicate_matches_exact_intersections_for_both_orders() {
        let a = Plane {
            x: integer(1),
            y: integer(0),
            support: integer(2),
        };
        let b = Plane {
            x: integer(0),
            y: integer(1),
            support: integer(3),
        };
        for (first, second) in [(&a, &b), (&b, &a)] {
            let point = first.intersection(second, &mut Budget(1)).unwrap();
            for support in [4, 5, 6] {
                let plane = Plane {
                    x: integer(1),
                    y: integer(1),
                    support: integer(support),
                };
                let expected = &plane.x * &point.x + &plane.y * &point.y > plane.support;
                assert_eq!(
                    plane
                        .excludes_intersection(first, second, &mut Budget(1))
                        .unwrap(),
                    expected
                );
            }
        }
    }

    #[test]
    fn clipped_convex_area_is_exact_at_fractional_pixel_boundaries() {
        let points = [(0.25, 0.25), (1.75, 0.25), (1.75, 1.75), (0.25, 1.75)]
            .map(|(x, y)| Point::new(rational(x), rational(y)))
            .to_vec();
        let polygon = Polygon::hull(points, &mut Budget(100)).unwrap();
        for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            assert_eq!(
                polygon.area(x, y, &mut Budget(100)).unwrap(),
                Rational::new(9.into(), 16.into())
            );
        }
        assert!(polygon.area(2, 0, &mut Budget(100)).unwrap().is_zero());
    }

    #[test]
    fn clipped_rows_preserve_exact_pixel_areas_for_fractional_and_degenerate_intersections() {
        for coordinates in [
            vec![(0.25, 0.25), (7.75, 0.25), (7.75, 7.75), (0.25, 7.75)],
            vec![(0.0, 0.0), (8.0, 0.0), (0.0, 8.0)],
            vec![(-2.0, -1.0), (4.0, 0.0), (9.0, 6.0), (1.0, 8.0)],
            vec![(0.0, 1.0), (8.0, 1.0), (8.0, 1.125), (0.0, 1.125)],
            vec![(0.0, -2.0), (8.0, -2.0), (8.0, 0.0), (0.0, 0.0)],
        ] {
            let polygon = Polygon::hull(
                coordinates
                    .into_iter()
                    .map(|(x, y)| Point::new(rational(x), rational(y)))
                    .collect(),
                &mut Budget(10000),
            )
            .unwrap();
            for y in 0..10 {
                let row = polygon.clipped_row(y, &mut Budget(10000)).unwrap();
                for x in 0..10 {
                    let actual = match &row {
                        Some(row) => row.area(x, y, &mut Budget(10000)).unwrap(),
                        None => Rational::zero(),
                    };
                    assert_eq!(
                        actual,
                        polygon.area(x, y, &mut Budget(10000)).unwrap(),
                        "pixel ({x},{y})"
                    );
                }
            }
        }
    }

    #[test]
    fn clipped_rows_keep_the_work_budget_and_closed_integer_boundaries() {
        let polygon = Polygon::hull(
            [(0.0, 0.0), (8.0, 0.0), (8.0, 8.0), (0.0, 8.0)]
                .map(|(x, y)| Point::new(rational(x), rational(y)))
                .to_vec(),
            &mut Budget(100),
        )
        .unwrap();
        assert!(matches!(
            polygon.clipped_row(0, &mut Budget(0)),
            Err(RasterError::ResourceLimit)
        ));
        for y in 0..8 {
            let row = polygon.clipped_row(y, &mut Budget(1000)).unwrap().unwrap();
            for x in 0..8 {
                assert!(row.area(x, y, &mut Budget(1000)).unwrap().is_one());
                assert!(row.area(x, y + 1, &mut Budget(1000)).unwrap().is_zero());
            }
        }
        assert!(polygon.clipped_row(8, &mut Budget(1000)).unwrap().is_none());
    }

    #[test]
    fn cyclic_halfplanes_remove_redundancy_and_retain_every_exact_constraint() {
        for diagonal in [integer(3), Rational::new(3.into(), 2.into())] {
            let planes: Vec<_> = [
                (1, 0),
                (1, 1),
                (0, 1),
                (-1, 1),
                (-1, 0),
                (-1, -1),
                (0, -1),
                (1, -1),
            ]
            .into_iter()
            .map(|(x, y)| Plane {
                x: Rational::from_integer(x.into()),
                y: Rational::from_integer(y.into()),
                support: if x == 0 || y == 0 {
                    Rational::one()
                } else {
                    diagonal.clone()
                },
            })
            .collect();
            let polygon = Polygon::halfplanes(
                planes
                    .iter()
                    .map(|plane| Plane {
                        x: plane.x.clone(),
                        y: plane.y.clone(),
                        support: plane.support.clone(),
                    })
                    .collect(),
                &mut Budget(100),
            )
            .unwrap();
            assert_eq!(polygon.0.len(), if diagonal == integer(3) { 4 } else { 8 });
            for point in &polygon.0 {
                for plane in &planes {
                    assert!(&plane.x * &point.x + &plane.y * &point.y <= plane.support);
                }
            }
            assert_eq!(
                polygon.area(0, 0, &mut Budget(100)).unwrap(),
                if diagonal == integer(3) {
                    Rational::one()
                } else {
                    Rational::new(7.into(), 8.into())
                }
            );
        }
    }

    #[test]
    fn exhausted_work_budget_fails_before_the_next_geometry_operation() {
        let point = Point::new(Rational::zero(), Rational::zero());
        assert!(matches!(
            orientation(&point, &point, &point, &mut Budget(0)),
            Err(RasterError::ResourceLimit)
        ));
    }
}
