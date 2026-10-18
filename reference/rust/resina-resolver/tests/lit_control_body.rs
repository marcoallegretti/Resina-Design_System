use resina_environment::EnvironmentSnapshot;
use resina_model::{KeyLight, MaterialLightChannels, OpaquePigmentProfiles, PhysicalVector};
use resina_resolver::{
    LitControlBodyInput, LitControlBodyIr, LitControlIdentity, parse_control_relief_source,
    resolve_edge_contrast_source, resolve_lit_control_body, resolve_material_relief_paint_source,
    resolve_surface_outline, resolve_surface_relief,
};
use serde_json::{Value, json};

fn baseline() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/ir/lit-control-body-source.json"
    ))
    .unwrap()
}

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../../conformance/ir/lit-control-body-vectors.json"
    ))
    .unwrap()
}

fn case_source(case: &Value) -> Value {
    let mut source = baseline();
    for change in case["sourceChanges"].as_array().unwrap() {
        *source
            .pointer_mut(change["path"].as_str().unwrap())
            .unwrap_or_else(|| panic!("{}: missing {}", case["name"], change["path"])) =
            change["value"].clone();
    }
    source
}

fn named(name: &str) -> Value {
    case_source(
        cases()
            .iter()
            .find(|case| case["name"] == name)
            .unwrap_or_else(|| panic!("{name}")),
    )
}

fn resolve(source: &Value) -> Result<LitControlBodyIr, String> {
    let decode = |name: &str| source[name].clone();
    let identity = &source["identity"];
    let identity = LitControlIdentity {
        material_role: serde_json::from_value(identity["materialRole"].clone())
            .map_err(|e| e.to_string())?,
        color_role: serde_json::from_value(identity["colorRole"].clone())
            .map_err(|e| e.to_string())?,
        material_family: serde_json::from_value(identity["materialFamily"].clone())
            .map_err(|e| format!("unsupported material family: {e}"))?,
        form: serde_json::from_value(identity["form"].clone()).map_err(|e| e.to_string())?,
        states: serde_json::from_value(identity["states"].clone())
            .map_err(|e| format!("unsupported state: {e}"))?,
        frost_representation: identity
            .get("frostRepresentation")
            .map(|value| serde_json::from_value(value.clone()))
            .transpose()
            .map_err(|e| e.to_string())?,
        foreground_role: serde_json::from_value(identity["foregroundRole"].clone())
            .map_err(|e| e.to_string())?,
        content_fallback_applied: identity["contentFallbackApplied"].as_bool().unwrap(),
    };
    let body = resina_color::resolve_srgb_fallback(&decode("body")).map_err(|e| e.to_string())?;
    let foreground =
        resina_color::resolve_srgb_fallback(&decode("foreground")).map_err(|e| e.to_string())?;
    let profiles: OpaquePigmentProfiles =
        serde_json::from_value(decode("pigmentProfiles")).map_err(|e| e.to_string())?;
    let key_light: KeyLight =
        serde_json::from_value(decode("keyLight")).map_err(|e| e.to_string())?;
    let light: MaterialLightChannels =
        serde_json::from_value(decode("light")).map_err(|e| e.to_string())?;
    let environment: EnvironmentSnapshot =
        serde_json::from_value(decode("environment")).map_err(|e| e.to_string())?;
    let edge =
        resolve_edge_contrast_source(&decode("edge").to_string()).map_err(|e| e.to_string())?;
    resolve_lit_control_body(LitControlBodyInput {
        identity,
        size: serde_json::from_value(decode("size")).map_err(|e| e.to_string())?,
        radii: serde_json::from_value(decode("radii")).map_err(|e| e.to_string())?,
        relief: serde_json::from_value(decode("relief")).map_err(|e| e.to_string())?,
        body: &body,
        pigment_profiles: &profiles,
        key_light: &key_light,
        light: &light,
        environment: &environment,
        edge,
        edge_width: source["edgeWidth"].as_f64().unwrap(),
        foreground,
        minimum_content_contrast: source["minimumContentContrast"].as_f64().unwrap(),
        depth: source["depth"].as_f64().unwrap(),
    })
    .map_err(|e| e.to_string())
}

fn points(source: &Value) -> Vec<PhysicalVector> {
    source["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|point| serde_json::from_value(point.clone()).unwrap())
        .collect()
}

fn mismatch(actual: &Value, expected: &Value, tolerance: f64, path: &str) -> Option<String> {
    match (actual, expected) {
        (Value::Number(a), Value::Number(e)) => {
            let (a, e) = (a.as_f64().unwrap(), e.as_f64().unwrap());
            ((a - e).abs() > tolerance.max(tolerance * e.abs()))
                .then(|| format!("{path}: {a} != {e}"))
        }
        (Value::Object(a), Value::Object(e)) => {
            if a.keys().collect::<Vec<_>>() != e.keys().collect::<Vec<_>>() {
                return Some(format!("{path}: members differ"));
            }
            e.iter().find_map(|(name, value)| {
                mismatch(&a[name], value, tolerance, &format!("{path}/{name}"))
            })
        }
        (Value::Array(a), Value::Array(e)) => {
            if a.len() != e.len() {
                return Some(format!("{path}: lengths differ"));
            }
            a.iter()
                .zip(e)
                .enumerate()
                .find_map(|(index, (a, e))| mismatch(a, e, tolerance, &format!("{path}/{index}")))
        }
        _ => (actual != expected).then(|| format!("{path}: {actual} != {expected}")),
    }
}

fn assert_close(actual: &Value, expected: &Value, path: &str) {
    if let Some(found) = mismatch(
        &sorted(actual.clone()),
        &sorted(expected.clone()),
        1e-12,
        path,
    ) {
        panic!("{found}");
    }
}

fn same(actual: &Value, expected: &Value) -> bool {
    mismatch(&sorted(actual.clone()), &sorted(expected.clone()), 0.0, "").is_none()
}

fn sorted(value: Value) -> Value {
    serde_json::from_str(&value.to_string()).unwrap()
}

#[test]
fn authored_vectors_classify_and_color_every_region() {
    for case in cases() {
        let source = case_source(&case);
        let resolved = resolve(&source);
        if let Some(fragment) = case.get("errorContains") {
            let error = resolved.expect_err(case["name"].as_str().unwrap());
            assert!(
                error.contains(fragment.as_str().unwrap()),
                "{}: {error}",
                case["name"]
            );
            continue;
        }
        let body = resolved.unwrap_or_else(|e| panic!("{}: {e}", case["name"]));
        let expected = &case["expected"];
        let ir = serde_json::to_value(&body).unwrap();
        assert_close(&ir["face"]["light"], &expected["light"], "/light");
        assert_close(
            &ir["lighting"]["sideOffset"],
            &expected["sideOffset"],
            "/sideOffset",
        );
        assert_close(
            &ir["face"]["contentBounds"],
            &expected["contentBounds"],
            "/contentBounds",
        );
        assert_close(
            &ir["face"]["colorRanges"],
            &expected["colorRanges"],
            "/colorRanges",
        );
        let exact = [
            source["body"].clone(),
            serde_json::to_value(body.face().pigment().side()).unwrap(),
            serde_json::to_value(body.face().edge().color()).unwrap(),
        ];
        for (point, sample) in points(&source)
            .into_iter()
            .zip(expected["samples"].as_array().unwrap())
        {
            let color = serde_json::to_value(body.sample_paint(point).unwrap()).unwrap();
            let label = format!("{} {point:?} rule {}", case["name"], sample["rule"]);
            if sample["color"].is_null() || exact.iter().any(|color| same(color, &sample["color"]))
            {
                assert!(
                    same(&color, &sample["color"]),
                    "{label}: {color} != {}",
                    sample["color"]
                );
            } else {
                assert_close(&color, &sample["color"], &label);
            }
        }
    }
}

#[test]
fn depth_zero_bodies_are_the_material_relief_face() {
    for case in cases() {
        let source = case_source(&case);
        if case.get("expected").is_none() || source["depth"] != json!(0) {
            continue;
        }
        let body = resolve(&source).unwrap();
        let states = source["identity"]["states"]["states"].as_array().unwrap();
        let has = |name: &str| states.iter().any(|state| state == name);
        let request = json!({
            "schemaVersion": "0.1.0",
            "outline": {
                "schemaVersion": "0.1.0", "size": source["size"], "radii": source["radii"],
                "layoutDirection": source["environment"]["layoutDirection"],
                "curvature": "circular", "minimumInset": source["edgeWidth"]
            },
            "profile": source["relief"],
            "pigment": {
                "schemaVersion": "0.1.0", "materialFamily": source["identity"]["materialFamily"],
                "body": source["body"], "profiles": source["pigmentProfiles"]
            },
            "light": {
                "schemaVersion": "0.1.0", "keyLight": source["keyLight"],
                "ambient": source["light"]["ambient"], "interaction": source["light"]["interaction"]
            },
            "environment": source["environment"],
            "enabled": true,
            "interactionActive": !has("disabled") && (has("hover") || has("pressed")),
            "edge": source["edge"],
            "edgeWidth": source["edgeWidth"],
            "foreground": source["foreground"],
            "minimumContentContrast": source["minimumContentContrast"],
            "points": source["points"]
        });
        let face = resolve_material_relief_paint_source(&request.to_string()).unwrap();
        assert_eq!(
            serde_json::to_value(body.face()).unwrap(),
            serde_json::to_value(&face.paint).unwrap(),
            "{}",
            case["name"]
        );
        for (point, sample) in points(&source).into_iter().zip(face.samples) {
            assert_eq!(
                body.sample_paint(point).unwrap(),
                sample.color,
                "{} {point:?}",
                case["name"]
            );
        }
    }
}

#[test]
fn published_invariants_hold_for_every_resolved_body() {
    for case in cases()
        .into_iter()
        .filter(|case| case.get("expected").is_some())
    {
        let source = case_source(&case);
        let body = resolve(&source).unwrap();
        let ir = serde_json::to_value(&body).unwrap();
        let name = case["name"].as_str().unwrap();
        assert_eq!(
            ir["face"]["light"]["keyDirection"], ir["lighting"]["direction"],
            "{name}"
        );
        assert_eq!(ir["representation"], "opaque");
        assert!(ir.get("highlightWidth").is_none());
        assert_eq!(ir["face"]["colorRanges"].as_array().unwrap().len(), 3);
        let offset = body.lighting().side_offset();
        let direction = body.lighting().direction();
        assert_eq!(offset.x, -direction.x * body.depth(), "{name}");
        assert_eq!(offset.y, -direction.y * body.depth(), "{name}");
        let size = &source["size"];
        let bounds = body.geometry().silhouette().bounds().unwrap();
        assert_eq!(bounds.x, offset.x.min(0.0), "{name}");
        assert_eq!(bounds.y, offset.y.min(0.0), "{name}");
        assert_eq!(
            bounds.width,
            size["width"].as_f64().unwrap() + offset.x.abs(),
            "{name}"
        );
        assert_eq!(
            bounds.height,
            size["height"].as_f64().unwrap() + offset.y.abs(),
            "{name}"
        );
        let edge = source["edgeWidth"].as_f64().unwrap();
        assert_eq!(
            body.geometry().edge_interior().offset(),
            PhysicalVector { x: edge, y: edge }
        );
        let content = body.face().content_bounds();
        let plateau = body.face().surface().content_bounds();
        assert!(content.x >= plateau.x && content.y >= plateau.y, "{name}");
        assert!(
            content.x + content.width <= plateau.x + plateau.width,
            "{name}"
        );
        assert!(
            content.y + content.height <= plateau.y + plateau.height,
            "{name}"
        );
        let ranges = body.paint_color_ranges();
        let side = body.face().pigment().side();
        assert!(
            ranges[1]
                .lower()
                .components()
                .iter()
                .zip(side.components())
                .all(|(l, s)| *l <= s)
        );
        assert!(
            ranges[1]
                .upper()
                .components()
                .iter()
                .zip(side.components())
                .all(|(u, s)| *u >= s)
        );
    }
}

#[test]
fn baseline_body_matches_the_published_ir_fixture() {
    let body = resolve(&named("hover-admits-interaction")).unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/lit-control-body-ir.json"
    ))
    .unwrap();
    assert_close(
        &sorted(serde_json::to_value(&body).unwrap()),
        &sorted(expected),
        "",
    );
}

#[test]
fn light_never_changes_body_edge_side_or_content() {
    let lit = resolve(&named("hover-admits-interaction")).unwrap();
    let dark = resolve(&named("absent-channels-keep-static-key")).unwrap();
    let content = lit.face().content_bounds();
    assert_eq!(content, dark.face().content_bounds());
    assert_eq!(lit.face().edge(), dark.face().edge());
    assert_eq!(lit.face().pigment(), dark.face().pigment());
    for point in [
        PhysicalVector { x: 48.0, y: 24.0 },
        PhysicalVector {
            x: content.x,
            y: content.y,
        },
        PhysicalVector { x: 48.0, y: 48.3 },
        PhysicalVector { x: 48.0, y: 0.5 },
        PhysicalVector { x: 97.0, y: 24.0 },
    ] {
        assert_eq!(
            lit.sample_paint(point),
            dark.sample_paint(point),
            "{point:?}"
        );
    }
    assert_ne!(
        lit.sample_paint(PhysicalVector { x: 48.0, y: 1.5 }),
        dark.sample_paint(PhysicalVector { x: 48.0, y: 1.5 })
    );
    assert!(
        lit.sample_paint(PhysicalVector {
            x: f64::NAN,
            y: 0.0
        })
        .is_err()
    );
}

#[test]
fn reference_control_relief_profiles_certify_their_illustrative_parts() {
    let relief =
        parse_control_relief_source(include_str!("../../../../definitions/control-relief.json"))
            .unwrap();
    let plateau = |width: f64, height: f64, radius: f64, profile| {
        let corner = resina_model::CornerRadius {
            x: radius,
            y: radius,
        };
        let outline = resolve_surface_outline(
            resina_model::SurfaceSize { width, height },
            resina_model::LogicalCornerRadii {
                top_start: corner,
                top_end: corner,
                bottom_end: corner,
                bottom_start: corner,
            },
            resina_environment::LayoutDirection::Ltr,
            resina_resolver::OutlineCurvature::Circular,
            1.0,
        )
        .unwrap();
        resolve_surface_relief(outline, profile)
            .unwrap()
            .content_bounds()
    };
    let command = plateau(96.0, 48.0, 24.0, relief.command_body());
    assert!((command.y - 10.1436).abs() < 1e-4, "{command:?}");
    assert!(command.y < 12.0);
    let track = plateau(64.5, 40.5, 20.25, relief.toggle_track());
    assert!(
        (track.x - 8.24).abs() < 5e-3 && (track.x + track.width - 56.26).abs() < 5e-3,
        "{track:?}"
    );
    assert!(
        (track.y - 8.24).abs() < 5e-3 && (track.y + track.height - 32.26).abs() < 5e-3,
        "{track:?}"
    );
    let thumb = plateau(16.0, 16.0, 8.0, relief.toggle_thumb());
    assert!(thumb.width > 0.0 && thumb.height > 0.0, "{thumb:?}");
    for source in [
        r#"{"schemaVersion":"0.2.0","commandBody":{"kind":"flat","bodyThickness":1,"amplitude":0,"shoulderWidth":0},"toggleTrack":{"kind":"flat","bodyThickness":1,"amplitude":0,"shoulderWidth":0},"toggleThumb":{"kind":"flat","bodyThickness":1,"amplitude":0,"shoulderWidth":0}}"#,
        r#"{"schemaVersion":"0.1.0","commandBody":["convex",2,1,2],"toggleTrack":{"kind":"flat","bodyThickness":1,"amplitude":0,"shoulderWidth":0},"toggleThumb":{"kind":"flat","bodyThickness":1,"amplitude":0,"shoulderWidth":0}}"#,
        r#"{"schemaVersion":"0.1.0","commandBody":{"kind":"flat","bodyThickness":1,"amplitude":0,"shoulderWidth":0},"toggleThumb":{"kind":"flat","bodyThickness":1,"amplitude":0,"shoulderWidth":0}}"#,
        r#"{"schemaVersion":"0.1.0","commandBody":{"kind":"flat","bodyThickness":1,"amplitude":0,"shoulderWidth":0},"toggleTrack":{"kind":"flat","bodyThickness":1,"amplitude":0,"shoulderWidth":0},"toggleThumb":{"kind":"flat","bodyThickness":1,"amplitude":0,"shoulderWidth":0},"sliderTrack":{"kind":"flat","bodyThickness":1,"amplitude":0,"shoulderWidth":0}}"#,
    ] {
        assert!(parse_control_relief_source(source).is_err(), "{source}");
    }
}
