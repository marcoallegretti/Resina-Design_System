use resina_model::PhysicalVector;
use resina_resolver::resolve_relief_paint_source;
use serde_json::{Value, json};

fn request(kind: &str, curve: &str) -> Value {
    json!({
        "schemaVersion": "0.1.0",
        "outline": {
            "schemaVersion": "0.1.0", "size": {"width": 120, "height": 60},
            "radii": {"topStart": {"x": 20, "y": 12}, "topEnd": {"x": 30, "y": 15},
                "bottomEnd": {"x": 24, "y": 18}, "bottomStart": {"x": 12, "y": 20}},
            "layoutDirection": "ltr", "curvature": curve, "minimumInset": 2
        },
        "profile": {"kind": kind, "bodyThickness": 4,
            "amplitude": if kind == "flat" {0.0} else {0.25},
            "shoulderWidth": if kind == "flat" {0.0} else {1.0}},
        "pigment": {"schemaVersion": "0.1.0", "materialFamily": "elastomer",
            "body": color([0.5, 0.4, 0.3]),
            "profiles": serde_json::from_str::<Value>(include_str!("../../../../definitions/tier0-pigment.json")).unwrap()},
        "keyLight": {"schemaVersion": "0.1.0", "direction": {"x": -1, "y": -1}},
        "edge": {"schemaVersion": "0.1.0", "outline": color([0.0; 3]),
            "outlineStrong": color([0.0; 3]), "adjacentColor": color([1.0; 3]), "minimumContrast": 3},
        "edgeWidth": 0.25, "foreground": color([1.0; 3]), "minimumContentContrast": 3,
        "points": []
    })
}

fn color(components: [f64; 3]) -> Value {
    json!({"colorSpace": "srgb", "components": components, "alpha": 1})
}

#[test]
fn signed_tilt_retains_strength_and_flat_plateau_is_exact_body() {
    for curve in ["circular", "quartic"] {
        for kind in ["flat", "convex", "recessed"] {
            let result = resolve_relief_paint_source(&request(kind, curve).to_string()).unwrap();
            let paint = &result.paint;
            let plateau = paint.content_bounds();
            for ix in 0..=10 {
                for iy in 0..=10 {
                    let point = PhysicalVector {
                        x: plateau.x + plateau.width * f64::from(ix) / 10.0,
                        y: plateau.y + plateau.height * f64::from(iy) / 10.0,
                    };
                    assert_eq!(
                        paint.sample_paint(point).unwrap().as_ref(),
                        Some(paint.pigment().body())
                    );
                }
            }
            let point = PhysicalVector { x: 60.0, y: 0.5 };
            let normal = paint.surface().sample(point).unwrap().unwrap().normal;
            let q = -(normal.x + normal.y) / 2.0_f64.sqrt();
            let endpoint = if q >= 0.0 {
                paint.pigment().highlight()
            } else {
                paint.pigment().side()
            };
            let actual = paint.sample_paint(point).unwrap().unwrap();
            for ((got, body), tip) in actual
                .components()
                .into_iter()
                .zip(paint.pigment().body().components())
                .zip(endpoint.components())
            {
                assert!((got - (body * (1.0 - q.abs()) + tip * q.abs())).abs() <= 1e-12);
            }
            if kind != "flat" {
                assert!(q.abs() > 0.0 && q.abs() < 0.5);
            }
        }
    }
}

#[test]
fn edge_is_opaque_and_quartic_membership_is_not_circular() {
    for curve in ["circular", "quartic"] {
        let result = resolve_relief_paint_source(&request("convex", curve).to_string()).unwrap();
        let paint = result.paint;
        assert_eq!(
            paint
                .sample_paint(PhysicalVector { x: 60.0, y: 0.125 })
                .unwrap()
                .as_ref(),
            Some(paint.edge().color())
        );
        assert!(
            paint
                .sample_paint(PhysicalVector { x: -1.0, y: 30.0 })
                .unwrap()
                .is_none()
        );
        for y in 0..=60 {
            for x in 0..=120 {
                let point = PhysicalVector {
                    x: f64::from(x),
                    y: f64::from(y),
                };
                if let Some(sample) = paint.sample_paint(point).unwrap() {
                    assert_eq!(sample.alpha(), 1.0);
                    assert!(paint.paint_color_ranges().iter().any(|range| {
                        sample
                            .components()
                            .into_iter()
                            .zip(range.lower().components())
                            .zip(range.upper().components())
                            .all(|((v, lo), hi)| v >= lo && v <= hi)
                    }));
                }
            }
        }
    }
    let circular = resolve_relief_paint_source(&request("flat", "circular").to_string()).unwrap();
    let quartic = resolve_relief_paint_source(&request("flat", "quartic").to_string()).unwrap();
    let point = PhysicalVector { x: 4.0, y: 2.4 };
    assert!(circular.paint.sample_paint(point).unwrap().is_none());
    assert!(quartic.paint.sample_paint(point).unwrap().is_some());
}

#[test]
fn rejects_unreadable_collapsed_translucent_and_positional_inputs() {
    let baseline = request("convex", "quartic");
    for (pointer, value) in [
        ("/edgeWidth", json!(0)),
        ("/edgeWidth", json!(30)),
        ("/minimumContentContrast", json!(21)),
        ("/foreground/alpha", json!(0.5)),
        ("/keyLight", json!(["0.1.0",{"x":-1,"y":-1}])),
        ("/points", json!([[1, 2]])),
        ("/profile", json!(null)),
    ] {
        let mut bad = baseline.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            resolve_relief_paint_source(&bad.to_string()).is_err(),
            "{pointer}"
        );
    }
    assert!(
        resolve_relief_paint_source(&baseline.to_string().replacen(
            "\"edgeWidth\":0.25",
            "\"edgeWidth\":0.25,\"edgeWidth\":0.25",
            1
        ))
        .is_err()
    );
}

#[test]
fn logical_mapping_preserves_physical_light_and_large_corner_inset() {
    let source = request("convex", "quartic");
    let ltr = resolve_relief_paint_source(&source.to_string())
        .unwrap()
        .paint;
    let mut mirrored = source.clone();
    mirrored["outline"]["layoutDirection"] = json!("rtl");
    let rtl = resolve_relief_paint_source(&mirrored.to_string())
        .unwrap()
        .paint;
    let point = PhysicalVector { x: 60.0, y: 0.5 };
    assert_eq!(
        ltr.sample_paint(point).unwrap(),
        rtl.sample_paint(point).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&ltr).unwrap()["direction"],
        serde_json::to_value(&rtl).unwrap()["direction"]
    );
    assert!(
        ltr.sample_paint(PhysicalVector {
            x: f64::NAN,
            y: 0.0
        })
        .is_err()
    );

    let mut large = request("flat", "quartic");
    large["outline"]["radii"] = json!({
        "topStart":{"x":0,"y":0},"topEnd":{"x":120,"y":30},
        "bottomEnd":{"x":0,"y":0},"bottomStart":{"x":0,"y":0}});
    large["edgeWidth"] = json!(5);
    let paint = resolve_relief_paint_source(&large.to_string())
        .unwrap()
        .paint;
    // The derived top-right x axis is 115 while the inset box is only 110 wide.
    // Renormalizing that axis would alter this independently authored region.
    for y in 5..=55 {
        for x in 5..=115 {
            let point = PhysicalVector {
                x: f64::from(x),
                y: f64::from(y),
            };
            if !paint.surface().outline().contains(point).unwrap() {
                continue;
            }
            let u = (point.x / 115.0).max(0.0);
            let v = ((30.0 - point.y) / 25.0).max(0.0);
            let expected = if u.powi(4) + v.powi(4) <= 1.0 {
                paint.pigment().body()
            } else {
                paint.edge().color()
            };
            assert_eq!(paint.sample_paint(point).unwrap().as_ref(), Some(expected));
        }
    }
}

#[test]
fn closed_edge_interior_distinguishes_exact_boundary_and_neighbors() {
    for (size, radius, point) in [(56.0, 14.0, (9.0, 2.0_f64)), (18.0, 6.0, (3.0, 2.0))] {
        let mut source = request("flat", "circular");
        source["outline"]["size"] = json!({"width":size,"height":size});
        source["outline"]["minimumInset"] = json!(0);
        for corner in ["topStart", "topEnd", "bottomEnd", "bottomStart"] {
            source["outline"]["radii"][corner] = json!({"x":radius,"y":radius});
        }
        source["edgeWidth"] = json!(1);
        let paint = resolve_relief_paint_source(&source.to_string())
            .unwrap()
            .paint;
        for (y, expected) in [
            (point.1.next_down(), paint.edge().color()),
            (point.1, paint.pigment().body()),
            (point.1.next_up(), paint.pigment().body()),
        ] {
            assert_eq!(
                paint
                    .sample_paint(PhysicalVector { x: point.0, y })
                    .unwrap()
                    .as_ref(),
                Some(expected),
                "size={size} y={y}"
            );
        }
    }
}
