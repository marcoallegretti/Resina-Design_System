use resina_environment::EnvironmentSnapshot;
use resina_model::ColorRole;
use resina_resolver::{
    DERIVED_COLOR_ROLES, DerivedThemeError, ThemeDerivationError,
    compile_theme_source_with_sources, derive_theme_request_source, legibility_guard_pairs,
};
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeMap,
    io::Write,
    process::{Command, Output, Stdio},
};

const VECTORS: &str = include_str!("../../../../conformance/themes/derivation-vectors.json");
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
        "templates": templates,
        "externalSources": {"foundation.json": FOUNDATION}
    });
    if let Some(seed) = case.get("seed") {
        request["seed"] = seed.clone();
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

#[test]
fn derivation_vectors_match_the_reference() {
    let cases: Vec<Value> = serde_json::from_str(VECTORS).unwrap();
    let environment: Value = serde_json::from_str(ENVIRONMENT).unwrap();
    let environment: EnvironmentSnapshot =
        serde_json::from_value(environment["environment"].clone()).unwrap();
    let external = BTreeMap::from([("foundation.json".to_owned(), FOUNDATION.to_owned())]);
    let mut valid = 0;
    for case in &cases {
        let name = case["name"].as_str().unwrap();
        let result = derive_theme_request_source(&build_request(case));
        if case["outcome"] == "invalid" {
            let error = result.expect_err(name);
            if let Some(failing) = case.get("failingThemes") {
                let ThemeDerivationError::Themes(errors) = &error else {
                    panic!("{name}: {error}");
                };
                let themes: Vec<_> = errors.iter().map(|(theme, _)| *theme).collect();
                let expected: Vec<_> = failing
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|theme| theme.as_str().unwrap())
                    .collect();
                assert_eq!(themes, expected, "{name}");
            }
            continue;
        }
        let derivation = result.unwrap_or_else(|error| panic!("{name}: {error}"));
        let mut actual = serde_json::to_value(&derivation).unwrap();
        for theme in ["light", "dark"] {
            let source = actual["themes"][theme]
                .as_object_mut()
                .unwrap()
                .remove("source")
                .unwrap();
            for role in DERIVED_COLOR_ROLES {
                let role = serde_json::to_value(role).unwrap();
                let role = role.as_str().unwrap();
                assert_eq!(
                    source["colorAssignments"]["roles"][role],
                    json!(format!("derivedColor.{role}")),
                    "{name}"
                );
            }
            let compiled = compile_theme_source_with_sources(&source.to_string(), &external)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            let derived = derivation.theme(theme).unwrap();
            for role in DERIVED_COLOR_ROLES {
                let (_, components) = derived.role(role).unwrap();
                assert_eq!(
                    compiled.opaque_color_fallbacks()[&role].components(),
                    components
                );
                assert!(components.iter().all(|value| (0.0..=1.0).contains(value)));
            }
            compiled.resolve(&environment).unwrap();
        }
        actual.as_object_mut().unwrap().remove("schemaVersion");
        close(&actual, &case["expected"], name);
        valid += 1;
    }
    assert_eq!(valid, 25);
}

#[test]
fn derived_themes_publish_every_guard_pair() {
    let request = json!({
        "schemaVersion": "0.1.0",
        "seed": {"colorSpace": "srgb", "components": [0.0, 0.0, 1.0]},
        "templates": {"light": LIGHT, "dark": DARK},
        "externalSources": {"foundation.json": FOUNDATION}
    });
    let derivation = derive_theme_request_source(&request.to_string()).unwrap();
    let pairs = legibility_guard_pairs();
    assert_eq!(pairs.len(), 40);
    for theme in ["light", "dark"] {
        let guard = derivation.theme(theme).unwrap().legibility_guard();
        assert_eq!(guard.len(), pairs.len());
        for (pair, (foreground, background, minimum)) in guard.iter().zip(&pairs) {
            assert_eq!(
                (
                    pair.foreground(),
                    pair.background(),
                    pair.minimum_contrast()
                ),
                (*foreground, *background, *minimum)
            );
            assert!(pair.passed());
        }
    }
    assert!(pairs.contains(&(ColorRole::Focus, ColorRole::SurfaceChrome, 3.0)));
}

#[test]
fn out_of_gamut_targets_are_mapped_by_reducing_chroma() {
    let request = json!({
        "schemaVersion": "0.1.0",
        "seed": {"colorSpace": "srgb", "components": [0.0, 0.0, 1.0]},
        "templates": {"light": LIGHT, "dark": DARK},
        "externalSources": {"foundation.json": FOUNDATION}
    });
    let derivation = derive_theme_request_source(&request.to_string()).unwrap();
    let (target, components) = derivation
        .theme("dark")
        .unwrap()
        .role(ColorRole::Focus)
        .unwrap();
    let [_, chroma, _] = resina_color::srgb_to_oklch(components).unwrap();
    assert!(chroma < target[1] - 0.001);
}

#[test]
fn template_role_errors_name_the_role() {
    let mut dark: Value = serde_json::from_str(DARK).unwrap();
    dark["opaqueColorAssignments"]["roles"]["selection"] = json!("pigment.selection");
    let request = json!({
        "schemaVersion": "0.1.0",
        "seed": {"colorSpace": "srgb", "components": [1.0, 0.0, 0.0]},
        "templates": {"light": LIGHT, "dark": dark.to_string()},
        "externalSources": {"foundation.json": FOUNDATION}
    });
    let error = derive_theme_request_source(&request.to_string()).unwrap_err();
    assert!(matches!(
        &error,
        ThemeDerivationError::Themes(errors)
            if matches!(errors.as_slice(), [("dark", DerivedThemeError::OpaqueOverride(ColorRole::Selection))])
    ));
    assert!(
        error
            .to_string()
            .contains("dark theme: template role selection")
    );
}

fn run_stdin(source: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resina-theme-derive"))
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
        "seed": {"colorSpace": "srgb", "components": [1.0, 0.5, 0.0]},
        "templates": {"light": LIGHT, "dark": DARK},
        "externalSources": {"foundation.json": FOUNDATION}
    });
    let output = run_stdin(&request.to_string());
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["schemaVersion"], "0.1.0");
    let mut invalid = request.clone();
    invalid["seed"]["components"][2] = json!(1.0000001);
    let output = run_stdin(&invalid.to_string());
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid seed"));
    let usage = Command::new(env!("CARGO_BIN_EXE_resina-theme-derive"))
        .output()
        .unwrap();
    assert_eq!(usage.status.code(), Some(2));
}
