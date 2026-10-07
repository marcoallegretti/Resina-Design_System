use resina_environment::EnvironmentSnapshot;
use resina_resolver::{
    ThemeCompilationError, compile_theme_source, compile_theme_source_with_sources,
    resolve_focus_ir_source, resolve_opaque_surface_source, resolve_surface_paint_source,
    resolve_theme_request_source,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Write,
    process::{Command, Output, Stdio},
};

const VECTORS: &str = include_str!("../../../../conformance/themes/source-vectors.json");
const FIELDS: [&str; 10] = [
    "schemaVersion",
    "tokens",
    "tokenResolver",
    "tokenInput",
    "materialAssignments",
    "frostPigment",
    "colorAssignments",
    "opaqueColorAssignments",
    "spatialAssignments",
    "typographyAssignments",
];

fn vectors() -> Vec<Value> {
    let vectors: Vec<Value> = serde_json::from_str(VECTORS).unwrap();
    assert_eq!(vectors.len(), 9);
    vectors
}

#[test]
fn both_compilation_entries_reject_every_public_non_object_source() {
    for vector in vectors() {
        let source = vector["document"].to_string();
        for result in [
            compile_theme_source(&source),
            compile_theme_source_with_sources(&source, &BTreeMap::new()),
        ] {
            let error = result.err().expect("invalid source must not compile");
            assert!(matches!(error, ThemeCompilationError::Source(_)), "{error}");
            assert!(
                error
                    .to_string()
                    .contains(vector["error"].as_str().unwrap())
            );
        }
    }
}

#[test]
fn positional_payloads_are_otherwise_valid_in_both_authoring_forms() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../../conformance/headless/valid-request.json"
    ))
    .unwrap();
    let environment: EnvironmentSnapshot =
        serde_json::from_value(request["environment"].clone()).unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../conformance/headless/expected-resolution.json"
    ))
    .unwrap();
    for vector in &vectors()[..4] {
        let values = vector["document"].as_array().unwrap();
        assert_eq!(values.len(), FIELDS.len());
        let mut object: serde_json::Map<String, Value> = FIELDS
            .iter()
            .zip(values)
            .filter(|(_, value)| !value.is_null())
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect();
        object.insert("schemaVersion".into(), json!("0.1.0"));
        let source = Value::Object(object).to_string();
        for theme in [
            compile_theme_source(&source).unwrap(),
            compile_theme_source_with_sources(&source, &BTreeMap::new()).unwrap(),
        ] {
            assert_eq!(
                serde_json::to_value(theme.resolve(&environment).unwrap()).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn strict_parsing_and_object_version_diagnostics_remain_first() {
    assert!(matches!(
        compile_theme_source(r#"{"schemaVersion":"0.2.0"}"#),
        Err(ThemeCompilationError::UnsupportedVersion)
    ));
    for source in [
        r#"{"schemaVersion":"0.2.0","schemaVersion":"0.1.0"}"#,
        r#"{"schemaVersion":"0.2.0","schema\u0056ersion":"0.1.0"}"#,
        r#"[{"member":1,"member":2}]"#,
        r#"[{"member":1,"\u006dember":2}]"#,
    ] {
        for result in [
            compile_theme_source(source),
            compile_theme_source_with_sources(source, &BTreeMap::new()),
        ] {
            let error = result.err().unwrap();
            assert!(matches!(error, ThemeCompilationError::Parse(_)), "{error}");
            assert!(error.to_string().contains("duplicate JSON member"));
        }
    }
}

fn requests() -> Vec<(&'static str, Value, &'static str)> {
    let headless: Value = serde_json::from_str(include_str!(
        "../../../../conformance/headless/valid-request.json"
    ))
    .unwrap();
    let opaque: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json"
    ))
    .unwrap();
    vec![
        (
            env!("CARGO_BIN_EXE_resina-theme-resolve"),
            json!({"schemaVersion":"0.1.0", "themeSource": include_str!("../../../../conformance/themes/valid-source.json"), "externalSources":{}, "environment":headless["environment"]}),
            "/themeSource",
        ),
        (
            env!("CARGO_BIN_EXE_resina-focus-ir"),
            serde_json::from_str(include_str!(
                "../../../../conformance/ir/focus-ir-request.json"
            ))
            .unwrap(),
            "/theme/themeSource",
        ),
        (
            env!("CARGO_BIN_EXE_resina-opaque-surface"),
            opaque.clone(),
            "/theme/themeSource",
        ),
        (
            env!("CARGO_BIN_EXE_resina-surface-paint"),
            json!({"schemaVersion":"0.1.0", "body":opaque, "surroundingColor":{"colorSpace":"srgb", "components":[0,0,0], "alpha":1}}),
            "/body/theme/themeSource",
        ),
    ]
}

fn resolve(binary: &str, source: &str) -> Result<Value, String> {
    if binary == env!("CARGO_BIN_EXE_resina-theme-resolve") {
        resolve_theme_request_source(source)
            .map(|result| serde_json::to_value(result).unwrap())
            .map_err(|error| error.to_string())
    } else if binary == env!("CARGO_BIN_EXE_resina-focus-ir") {
        resolve_focus_ir_source(source)
            .map(|result| serde_json::to_value(result).unwrap())
            .map_err(|error| error.to_string())
    } else if binary == env!("CARGO_BIN_EXE_resina-opaque-surface") {
        resolve_opaque_surface_source(source)
            .map(|result| serde_json::to_value(result).unwrap())
            .map_err(|error| error.to_string())
    } else {
        assert_eq!(binary, env!("CARGO_BIN_EXE_resina-surface-paint"));
        resolve_surface_paint_source(source)
            .map(|result| serde_json::to_value(result).unwrap())
            .map_err(|error| error.to_string())
    }
}

fn run(binary: &str, source: &str) -> Output {
    let mut child = Command::new(binary)
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
fn all_source_consumers_reject_without_partial_results() {
    for (binary, baseline, pointer) in requests() {
        let expected = resolve(binary, &baseline.to_string()).unwrap();
        let original: Value =
            serde_json::from_str(baseline.pointer(pointer).unwrap().as_str().unwrap()).unwrap();
        for vector in vectors() {
            let mut request = baseline.clone();
            let mut document = vector["document"].clone();
            if let Some(array) = document
                .as_array_mut()
                .filter(|array| array.len() == FIELDS.len())
            {
                let mut source = original.clone();
                if array[1].is_null() {
                    let tokens = source.as_object_mut().unwrap().remove("tokens").unwrap();
                    source["tokenResolver"] = json!({"version":"2025.10", "resolutionOrder":[{"type":"set", "name":"theme", "sources":[tokens]}]});
                    source["tokenInput"] = json!({});
                }
                *request.pointer_mut(pointer).unwrap() = Value::String(source.to_string());
                assert_eq!(resolve(binary, &request.to_string()).unwrap(), expected);
                let valid = run(binary, &request.to_string());
                assert_eq!(valid.status.code(), Some(0));
                assert!(valid.stderr.is_empty());
                assert_eq!(
                    serde_json::from_slice::<Value>(&valid.stdout).unwrap(),
                    expected
                );
                for (index, field) in FIELDS.iter().enumerate().skip(1) {
                    array[index] = source.get(field).cloned().unwrap_or(Value::Null);
                }
            }
            *request.pointer_mut(pointer).unwrap() = Value::String(document.to_string());
            let source = request.to_string();
            let diagnostic = vector["error"].as_str().unwrap();
            let error = resolve(binary, &source).unwrap_err();
            assert!(error.contains(diagnostic), "{binary}: {error}");
            let invalid = run(binary, &source);
            assert_eq!(
                invalid.status.code(),
                Some(1),
                "{binary}: {}",
                vector["name"]
            );
            assert!(invalid.stdout.is_empty());
            assert!(String::from_utf8_lossy(&invalid.stderr).contains(diagnostic));
        }
    }
}
