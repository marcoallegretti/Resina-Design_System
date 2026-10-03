use resina_model::PhysicalVector;
use resina_resolver::{OpaqueSurfaceIr, SurfacePaintError, resolve_opaque_surface_source};
use serde_json::{Value, json};

fn request() -> Value {
    let mut request: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json"
    ))
    .unwrap();
    let mut theme: Value =
        serde_json::from_str(request["theme"]["themeSource"].as_str().unwrap()).unwrap();
    theme["tokens"]["palette"]["base"]["$value"]["components"] = json!([0.25, 0.25, 0.25]);
    request["theme"]["themeSource"] = theme.to_string().into();
    request["minimumContentContrast"] = 1.into();
    request
}

fn sample(ir: &OpaqueSurfaceIr, x: f64, y: f64, expected: Option<f64>) {
    let actual = ir.sample_paint(PhysicalVector { x, y }).unwrap();
    if let Some(channel) = expected {
        let actual = actual.unwrap();
        assert_eq!(actual.alpha(), 1.0);
        for c in actual.components() {
            assert!((c - channel).abs() <= 1e-12, "({x},{y}): {c} != {channel}");
        }
    } else {
        assert!(actual.is_none(), "({x},{y}) must be unpainted");
    }
}

#[test]
fn square_regions_and_closed_boundaries_follow_paint_order() {
    let ir = resolve_opaque_surface_source(&request().to_string()).unwrap();
    for (x, y, channel) in [
        (-0.01, 6.0, None),
        (10.0, 14.01, None),
        (0.0, 6.0, Some(0.0)),
        (10.0, 14.0, Some(0.0)),
        (0.5, 6.0, Some(0.0)),
        (10.0, 13.5, Some(0.0)),
        (10.0, 12.5, Some(0.225)),
        (10.0, 12.0, Some(0.25)),
        (10.0, 1.0, Some(0.31)),
        (10.0, 1.5, Some(0.31)),
        (1.5, 1.5, Some(0.31)),
        (1.5, 2.0, Some(0.25)),
        (10.0, 2.0, Some(0.25)),
        (10.0, 10.5, Some(0.25)),
        (10.0, 6.0, Some(0.25)),
    ] {
        sample(&ir, x, y, channel);
    }
}

#[test]
fn rounded_highlight_uses_geometric_normals_and_rejects_corner_voids() {
    let mut request = request();
    let mut theme: Value =
        serde_json::from_str(request["theme"]["themeSource"].as_str().unwrap()).unwrap();
    theme["tokens"]["radius"]["0"]["$value"]["value"] = 4.into();
    request["theme"]["themeSource"] = theme.to_string().into();
    let ir = resolve_opaque_surface_source(&request.to_string()).unwrap();
    sample(&ir, 0.1, 0.1, None);
    let radial = 1.0 / 2.0_f64.sqrt();
    sample(
        &ir,
        4.0 - 2.5 * radial,
        4.0 - 2.5 * radial,
        Some(0.25 + 0.06 * radial),
    );
    sample(
        &ir,
        16.0 + 2.5 * radial,
        4.0 - 2.5 * radial,
        Some(0.25 + 0.06 * radial),
    );
    sample(&ir, 16.0 + 2.5 * radial, 8.0 + 2.5 * radial, Some(0.25));
    sample(&ir, 4.0 - 3.5 * radial, 4.0 - 3.5 * radial, Some(0.0));
    sample(&ir, 4.0 - 1.5 * radial, 4.0 - 1.5 * radial, Some(0.25));
}

#[test]
fn unequal_nearest_segments_do_not_inherit_a_brighter_tied_normal() {
    let ir = resolve_opaque_surface_source(&request().to_string()).unwrap();
    sample(&ir, 1.5, 1.500001, Some(0.25));
    sample(&ir, 1.500001, 1.5, Some(0.31));
}

#[test]
fn zero_highlight_and_zero_depth_have_no_fabricated_regions() {
    let mut request = request();
    request["appearance"]["bands"]["cast"]["highlightWidth"] = 0.into();
    request["surface"]["form"]["elevation"] = "base".into();
    let ir = resolve_opaque_surface_source(&request.to_string()).unwrap();
    sample(&ir, 10.0, 1.5, Some(0.25));
    sample(&ir, 10.0, 12.5, None);
}

#[test]
fn invalid_points_fail_and_finite_distant_points_remain_unpainted() {
    let ir = resolve_opaque_surface_source(&request().to_string()).unwrap();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            ir.sample_paint(PhysicalVector { x: value, y: 0.0 }),
            Err(SurfacePaintError::InvalidPoint)
        );
        assert_eq!(
            ir.sample_paint(PhysicalVector { x: 0.0, y: value }),
            Err(SurfacePaintError::InvalidPoint)
        );
    }
    sample(&ir, f64::MAX, f64::MAX, None);
    sample(&ir, -f64::MAX, -f64::MAX, None);
}

#[test]
fn every_family_uses_its_resolved_chromatic_pigments() {
    for (family, side_shade, highlight_lift) in [
        ("cast", 0.1, 0.08),
        ("frost", 0.14, 0.18),
        ("elastomer", 0.25, 0.14),
        ("gel", 0.18, 0.2),
    ] {
        let mut request = request();
        let mut theme: Value =
            serde_json::from_str(request["theme"]["themeSource"].as_str().unwrap()).unwrap();
        theme["materialAssignments"]["surface"]["transient"] = family.into();
        let body = [0.2, 0.4, 0.6];
        for token in ["base", "opaque", "opaqueAlt"] {
            theme["tokens"]["palette"][token]["$value"]["components"] = json!(body);
        }
        request["theme"]["themeSource"] = theme.to_string().into();
        request["surface"]["materialRole"] = "surface.transient".into();
        let ir = resolve_opaque_surface_source(&request.to_string()).unwrap();
        for (point, expected) in [
            (PhysicalVector { x: 10.0, y: 6.0 }, body),
            (
                PhysicalVector { x: 10.0, y: 12.5 },
                body.map(|c| c * (1.0 - side_shade)),
            ),
            (
                PhysicalVector { x: 10.0, y: 1.5 },
                body.map(|c| c + (1.0 - c) * highlight_lift),
            ),
        ] {
            let actual = ir.sample_paint(point).unwrap().unwrap();
            for (actual, expected) in actual.components().into_iter().zip(expected) {
                assert!(
                    (actual - expected).abs() <= 1e-12,
                    "{family}: {actual} != {expected}"
                );
            }
        }
    }
}
