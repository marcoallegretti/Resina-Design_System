use resina_environment::LayoutDirection;
use resina_model::{CornerRadius, LogicalCornerRadii, PhysicalVector, SurfaceSize};
use resina_resolver::{OutlineCurvature, resolve_surface_outline};

#[test]
fn authored_public_vectors_and_diagnostic_failures() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/surface-outline-vectors.json"
    ))
    .unwrap();
    fn compare(actual: &serde_json::Value, expected: &serde_json::Value) {
        match (actual, expected) {
            (serde_json::Value::Object(a), serde_json::Value::Object(e)) => {
                assert_eq!(a.len(), e.len());
                for (key, value) in e {
                    compare(&a[key], value);
                }
            }
            (serde_json::Value::Number(a), serde_json::Value::Number(e)) => {
                assert!((a.as_f64().unwrap() - e.as_f64().unwrap()).abs() <= 1e-12);
            }
            _ => assert_eq!(actual, expected),
        }
    }
    for case in cases {
        let result = resina_resolver::resolve_surface_outline_source(&case["request"].to_string());
        if let Some(expected) = case.get("expected") {
            compare(&serde_json::to_value(result.unwrap()).unwrap(), expected);
        } else {
            let message = result.unwrap_err().to_string();
            assert!(
                message.contains(case["errorContains"].as_str().unwrap()),
                "{}: {message}",
                case["name"]
            );
        }
    }
}

fn corners(x: f64, y: f64) -> LogicalCornerRadii {
    let radius = CornerRadius { x, y };
    LogicalCornerRadii {
        top_start: radius,
        top_end: radius,
        bottom_end: radius,
        bottom_start: radius,
    }
}

#[test]
fn quartic_boundary_and_safe_content_have_independent_implicit_proof() {
    let size = SurfaceSize {
        width: 120.0,
        height: 60.0,
    };
    let outline = resolve_surface_outline(
        size,
        corners(20.0, 12.0),
        LayoutDirection::Ltr,
        OutlineCurvature::Quartic,
        2.0,
    )
    .unwrap();
    let content = outline.content_bounds();
    for iy in 0..=40 {
        for ix in 0..=80 {
            let p = PhysicalVector {
                x: content.x + content.width * f64::from(ix) / 80.0,
                y: content.y + content.height * f64::from(iy) / 40.0,
            };
            assert!(outline.contains(p).unwrap(), "{p:?}");
        }
    }
    let diagonal = 1.0 - 2.0_f64.powf(-0.25);
    let boundary = PhysicalVector {
        x: 20.0 * diagonal,
        y: 12.0 * diagonal,
    };
    let u = (20.0 - boundary.x) / 20.0;
    let v = (12.0 - boundary.y) / 12.0;
    assert!((u.powi(4) + v.powi(4) - 1.0).abs() < 1e-14);
    assert!(
        !outline
            .contains(PhysicalVector {
                x: boundary.x - 0.01,
                y: boundary.y - 0.01
            })
            .unwrap()
    );
    assert!(
        outline
            .contains(PhysicalVector {
                x: boundary.x + 0.01,
                y: boundary.y + 0.01
            })
            .unwrap()
    );
}

#[test]
fn circular_fallback_is_contained_in_quartic_outline() {
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        let radii = LogicalCornerRadii {
            top_start: CornerRadius { x: 22.0, y: 14.0 },
            top_end: CornerRadius { x: 7.0, y: 20.0 },
            bottom_end: CornerRadius { x: 15.0, y: 8.0 },
            bottom_start: CornerRadius { x: 5.0, y: 12.0 },
        };
        let size = SurfaceSize {
            width: 60.0,
            height: 50.0,
        };
        let circular =
            resolve_surface_outline(size, radii, direction, OutlineCurvature::Circular, 1.0)
                .unwrap();
        let quartic =
            resolve_surface_outline(size, radii, direction, OutlineCurvature::Quartic, 1.0)
                .unwrap();
        for iy in 0..=200 {
            for ix in 0..=240 {
                let p = PhysicalVector {
                    x: f64::from(ix) / 4.0,
                    y: f64::from(iy) / 4.0,
                };
                if circular.contains(p).unwrap() {
                    assert!(quartic.contains(p).unwrap(), "{direction:?}: {p:?}");
                }
            }
        }
    }
}

#[test]
fn logical_corners_mirror_but_coordinates_remain_physical() {
    let size = SurfaceSize {
        width: 60.0,
        height: 50.0,
    };
    let radii = LogicalCornerRadii {
        top_start: CornerRadius { x: 22.0, y: 14.0 },
        top_end: CornerRadius { x: 7.0, y: 20.0 },
        bottom_end: CornerRadius { x: 15.0, y: 8.0 },
        bottom_start: CornerRadius { x: 5.0, y: 12.0 },
    };
    let ltr = resolve_surface_outline(
        size,
        radii,
        LayoutDirection::Ltr,
        OutlineCurvature::Quartic,
        1.0,
    )
    .unwrap();
    let rtl = resolve_surface_outline(
        size,
        radii,
        LayoutDirection::Rtl,
        OutlineCurvature::Quartic,
        1.0,
    )
    .unwrap();
    for iy in 0..200 {
        for ix in 0..240 {
            let p = PhysicalVector {
                x: (f64::from(ix) + 0.5) / 4.0,
                y: (f64::from(iy) + 0.5) / 4.0,
            };
            assert_eq!(
                ltr.contains(p).unwrap(),
                rtl.contains(PhysicalVector {
                    x: 60.0 - p.x,
                    y: p.y
                })
                .unwrap()
            );
        }
    }
}

#[test]
fn invalid_and_unrepresentable_geometry_fails_without_collapsed_content() {
    for width in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            resolve_surface_outline(
                SurfaceSize {
                    width,
                    height: 10.0
                },
                corners(2.0, 2.0),
                LayoutDirection::Ltr,
                OutlineCurvature::Quartic,
                1.0
            )
            .is_err()
        );
    }
    for inset in [-1.0, 5.0, f64::NAN, f64::INFINITY] {
        assert!(
            resolve_surface_outline(
                SurfaceSize {
                    width: 20.0,
                    height: 10.0
                },
                corners(2.0, 2.0),
                LayoutDirection::Ltr,
                OutlineCurvature::Quartic,
                inset
            )
            .is_err()
        );
    }
}

#[test]
fn content_reservation_remains_contained_across_numeric_scales_and_overlap() {
    for exponent in [-300, -100, -10, 0, 10, 100, 300] {
        let scale = 10.0_f64.powi(exponent);
        for (width_factor, height_factor) in [(1.0, 1.0), (3.0, 1.0), (1.0, 3.0)] {
            let size = SurfaceSize {
                width: width_factor * scale,
                height: height_factor * scale,
            };
            for fraction in [0.1, 0.4, 0.8] {
                for curvature in [OutlineCurvature::Circular, OutlineCurvature::Quartic] {
                    let outline = resolve_surface_outline(
                        size,
                        corners(size.width * fraction, size.height * fraction),
                        LayoutDirection::Ltr,
                        curvature,
                        scale * 0.01,
                    )
                    .unwrap();
                    let bounds = outline.content_bounds();
                    for iy in 0..=20 {
                        for ix in 0..=20 {
                            let p = PhysicalVector {
                                x: bounds.x + bounds.width * (f64::from(ix) / 20.0),
                                y: bounds.y + bounds.height * (f64::from(iy) / 20.0),
                            };
                            assert!(
                                outline.contains(p).unwrap(),
                                "{exponent} {fraction} {curvature:?} {p:?}"
                            );
                        }
                    }
                    assert!(
                        outline
                            .contains(PhysicalVector {
                                x: f64::NAN,
                                y: 0.0
                            })
                            .is_err()
                    );
                    assert!(
                        outline
                            .contains(PhysicalVector {
                                x: 0.0,
                                y: f64::INFINITY
                            })
                            .is_err()
                    );
                }
            }
        }
    }
}
