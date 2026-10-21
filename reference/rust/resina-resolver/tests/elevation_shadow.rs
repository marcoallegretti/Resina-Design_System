use resina_resolver::resolve_elevation_shadow_source;
use serde_json::{Value, json};

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../../conformance/elevation/shadow-vectors.json"
    ))
    .unwrap()
}

fn resolve(request: &Value) -> Value {
    serde_json::to_value(resolve_elevation_shadow_source(&request.to_string()).unwrap()).unwrap()
}

const GEOMETRY: [&str; 8] = [
    "/finalDepth",
    "/sideOffset/",
    "/contactMargin",
    "/shadow/sweep/",
    "/shadow/hull/",
    "/shadow/bandThickness/",
    "/shadow/penumbraWidth",
    "/shadow/paintBounds/",
];

/// Logical-px geometry is compared absolutely or relatively; every other number,
/// including the hull's unit radial parameters, absolutely (spec/92).
fn geometry(path: &str) -> bool {
    let radial = path.ends_with("/start/x")
        || path.ends_with("/start/y")
        || path.ends_with("/end/x")
        || path.ends_with("/end/y");
    !radial && GEOMETRY.iter().any(|prefix| path.starts_with(prefix))
}

fn compare(actual: &Value, expected: &Value, path: &str, name: &Value) {
    match expected {
        Value::Object(object) => {
            let actual = actual.as_object().unwrap();
            assert_eq!(actual.len(), object.len(), "{name} {path}");
            for (key, value) in object {
                compare(&actual[key], value, &format!("{path}/{key}"), name);
            }
        }
        Value::Array(values) => {
            let actual = actual.as_array().unwrap();
            assert_eq!(actual.len(), values.len(), "{name} {path}");
            for (index, (a, e)) in actual.iter().zip(values).enumerate() {
                compare(a, e, &format!("{path}/{index}"), name);
            }
        }
        Value::Number(number) => {
            let (actual, expected) = (actual.as_f64().unwrap(), number.as_f64().unwrap());
            let bound = if geometry(path) {
                1e-12_f64.max(1e-12 * expected.abs())
            } else {
                1e-12
            };
            assert!(
                (actual - expected).abs() <= bound,
                "{name} {path}: {actual} != {expected}"
            );
        }
        _ => assert_eq!(actual, expected, "{name} {path}"),
    }
}

#[test]
fn tolerance_classes_follow_the_result_members() {
    for path in [
        "/finalDepth",
        "/sideOffset/x",
        "/shadow/sweep/y",
        "/shadow/hull/bounds/width",
        "/shadow/hull/segments/0/center/x",
        "/shadow/hull/segments/0/radii/y",
        "/shadow/bandThickness/bottom",
        "/shadow/penumbraWidth",
        "/shadow/paintBounds/height",
    ] {
        assert!(geometry(path), "{path}");
    }
    for path in [
        "/keyDirection/x",
        "/shadowShade",
        "/shadow/hull/segments/0/start/x",
        "/shadow/hull/segments/0/end/y",
        "/shadow/bandColor/components/0",
        "/shadow/colorRange/lower/components/0",
        "/shadow/deltaEOk",
        "/edge/contrastRatio",
        "/samples/0/weight",
    ] {
        assert!(!geometry(path), "{path}");
    }
}

#[test]
fn shadow_vectors() {
    for case in cases() {
        let result = resolve_elevation_shadow_source(&case["request"].to_string());
        if let Some(expected) = case.get("expected") {
            let actual =
                serde_json::to_value(result.unwrap_or_else(|e| panic!("{}: {e}", case["name"])))
                    .unwrap();
            compare(&actual, expected, "", &case["name"]);
        } else {
            let error = result.unwrap_err().to_string();
            assert!(
                error.contains(case["errorContains"].as_str().unwrap()),
                "{}: {error}",
                case["name"]
            );
        }
    }
}

fn baseline() -> Value {
    cases()
        .into_iter()
        .find(|case| case["name"] == "light raised depthScale 1 on surface.base")
        .unwrap()["request"]
        .clone()
}

fn support(segments: &[Value], n: (f64, f64)) -> f64 {
    let point = |p: &Value| n.0 * p["x"].as_f64().unwrap() + n.1 * p["y"].as_f64().unwrap();
    segments
        .iter()
        .flat_map(|segment| {
            if segment["kind"] == "line" {
                vec![point(&segment["from"]), point(&segment["to"])]
            } else {
                let (rx, ry) = (
                    segment["radii"]["x"].as_f64().unwrap(),
                    segment["radii"]["y"].as_f64().unwrap(),
                );
                let angle = |r: &Value| r["y"].as_f64().unwrap().atan2(r["x"].as_f64().unwrap());
                let start = angle(&segment["start"]);
                let span = (angle(&segment["end"]) - start).rem_euclid(std::f64::consts::TAU);
                let center = point(&segment["center"]);
                let (ax, ay) = (rx * n.0, ry * n.1);
                let at = |r: &Value| {
                    center + ax * r["x"].as_f64().unwrap() + ay * r["y"].as_f64().unwrap()
                };
                let mut values = vec![at(&segment["start"]), at(&segment["end"])];
                if (ay.atan2(ax) - start).rem_euclid(std::f64::consts::TAU) <= span {
                    values.push(center + ax.hypot(ay));
                }
                values
            }
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

#[test]
fn hull_is_the_front_swept_along_the_shadow_vector() {
    let mut request = baseline();
    request["radii"] = json!({
        "topStart": {"x": 16, "y": 16}, "topEnd": {"x": 4, "y": 4},
        "bottomEnd": {"x": 10, "y": 6}, "bottomStart": {"x": 0, "y": 0}
    });
    for layout in ["ltr", "rtl"] {
        request["environment"]["layoutDirection"] = json!(layout);
        let shadow = resolve(&request)["shadow"].clone();
        let front = resina_resolver::resolve_extruded_contour_source(
            &json!({
                "schemaVersion": "0.1.0", "size": request["size"], "radii": request["radii"],
                "layoutDirection": layout, "offset": {"x": 0, "y": 0}
            })
            .to_string(),
        )
        .unwrap();
        let front = serde_json::to_value(front).unwrap();
        let sweep = (
            shadow["sweep"]["x"].as_f64().unwrap(),
            shadow["sweep"]["y"].as_f64().unwrap(),
        );
        for step in 0..360 {
            let angle = std::f64::consts::TAU * step as f64 / 360.0;
            let n = (angle.cos(), angle.sin());
            let hull = support(shadow["hull"]["segments"].as_array().unwrap(), n);
            let expected = support(front["segments"].as_array().unwrap(), n)
                + (n.0 * sweep.0 + n.1 * sweep.1).max(0.0);
            assert!((hull - expected).abs() < 1e-9, "{layout} {step}");
        }
    }
}

#[test]
fn penumbra_is_outward_monotone_and_absent_without_capability() {
    let mut request = baseline();
    request["environment"]["rendererCapabilities"]["advancedShadow"] = json!(true);
    let distances: Vec<f64> = (-40..=40).map(|step| step as f64 / 20.0).collect();
    request["distanceSamples"] = json!(distances);
    let enhanced = resolve(&request);
    let band = enhanced["shadow"]["bandColor"].clone();
    let width = enhanced["shadow"]["penumbraWidth"].as_f64().unwrap();
    assert_eq!(width, 1.0);
    let mut previous = f64::INFINITY;
    for (distance, sample) in distances
        .iter()
        .zip(enhanced["samples"].as_array().unwrap())
    {
        let weight = sample["weight"].as_f64().unwrap();
        assert!(weight <= previous);
        previous = weight;
        if *distance <= 0.0 {
            assert_eq!(weight, 1.0);
            assert_eq!(sample["color"], band);
        }
        if *distance >= width {
            assert_eq!(weight, 0.0);
            assert_eq!(
                sample["color"]["components"],
                request["adjacentColor"]["components"]
            );
        }
    }
    request["environment"]["rendererCapabilities"]["advancedShadow"] = json!(false);
    let tier0 = resolve(&request);
    assert_eq!(tier0["shadow"]["penumbraWidth"], json!(0.0));
    assert_eq!(tier0["edge"], enhanced["edge"]);
    assert_eq!(tier0["shadow"]["hull"], enhanced["shadow"]["hull"]);
}

fn pigments(theme: &str) -> serde_json::Map<String, Value> {
    let source = match theme {
        "light" => include_str!("../../../../tokens/themes/light.json"),
        _ => include_str!("../../../../tokens/themes/dark.json"),
    };
    let document: Value = serde_json::from_str(source).unwrap();
    let mut out = serde_json::Map::new();
    fn walk(node: &Value, path: String, out: &mut serde_json::Map<String, Value>) {
        if let Some(value) = node.get("$value") {
            out.insert(path, value["components"].clone());
            return;
        }
        for (key, child) in node.as_object().unwrap() {
            if !key.starts_with('$') {
                let path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                walk(child, path, out);
            }
        }
    }
    walk(
        &document["tokenResolver"]["sets"]["theme"]["sources"][0]["pigment"],
        String::new(),
        &mut out,
    );
    out
}

#[test]
fn reference_profiles_are_perceivable_and_guarded_on_authored_surfaces() {
    let color =
        |components: &Value| json!({"colorSpace": "srgb", "components": components, "alpha": 1});
    for (theme, profiles) in [
        (
            "light",
            include_str!("../../../../definitions/shadow-light.json"),
        ),
        (
            "dark",
            include_str!("../../../../definitions/shadow-dark.json"),
        ),
    ] {
        let pigment = pigments(theme);
        let mut request = baseline();
        request["profiles"] = serde_json::from_str(profiles).unwrap();
        request["edge"]["outline"] = color(&pigment["outlineBase"]);
        request["edge"]["outlineStrong"] = color(&pigment["outlineStrong"]);
        request["focus"]["color"] = color(&pigment["focus"]);
        for role in ["raised", "floating", "overlay", "modal"] {
            request["elevation"] = json!(role);
            for surface in [
                "surface.base",
                "surface.low",
                "surface.high",
                "chromeOpaque",
                "selection",
            ] {
                request["adjacentColor"] = color(&pigment[surface]);
                let result = resolve(&request);
                let difference = result["shadow"]["deltaEOk"].as_f64().unwrap();
                assert!(difference >= 0.04, "{theme} {role} {surface}: {difference}");
                let edge = result["edge"]["colorRole"].as_str().unwrap();
                let strong =
                    theme == "light" && (surface == "surface.low" || surface == "selection");
                assert_eq!(edge == "outline.strong", strong, "{theme} {role} {surface}");
            }
        }
    }
}

#[test]
fn reduced_transparency_and_quality_leave_the_shadow_unchanged() {
    let mut request = baseline();
    request["environment"]["rendererCapabilities"]["advancedShadow"] = json!(true);
    let standard = resolve(&request);
    request["environment"]["accessibilityPreferences"]["reducedTransparency"] = json!(true);
    request["environment"]["accessibilityPreferences"]["reducedMotion"] = json!(true);
    request["environment"]["qualityPolicy"] = json!("economy");
    assert_eq!(resolve(&request), standard);
    request["environment"]["accessibilityPreferences"]["highContrast"] = json!(true);
    assert_eq!(resolve(&request)["shadow"]["penumbraWidth"], json!(0.0));
}

#[test]
fn focus_falls_back_to_outline_strong_beside_the_band() {
    let mut request = baseline();
    request["focus"]["minimumContrast"] = json!(4.8);
    let result = resolve(&request);
    assert_eq!(result["focus"]["colorRole"], "outline.strong");
    assert_eq!(result["focus"]["fallbackApplied"], true);
    assert_eq!(
        result["focus"]["color"]["components"],
        request["edge"]["outlineStrong"]["components"]
    );
    let ratio = result["focus"]["contrastRatio"].as_f64().unwrap();
    assert!((4.8..5.0).contains(&ratio), "{ratio}");
    assert_eq!(result["edge"]["colorRole"], "outline");
    request["edge"]["outlineStrong"] = request["edge"]["outline"].clone();
    let error = resolve_elevation_shadow_source(&request.to_string()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("focus contrast over the shadow range is insufficient")
    );
}

#[test]
fn guard_failures_depend_on_the_band() {
    for (pointer, label) in [
        ("/edge/minimumContrast", "edge"),
        ("/focus/minimumContrast", "focus"),
    ] {
        let mut request = baseline();
        *request.pointer_mut(pointer).unwrap() = json!(5.5);
        let error = resolve_elevation_shadow_source(&request.to_string()).unwrap_err();
        assert!(
            error.to_string().contains(&format!(
                "{label} contrast over the shadow range is insufficient"
            )),
            "{error}"
        );
        request["depthScale"] = json!(0);
        let result = resolve(&request);
        assert!(result["shadow"].is_null());
        let guard = &result[label];
        assert_eq!(guard["colorRole"], "outline.strong", "{label}");
        assert!(guard["contrastRatio"].as_f64().unwrap() >= 5.5, "{label}");
    }
}
