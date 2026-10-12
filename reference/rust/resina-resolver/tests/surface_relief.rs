use resina_environment::LayoutDirection;
use resina_model::{CornerRadius, LogicalCornerRadii, PhysicalVector, SurfaceSize};
use resina_resolver::{
    OutlineCurvature, ReliefKind, SurfaceReliefProfile, resolve_surface_outline,
    resolve_surface_relief,
};

#[test]
fn authored_relief_vectors_match_independent_decimal_expectations() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/surface-relief-vectors.json"
    ))
    .unwrap();
    fn compare(a: &serde_json::Value, e: &serde_json::Value) {
        match (a, e) {
            (serde_json::Value::Object(a), serde_json::Value::Object(e)) => {
                assert_eq!(a.len(), e.len());
                for (k, v) in e {
                    compare(&a[k], v);
                }
            }
            (serde_json::Value::Array(a), serde_json::Value::Array(e)) => {
                assert_eq!(a.len(), e.len());
                for (a, e) in a.iter().zip(e) {
                    compare(a, e);
                }
            }
            (serde_json::Value::Number(a), serde_json::Value::Number(e)) => {
                let (a, e) = (a.as_f64().unwrap(), e.as_f64().unwrap());
                assert!(
                    (a - e).abs() <= 1e-12_f64.max(e.abs() * 1e-12),
                    "{a} != {e}"
                );
            }
            _ => assert_eq!(a, e),
        }
    }
    for case in cases {
        let result = resina_resolver::resolve_surface_relief_source(&case["request"].to_string());
        if let Some(expected) = case.get("expected") {
            compare(&serde_json::to_value(result.unwrap()).unwrap(), expected);
        } else {
            let diagnostic = result.unwrap_err().to_string();
            assert!(
                diagnostic.contains(case["errorContains"].as_str().unwrap()),
                "{}: {diagnostic}",
                case["name"]
            );
        }
    }
}

fn relief(
    kind: ReliefKind,
    curvature: OutlineCurvature,
    scale: f64,
) -> resina_resolver::SurfaceRelief {
    let radius = CornerRadius {
        x: 20.0 * scale,
        y: 12.0 * scale,
    };
    let outline = resolve_surface_outline(
        SurfaceSize {
            width: 120.0 * scale,
            height: 60.0 * scale,
        },
        LogicalCornerRadii {
            top_start: radius,
            top_end: radius,
            bottom_end: radius,
            bottom_start: radius,
        },
        LayoutDirection::Ltr,
        curvature,
        2.0 * scale,
    )
    .unwrap();
    resolve_surface_relief(
        outline,
        SurfaceReliefProfile {
            kind,
            body_thickness: 4.0 * scale,
            amplitude: if kind == ReliefKind::Flat {
                0.0
            } else {
                0.5 * scale
            },
            shoulder_width: if kind == ReliefKind::Flat {
                0.0
            } else {
                1.0 * scale
            },
        },
    )
    .unwrap()
}

#[test]
fn square_shoulder_has_independent_height_and_normal_expectations() {
    let radius = CornerRadius { x: 0.0, y: 0.0 };
    let outline = resolve_surface_outline(
        SurfaceSize {
            width: 20.0,
            height: 10.0,
        },
        LogicalCornerRadii {
            top_start: radius,
            top_end: radius,
            bottom_end: radius,
            bottom_start: radius,
        },
        LayoutDirection::Ltr,
        OutlineCurvature::Quartic,
        0.0,
    )
    .unwrap();
    let face = resolve_surface_relief(
        outline,
        SurfaceReliefProfile {
            kind: ReliefKind::Convex,
            body_thickness: 4.0,
            amplitude: 0.5,
            shoulder_width: 1.0,
        },
    )
    .unwrap();
    let sample = face
        .sample(PhysicalVector { x: 0.5, y: 5.0 })
        .unwrap()
        .unwrap();
    assert_eq!(sample.height, 0.25);
    // S(1/2)=1/2 and S'(1/2)=15/8; dz/dx=(1/2)*(15/8).
    let slope: f64 = 15.0 / 16.0;
    let length = (1.0 + slope * slope).sqrt();
    assert!((sample.normal.x + slope / length).abs() < 1e-12);
    assert_eq!(sample.normal.y, 0.0);
    assert!((sample.normal.z - 1.0 / length).abs() < 1e-12);
    let edge = face
        .sample(PhysicalVector { x: 0.0, y: 5.0 })
        .unwrap()
        .unwrap();
    assert_eq!(edge.height, 0.0);
    assert_eq!(edge.normal.z, 1.0);
    let inner = face
        .sample(PhysicalVector { x: 1.0, y: 5.0 })
        .unwrap()
        .unwrap();
    assert_eq!(inner.height, 0.5);
    assert_eq!(inner.normal.z, 1.0);
}

#[test]
fn plateau_is_uniform_and_recess_is_the_signed_face_inverse() {
    for curvature in [OutlineCurvature::Circular, OutlineCurvature::Quartic] {
        for scale in [1e-300, 1e-100, 1.0, 1e100, 1e300] {
            let face = relief(ReliefKind::Convex, curvature, scale);
            let well = relief(ReliefKind::Recessed, curvature, scale);
            let bounds = face.content_bounds();
            assert_eq!(bounds, well.content_bounds());
            for iy in 0..=20 {
                for ix in 0..=40 {
                    let p = PhysicalVector {
                        x: bounds.x + bounds.width * f64::from(ix) / 40.0,
                        y: bounds.y + bounds.height * f64::from(iy) / 20.0,
                    };
                    for (surface, sign) in [(&face, 1.0), (&well, -1.0)] {
                        let s = surface.sample(p).unwrap().unwrap();
                        assert_eq!(s.height, sign * 0.5 * scale, "{p:?}");
                        assert_eq!(s.normal.x, 0.0);
                        assert_eq!(s.normal.y, 0.0);
                        assert_eq!(s.normal.z, 1.0);
                    }
                }
            }
            let p = PhysicalVector {
                x: 0.5 * scale,
                y: 30.0 * scale,
            };
            let raised = face.sample(p).unwrap().unwrap();
            let recessed = well.sample(p).unwrap().unwrap();
            assert_eq!(raised.height, -recessed.height);
            assert_eq!(raised.normal.x, -recessed.normal.x);
            assert_eq!(raised.normal.y, -recessed.normal.y);
            assert_eq!(raised.normal.z, recessed.normal.z);
        }
    }
}

#[test]
fn analytic_normal_matches_finite_differences_on_curved_shoulders() {
    for kind in [ReliefKind::Convex, ReliefKind::Recessed] {
        let face = relief(kind, OutlineCurvature::Quartic, 1.0);
        let epsilon = 1e-5;
        for p in [
            PhysicalVector { x: 4.0, y: 3.0 },
            PhysicalVector { x: 5.0, y: 2.5 },
            PhysicalVector { x: 116.0, y: 57.0 },
        ] {
            let s = face.sample(p).unwrap().unwrap();
            for axis in [0, 1] {
                let mut low = p;
                let mut high = p;
                if axis == 0 {
                    low.x -= epsilon;
                    high.x += epsilon;
                } else {
                    low.y -= epsilon;
                    high.y += epsilon;
                }
                let derivative = (face.sample(high).unwrap().unwrap().height
                    - face.sample(low).unwrap().unwrap().height)
                    / (2.0 * epsilon);
                let normal = if axis == 0 { s.normal.x } else { s.normal.y };
                assert!((derivative + normal / s.normal.z).abs() < 1e-7);
            }
            assert!(
                (s.normal.x * s.normal.x + s.normal.y * s.normal.y + s.normal.z * s.normal.z - 1.0)
                    .abs()
                    < 1e-12
            );
        }
    }
}

#[test]
fn thickness_and_relief_limits_fail_explicitly() {
    let face = relief(ReliefKind::Convex, OutlineCurvature::Quartic, 1.0);
    for profile in [
        SurfaceReliefProfile {
            kind: ReliefKind::Convex,
            body_thickness: 4.0,
            amplitude: 0.6,
            shoulder_width: 1.0,
        },
        SurfaceReliefProfile {
            kind: ReliefKind::Recessed,
            body_thickness: 0.5,
            amplitude: 0.5,
            shoulder_width: 1.0,
        },
        SurfaceReliefProfile {
            kind: ReliefKind::Convex,
            body_thickness: 4.0,
            amplitude: 0.5,
            shoulder_width: 10.0,
        },
        SurfaceReliefProfile {
            kind: ReliefKind::Flat,
            body_thickness: 4.0,
            amplitude: 0.1,
            shoulder_width: 0.0,
        },
        SurfaceReliefProfile {
            kind: ReliefKind::Convex,
            body_thickness: 4.0,
            amplitude: f64::NAN,
            shoulder_width: 1.0,
        },
        SurfaceReliefProfile {
            kind: ReliefKind::Convex,
            body_thickness: 1e300,
            amplitude: 0.5,
            shoulder_width: 1.0,
        },
    ] {
        assert!(resolve_surface_relief(face.outline().clone(), profile).is_err());
    }
    assert!(
        face.sample(PhysicalVector {
            x: f64::NAN,
            y: 0.0
        })
        .is_err()
    );
    assert!(
        face.sample(PhysicalVector { x: -1.0, y: 20.0 })
            .unwrap()
            .is_none()
    );
}

#[test]
fn bounded_normals_and_height_are_independent_of_nominal_thickness() {
    for kind in [ReliefKind::Flat, ReliefKind::Convex, ReliefKind::Recessed] {
        let face = relief(kind, OutlineCurvature::Quartic, 1.0);
        let mut profile = face.profile();
        profile.body_thickness = 10.0;
        let thick = resolve_surface_relief(face.outline().clone(), profile).unwrap();
        assert_eq!(face.content_bounds(), thick.content_bounds());
        for iy in 0..=80 {
            for ix in 0..=160 {
                let p = PhysicalVector {
                    x: 120.0 * f64::from(ix) / 160.0,
                    y: 60.0 * f64::from(iy) / 80.0,
                };
                let a = face.sample(p).unwrap();
                assert_eq!(a, thick.sample(p).unwrap());
                if let Some(a) = a {
                    assert!(a.height.is_finite() && a.height.abs() <= profile.amplitude);
                    assert!(
                        a.normal.x.is_finite()
                            && a.normal.y.is_finite()
                            && a.normal.z > 0.0
                            && a.normal.z <= 1.0
                    );
                    assert!(
                        (a.normal.x * a.normal.x
                            + a.normal.y * a.normal.y
                            + a.normal.z * a.normal.z
                            - 1.0)
                            .abs()
                            <= 1e-12
                    );
                    assert!((a.normal.x / a.normal.z).abs() <= 45.0 / 8.0);
                    assert!((a.normal.y / a.normal.z).abs() <= 45.0 / 8.0);
                }
            }
        }
    }
}

#[test]
fn asymmetric_logical_mirroring_preserves_height_and_physical_normals() {
    let radii = LogicalCornerRadii {
        top_start: CornerRadius { x: 22.0, y: 14.0 },
        top_end: CornerRadius { x: 12.0, y: 10.0 },
        bottom_end: CornerRadius { x: 18.0, y: 16.0 },
        bottom_start: CornerRadius { x: 10.0, y: 8.0 },
    };
    let build = |direction| {
        resolve_surface_relief(
            resolve_surface_outline(
                SurfaceSize {
                    width: 120.0,
                    height: 60.0,
                },
                radii,
                direction,
                OutlineCurvature::Quartic,
                2.0,
            )
            .unwrap(),
            SurfaceReliefProfile {
                kind: ReliefKind::Convex,
                body_thickness: 4.0,
                amplitude: 0.5,
                shoulder_width: 1.0,
            },
        )
        .unwrap()
    };
    let ltr = build(LayoutDirection::Ltr);
    let rtl = build(LayoutDirection::Rtl);
    for iy in 0..120 {
        for ix in 0..240 {
            let p = PhysicalVector {
                x: (f64::from(ix) + 0.5) / 2.0,
                y: (f64::from(iy) + 0.5) / 2.0,
            };
            let a = ltr.sample(p).unwrap();
            let b = rtl
                .sample(PhysicalVector {
                    x: 120.0 - p.x,
                    y: p.y,
                })
                .unwrap();
            assert_eq!(a.is_some(), b.is_some());
            if let (Some(a), Some(b)) = (a, b) {
                assert!((a.height - b.height).abs() < 1e-12);
                assert!((a.normal.x + b.normal.x).abs() < 1e-12);
                assert!((a.normal.y - b.normal.y).abs() < 1e-12);
                assert!((a.normal.z - b.normal.z).abs() < 1e-12);
            }
        }
    }
}
