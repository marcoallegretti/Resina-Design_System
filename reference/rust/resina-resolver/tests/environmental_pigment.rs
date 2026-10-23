use resina_environment::EnvironmentSnapshot;
use resina_model::ColorRole;
use resina_resolver::{
    ADAPTABLE_ROLES, AdaptedThemeError, PigmentAdaptationError, adapt_pigment_request_source,
    compile_theme_source_with_sources, legibility_guard_pairs,
};
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeMap,
    io::Write,
    process::{Command, Output, Stdio},
};

const VECTORS: &str =
    include_str!("../../../../conformance/themes/environmental-pigment-vectors.json");
const PROFILE: &str = include_str!("../../../../definitions/environmental-pigment-profile.json");
const FOUNDATION: &str = include_str!("../../../../tokens/foundation.json");
const LIGHT: &str = include_str!("../../../../tokens/themes/light.json");
const DARK: &str = include_str!("../../../../tokens/themes/dark.json");
const ENVIRONMENT: &str = include_str!("../../../../conformance/headless/valid-request.json");
const TOLERANCE: f64 = 1e-9;

fn merge(target: &mut Map<String, Value>, source: &Map<String, Value>) {
    for (name, value) in source {
        match (target.get_mut(name), value) {
            (Some(Value::Object(existing)), Value::Object(group))
                if !group.contains_key("$value") =>
            {
                merge(existing, group)
            }
            _ => {
                target.insert(name.clone(), value.clone());
            }
        }
    }
}

fn embedded(theme: &mut Value) {
    let resolver = theme
        .as_object_mut()
        .unwrap()
        .remove("tokenResolver")
        .unwrap();
    theme.as_object_mut().unwrap().remove("tokenInput");
    let mut tokens = Map::new();
    merge(
        &mut tokens,
        serde_json::from_str::<Value>(FOUNDATION)
            .unwrap()
            .as_object()
            .unwrap(),
    );
    for source in resolver["sets"]["theme"]["sources"].as_array().unwrap() {
        merge(&mut tokens, source.as_object().unwrap());
    }
    theme["tokens"] = Value::Object(tokens);
}

fn build_request(case: &Value) -> String {
    if let Some(document) = case.get("requestDocument") {
        return document.to_string();
    }
    let mut templates = Map::new();
    for (name, text) in [("light", LIGHT), ("dark", DARK)] {
        let mut theme: Value = serde_json::from_str(text).unwrap();
        if case["templateVariant"] == "embeddedTokens" {
            embedded(&mut theme);
        }
        for change in case["templateChanges"].as_array().into_iter().flatten() {
            if change["theme"] == name {
                *theme.pointer_mut(change["path"].as_str().unwrap()).unwrap() =
                    change["value"].clone();
            }
        }
        templates.insert(name.into(), json!(theme.to_string()));
    }
    let mut request = json!({
        "schemaVersion": "0.1.0",
        "profile": serde_json::from_str::<Value>(PROFILE).unwrap(),
        "accessibilityPreferences": case.get("accessibilityPreferences").cloned().unwrap_or(json!({
            "reducedMotion": false, "reducedTransparency": false, "highContrast": false
        })),
        "templates": templates,
        "externalSources": {"foundation.json": FOUNDATION}
    });
    if let Some(observation) = case.get("observation") {
        request["observation"] = observation.clone();
    }
    for change in case["requestChanges"].as_array().into_iter().flatten() {
        *request
            .pointer_mut(change["path"].as_str().unwrap())
            .unwrap() = change["value"].clone();
    }
    for pointer in case["requestOmit"].as_array().into_iter().flatten() {
        let pointer = pointer.as_str().unwrap();
        let (parent, member) = pointer.rsplit_once('/').unwrap();
        let parent = if parent.is_empty() {
            &mut request
        } else {
            request.pointer_mut(parent).unwrap()
        };
        assert!(parent.as_object_mut().unwrap().remove(member).is_some());
    }
    request.to_string()
}

fn close(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(
                actual.keys().collect::<Vec<_>>(),
                expected.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (name, value) in expected {
                close(&actual[name], value, &format!("{path}/{name}"));
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                close(actual, expected, &format!("{path}/{index}"));
            }
        }
        (Value::Number(actual), Value::Number(expected)) => {
            let difference = (actual.as_f64().unwrap() - expected.as_f64().unwrap()).abs();
            assert!(difference <= TOLERANCE, "{path}: {actual} != {expected}");
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

fn role_key(role: ColorRole) -> String {
    serde_json::to_value(role)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}

fn external() -> BTreeMap<String, String> {
    BTreeMap::from([("foundation.json".to_owned(), FOUNDATION.to_owned())])
}

#[test]
fn adaptation_vectors_match_the_reference() {
    let cases: Vec<Value> = serde_json::from_str(VECTORS).unwrap();
    let environment: Value = serde_json::from_str(ENVIRONMENT).unwrap();
    let environment: EnvironmentSnapshot =
        serde_json::from_value(environment["environment"].clone()).unwrap();
    let mut valid = 0;
    for case in &cases {
        let name = case["name"].as_str().unwrap();
        let request = build_request(case);
        let result = adapt_pigment_request_source(&request);
        if case["outcome"] == "invalid" {
            let error = result.expect_err(name);
            let themes: Vec<_> = match &error {
                PigmentAdaptationError::Themes(errors) => {
                    errors.iter().map(|(theme, _)| *theme).collect()
                }
                _ => Vec::new(),
            };
            let expected: Vec<_> = case["failingThemes"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|theme| theme.as_str().unwrap())
                .collect();
            assert_eq!(themes, expected, "{name}: {error}");
            continue;
        }
        let adaptation = result.unwrap_or_else(|error| panic!("{name}: {error}"));
        let mut actual = serde_json::to_value(&adaptation).unwrap();
        for theme in ["light", "dark"] {
            let source = actual["themes"][theme]
                .as_object_mut()
                .unwrap()
                .remove("source")
                .unwrap();
            let adapted = adaptation.theme(theme).unwrap();
            let request: Value = serde_json::from_str(&request).unwrap();
            let template: Value =
                serde_json::from_str(request["templates"][theme].as_str().unwrap()).unwrap();
            let compiled = compile_theme_source_with_sources(&source.to_string(), &external())
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            for role in ADAPTABLE_ROLES {
                let key = role_key(role);
                let binding = &source["colorAssignments"]["roles"][&key];
                match adapted.role(role) {
                    Some((_, components)) => {
                        assert_eq!(binding, &json!(format!("adaptedColor.{key}")), "{name}");
                        assert_eq!(compiled.color_fallbacks()[&role].components(), components);
                        assert!(components.iter().all(|value| (0.0..=1.0).contains(value)));
                    }
                    None => assert_eq!(
                        binding, &template["colorAssignments"]["roles"][&key],
                        "{name}"
                    ),
                }
            }
            for (role, path) in source["colorAssignments"]["roles"].as_object().unwrap() {
                if !ADAPTABLE_ROLES
                    .iter()
                    .any(|adapted| role_key(*adapted) == *role)
                {
                    assert_eq!(path, &template["colorAssignments"]["roles"][role], "{name}");
                }
            }
            assert_eq!(
                source["opaqueColorAssignments"], template["opaqueColorAssignments"],
                "{name}"
            );
            if !adaptation.adaptation().applied() {
                assert_eq!(source, template, "{name}");
            }
            compiled.resolve(&environment).unwrap();
        }
        actual.as_object_mut().unwrap().remove("schemaVersion");
        close(&actual, &case["expected"], name);
        valid += 1;
    }
    assert_eq!(valid, 29);
}

#[test]
fn adapted_themes_publish_every_guard_pair_and_every_frost_bound() {
    let request = json!({
        "schemaVersion": "0.1.0",
        "profile": serde_json::from_str::<Value>(PROFILE).unwrap(),
        "observation": {
            "color": {"colorSpace": "srgb", "components": [0.98, 0.97, 0.93]},
            "range": {
                "lower": {"colorSpace": "srgb", "components": [0.95, 0.94, 0.90]},
                "upper": {"colorSpace": "srgb", "components": [1.0, 1.0, 1.0]}
            }
        },
        "accessibilityPreferences": {
            "reducedMotion": false, "reducedTransparency": false, "highContrast": false
        },
        "templates": {"light": LIGHT, "dark": DARK},
        "externalSources": {"foundation.json": FOUNDATION}
    });
    let adaptation = adapt_pigment_request_source(&request.to_string()).unwrap();
    let pairs = legibility_guard_pairs();
    assert_eq!(pairs.len(), 40);
    let frost: Vec<_> = pairs
        .iter()
        .filter(|(_, background, _)| *background == ColorRole::SurfaceChrome)
        .collect();
    assert_eq!(frost.len(), 9);
    for theme in ["light", "dark"] {
        let adapted = adaptation.theme(theme).unwrap();
        assert_eq!(adapted.legibility_guard().len(), pairs.len());
        assert!(adapted.legibility_guard().iter().all(|pair| pair.passed()));
        let bounds = adapted.frost_certification().unwrap();
        assert_eq!(bounds.len(), frost.len());
        for (bound, (foreground, _, minimum)) in bounds.iter().zip(&frost) {
            assert_eq!(
                (bound.foreground(), bound.minimum_contrast()),
                (*foreground, *minimum)
            );
            assert_eq!(bound.passed(), bound.contrast_bound() >= *minimum);
        }
        assert_eq!(
            adapted.role(ColorRole::SurfaceChrome).is_some(),
            bounds.iter().all(|bound| bound.passed())
        );
    }
    assert!(
        adaptation
            .theme("light")
            .unwrap()
            .role(ColorRole::SurfaceChrome)
            .is_some()
    );
    assert!(
        adaptation
            .theme("dark")
            .unwrap()
            .role(ColorRole::SurfaceChrome)
            .is_none()
    );
}

#[test]
fn every_observation_selects_inside_the_authored_ranges() {
    let profile: Value = serde_json::from_str(PROFILE).unwrap();
    for hue in (0..360).step_by(15) {
        for chroma in [0.0, 0.015, 0.03, 0.12, 0.35] {
            let color =
                resina_color::gamut_map_oklch_to_srgb([0.65, chroma, f64::from(hue)]).unwrap();
            let request = json!({
                "schemaVersion": "0.1.0",
                "profile": profile,
                "observation": {"color": {"colorSpace": "srgb", "components": color}},
                "accessibilityPreferences": {
                    "reducedMotion": false, "reducedTransparency": false, "highContrast": false
                },
                "templates": {"light": LIGHT, "dark": DARK},
                "externalSources": {"foundation.json": FOUNDATION}
            });
            let adaptation = adapt_pigment_request_source(&request.to_string())
                .unwrap_or_else(|error| panic!("hue {hue} chroma {chroma}: {error}"));
            for theme in ["light", "dark"] {
                let adapted = adaptation.theme(theme).unwrap();
                assert!(adapted.legibility_guard().iter().all(|pair| pair.passed()));
                for role in ADAPTABLE_ROLES {
                    let Some((target, components)) = adapted.role(role) else {
                        continue;
                    };
                    let mapped = resina_color::srgb_to_oklch(components).unwrap();
                    assert!(
                        (mapped[0] - target[0]).abs() <= 0.0201,
                        "{theme} {role:?} lightness left the gamut mapping bound"
                    );
                    let ranges = &profile["themes"][theme][role_key(role)];
                    let (low, high) = (
                        ranges["chroma"]["min"].as_f64().unwrap(),
                        ranges["chroma"]["max"].as_f64().unwrap(),
                    );
                    assert!((low..=high).contains(&target[1]), "{theme} {role:?}");
                }
            }
        }
    }
}

#[test]
fn template_role_errors_name_the_role() {
    let mut dark: Value = serde_json::from_str(DARK).unwrap();
    dark["opaqueColorAssignments"]["roles"]["surface.low"] = json!("pigment.surface.low");
    let request = json!({
        "schemaVersion": "0.1.0",
        "profile": serde_json::from_str::<Value>(PROFILE).unwrap(),
        "accessibilityPreferences": {
            "reducedMotion": false, "reducedTransparency": false, "highContrast": false
        },
        "templates": {"light": LIGHT, "dark": dark.to_string()},
        "externalSources": {"foundation.json": FOUNDATION}
    });
    let error = adapt_pigment_request_source(&request.to_string()).unwrap_err();
    assert!(matches!(
        &error,
        PigmentAdaptationError::Themes(errors)
            if matches!(errors.as_slice(), [("dark", AdaptedThemeError::OpaqueOverride(ColorRole::SurfaceLow))])
    ));
    assert!(
        error
            .to_string()
            .contains("dark theme: template role surface.low")
    );
}

fn run_stdin(source: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resina-pigment-adapt"))
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn command_publishes_results_or_diagnostics_only() {
    let request = json!({
        "schemaVersion": "0.1.0",
        "profile": serde_json::from_str::<Value>(PROFILE).unwrap(),
        "observation": {"color": {"colorSpace": "srgb", "components": [0.86, 0.72, 0.48]}},
        "accessibilityPreferences": {
            "reducedMotion": false, "reducedTransparency": false, "highContrast": false
        },
        "templates": {"light": LIGHT, "dark": DARK},
        "externalSources": {"foundation.json": FOUNDATION}
    });
    let output = run_stdin(&request.to_string());
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["adaptation"]["applied"], true);
    for component in ["1.0000001", "1e400", "NaN", "-0.1"] {
        let invalid = request.to_string().replace("0.86", component);
        let output = run_stdin(&invalid);
        assert_eq!(output.status.code(), Some(1), "{component}");
        assert!(output.stdout.is_empty(), "{component}");
        assert!(!output.stderr.is_empty(), "{component}");
    }
    let usage = Command::new(env!("CARGO_BIN_EXE_resina-pigment-adapt"))
        .output()
        .unwrap();
    assert_eq!(usage.status.code(), Some(2));
}
