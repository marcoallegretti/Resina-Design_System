use super::*;
use resina_model::ContourSegment;
use resina_resolver::{ExtrudedContourResult, resolve_extruded_contour_source};
use serde_json::json;

fn contour(
    radius: [f64; 2],
    size: [f64; 2],
    offset: [f64; 2],
    direction: &str,
) -> ExtrudedContourResult {
    let corner = json!({"x": radius[0], "y": radius[1]});
    resolve_extruded_contour_source(
        &json!({
            "schemaVersion": "0.1.0",
            "size": {"width": size[0], "height": size[1]},
            "radii": {"topStart": corner, "topEnd": corner, "bottomEnd": corner, "bottomStart": corner},
            "layoutDirection": direction,
            "offset": {"x": offset[0], "y": offset[1]},
        })
        .to_string(),
    )
    .unwrap()
}

fn point(x: f64, y: f64) -> SampleBox {
    let point = PhysicalVector { x, y };
    SampleBox {
        min: point,
        max: point,
    }
}

fn tolerance(region: SampleBox, contour: &ExtrudedContourResult) -> f64 {
    let bounds = contour.bounds().unwrap();
    let region = [region.min.x, region.min.y, region.max.x, region.max.y]
        .into_iter()
        .map(f64::abs)
        .fold(0.0, f64::max);
    64.0 * f64::EPSILON
        * region
            .max(bounds.x.abs())
            .max(bounds.y.abs())
            .max(bounds.width)
            .max(bounds.height)
}

/// The smallest `step` in `[low, high]` for which the monotone predicate holds.
fn first_true(low: f64, high: f64, holds: impl Fn(f64) -> bool) -> f64 {
    assert!(!holds(low) && holds(high));
    let (mut low, mut high) = (low.to_bits(), high.to_bits());
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if holds(f64::from_bits(middle)) {
            high = middle;
        } else {
            low = middle;
        }
    }
    f64::from_bits(high)
}

fn ulps(a: f64, b: f64) -> u64 {
    a.to_bits().abs_diff(b.to_bits())
}

#[test]
fn line_certification_flips_at_the_tolerance_margin() {
    let contour = contour([10.0, 10.0], [100.0, 60.0], [16.0, 16.0], "ltr");
    let (from, to) = contour
        .segments()
        .iter()
        .find_map(|segment| match *segment {
            ContourSegment::Line { from, to }
                if (to.y - from.y).abs() == (to.x - from.x).abs() && from.x != to.x =>
            {
                Some((from, to))
            }
            _ => None,
        })
        .expect("an extruded contour has a diagonal line");
    let normal = PhysicalVector {
        x: to.y - from.y,
        y: from.x - to.x,
    };
    let middle = PhysicalVector {
        x: (from.x + to.x) / 2.0,
        y: (from.y + to.y) / 2.0,
    };
    let toward = normal.x.signum();
    let advance = |x: f64| {
        if toward > 0.0 {
            x.next_up()
        } else {
            x.next_down()
        }
    };
    let retreat = |x: f64| {
        if toward > 0.0 {
            x.next_down()
        } else {
            x.next_up()
        }
    };
    let analytic = tolerance(point(middle.x, middle.y), &contour)
        * (normal.x.abs() + normal.y.abs())
        / normal.x.abs();
    let mut x = middle.x;
    let mut moved = 0_u64;
    while !uniform::outside(point(x, middle.y), &contour) {
        x = advance(x);
        moved += 1;
        assert!(moved < 1 << 20, "no certification within a million ulps");
    }
    let expected = ulps(middle.x, middle.x + toward * analytic);
    assert!(
        moved.abs_diff(expected) <= 4,
        "certified after {moved} ulps, tolerance is {expected} ulps"
    );
    assert!(!uniform::outside(point(retreat(x), middle.y), &contour));
    assert!(!uniform::outside(
        point(middle.x - toward * 3.0, middle.y),
        &contour
    ));

    let toward_y = normal.y.signum();
    let nearest = |x: f64| SampleBox {
        min: PhysicalVector {
            x: if toward > 0.0 { x } else { x - 5.0 },
            y: if toward_y > 0.0 {
                middle.y
            } else {
                middle.y - 5.0
            },
        },
        max: PhysicalVector {
            x: if toward > 0.0 { x + 5.0 } else { x },
            y: if toward_y > 0.0 {
                middle.y + 5.0
            } else {
                middle.y
            },
        },
    };
    assert!(uniform::outside(nearest(x), &contour));
    assert!(!uniform::outside(nearest(retreat(x)), &contour));
}

#[test]
fn arc_certification_flips_at_the_tolerance_margin() {
    let contour = contour([10.0, 10.0], [100.0, 60.0], [0.0, 0.0], "ltr");
    let center = contour
        .segments()
        .iter()
        .find_map(|segment| match *segment {
            ContourSegment::Arc { center, .. } if center.x == 10.0 && center.y == 10.0 => {
                Some(center)
            }
            _ => None,
        })
        .unwrap();
    let diagonal = |step: f64| point(center.x - step, center.y - step);
    let analytic = (10.0 + tolerance(diagonal(0.0), &contour)) / 2.0_f64.sqrt();
    let flip = first_true(0.0, 10.0, |step| uniform::outside(diagonal(step), &contour));
    assert!(
        ulps(flip, analytic) <= 4,
        "flip at {flip}, circle margin {analytic}"
    );
    assert!(!uniform::outside(diagonal(flip.next_down()), &contour));
    assert!(uniform::outside(diagonal(flip), &contour));
    let circle = 10.0 / 2.0_f64.sqrt();
    for step in [circle.next_down(), circle, circle.next_up(), circle - 1e-3] {
        assert!(!uniform::outside(diagonal(step), &contour), "{step}");
    }
    let wide = SampleBox {
        min: PhysicalVector {
            x: center.x - flip,
            y: center.y - flip,
        },
        max: PhysicalVector {
            x: center.x - flip + 4.0,
            y: center.y - flip + 4.0,
        },
    };
    assert!(
        !uniform::outside(wide, &contour),
        "a box reaching the circle is not outside"
    );
    let outer = SampleBox {
        min: PhysicalVector {
            x: center.x - flip - 1.0,
            y: center.y - flip - 1.0,
        },
        max: PhysicalVector {
            x: center.x - flip,
            y: center.y - flip,
        },
    };
    assert!(uniform::outside(outer, &contour));
}

#[test]
fn sector_edges_never_certify_points_on_or_inside_the_contour() {
    let mut certified = 0;
    for (offset, direction) in [
        ([0.0, 0.0], "ltr"),
        ([16.0, 16.0], "ltr"),
        ([-16.0, 8.0], "rtl"),
        ([24.0, -12.0], "ltr"),
    ] {
        let contour = contour([10.0, 10.0], [100.0, 60.0], offset, direction);
        for segment in contour.segments() {
            let ContourSegment::Arc {
                center,
                radii,
                start,
                end,
            } = *segment
            else {
                continue;
            };
            for radial in [start, end] {
                for (distance, may_certify) in [
                    (-1e-3, false),
                    (-1e-9, false),
                    (0.0, false),
                    (1e-13, false),
                    (1e-3, true),
                ] {
                    let at = point(
                        center.x + (radii.x + distance) * radial.x,
                        center.y + (radii.x + distance) * radial.y,
                    );
                    let outside = uniform::outside(at, &contour);
                    assert!(
                        may_certify || !outside,
                        "{offset:?} {direction} {radial:?} {distance}"
                    );
                    certified += usize::from(outside);
                }
            }
        }
    }
    assert!(
        certified > 0,
        "adjoining line segments certify clearly outside sector edges"
    );
}

#[test]
fn certified_boxes_contain_only_points_outside_the_swept_contour() {
    let size = [100.0, 60.0];
    let radius = 10.0;
    let distance = |x: f64, y: f64| {
        let dx = (x - 50.0).abs() - (size[0] / 2.0 - radius);
        let dy = (y - 30.0).abs() - (size[1] / 2.0 - radius);
        dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) - radius
    };
    for offset in [[0.0, 0.0], [16.0, 16.0], [-16.0, 8.0], [24.0, -12.0]] {
        let contour = contour([radius, radius], size, offset, "ltr");
        let swept_inside = |x: f64, y: f64| {
            (0..=512).any(|index| {
                let along = f64::from(index) / 512.0;
                distance(x - offset[0] * along, y - offset[1] * along) <= 0.0
            })
        };
        let mut certified = 0;
        for column in -8..=136 {
            for row in -8..=88 {
                let (x, y) = (f64::from(column), f64::from(row));
                for extent in [0.0, 0.25, 3.0] {
                    let region = SampleBox {
                        min: PhysicalVector { x, y },
                        max: PhysicalVector {
                            x: x + extent,
                            y: y + extent,
                        },
                    };
                    if !uniform::outside(region, &contour) {
                        continue;
                    }
                    certified += 1;
                    for (vx, vy) in [
                        (region.min.x, region.min.y),
                        (region.max.x, region.min.y),
                        (region.max.x, region.max.y),
                        (region.min.x, region.max.y),
                    ] {
                        assert!(
                            !swept_inside(vx, vy),
                            "{offset:?}: certified vertex ({vx}, {vy}) lies in the contour"
                        );
                    }
                }
            }
        }
        assert!(certified > 1000, "{offset:?}: {certified}");
    }
}

#[test]
fn unsupported_contours_are_never_certified_by_the_caller() {
    let elliptical = contour([12.0, 8.0], [100.0, 60.0], [0.0, 0.0], "ltr");
    assert!(!uniform::supported(&elliptical));
    let empty = contour([10.0, 10.0], [0.0, 60.0], [0.0, 0.0], "ltr");
    assert!(empty.bounds().is_none() && !uniform::supported(&empty));
    assert!(!uniform::outside(point(1e9, 1e9), &empty));
    let tiny = contour([0.0, 0.0], [1e-101, 1e-101], [0.0, 0.0], "ltr");
    assert!(!uniform::supported(&tiny));
    assert!(!uniform::outside(point(0.0, 0.0), &tiny));
    let regular = contour([10.0, 10.0], [100.0, 60.0], [0.0, 0.0], "ltr");
    assert!(uniform::supported(&regular));
    assert!(!uniform::outside(point(1e101, 1e101), &regular));
}
