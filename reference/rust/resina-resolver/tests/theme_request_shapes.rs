use resina_resolver::{
    resolve_focus_ir_source, resolve_opaque_surface_source, resolve_surface_paint_source,
    resolve_theme_request_source,
};
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

const VECTORS: &str = include_str!("../../../../conformance/themes/request-vectors.json");

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
            json!({
                "schemaVersion": "0.1.0",
                "themeSource": include_str!("../../../../conformance/themes/valid-source.json"),
                "externalSources": {},
                "environment": headless["environment"]
            }),
            "",
        ),
        (
            env!("CARGO_BIN_EXE_resina-focus-ir"),
            serde_json::from_str(include_str!(
                "../../../../conformance/ir/focus-ir-request.json"
            ))
            .unwrap(),
            "/theme",
        ),
        (
            env!("CARGO_BIN_EXE_resina-opaque-surface"),
            opaque.clone(),
            "/theme",
        ),
        (
            env!("CARGO_BIN_EXE_resina-surface-paint"),
            json!({
                "schemaVersion": "0.1.0",
                "body": opaque,
                "surroundingColor": {"colorSpace": "srgb", "components": [0, 0, 0], "alpha": 1}
            }),
            "/body/theme",
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

fn invalid_requests() -> Vec<(&'static str, String, String)> {
    let vectors: Vec<Value> = serde_json::from_str(VECTORS).unwrap();
    assert_eq!(vectors.len(), 7);
    let mut invalid = Vec::new();
    for (binary, baseline, pointer) in requests() {
        let source = baseline.to_string();
        let expected = resolve(binary, &source).unwrap();
        let result = run(binary, &source);
        assert_eq!(result.status.code(), Some(0));
        assert!(result.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<Value>(&result.stdout).unwrap(),
            expected
        );
        for vector in &vectors {
            let mut request = baseline.clone();
            let mut document = vector["document"].clone();
            if let Some(array) = document.as_array_mut().filter(|array| array.len() == 4) {
                let theme = baseline.pointer(pointer).unwrap();
                array[1] = theme["themeSource"].clone();
                array[2] = theme["externalSources"].clone();
                array[3] = theme["environment"].clone();
            }
            *request.pointer_mut(pointer).unwrap() = document;
            invalid.push((
                binary,
                request.to_string(),
                vector["error"].as_str().unwrap().into(),
            ));
        }
    }
    invalid
}

#[test]
fn all_theme_request_consumers_reject_non_object_roots() {
    for (binary, source, diagnostic) in invalid_requests() {
        let error = resolve(binary, &source).unwrap_err();
        assert!(error.contains(&diagnostic), "{binary}: {error}");
    }
}

#[test]
fn all_theme_request_commands_reject_without_partial_output() {
    for (binary, source, diagnostic) in invalid_requests() {
        let result = run(binary, &source);
        assert_eq!(result.status.code(), Some(1), "{binary}: {source}");
        assert!(result.stdout.is_empty(), "{binary}: {source}");
        assert!(String::from_utf8_lossy(&result.stderr).contains(&diagnostic));
    }
}

#[test]
fn object_version_and_duplicate_checks_precede_compilation() {
    for (binary, baseline, pointer) in requests() {
        let mut request = baseline;
        *request.pointer_mut(pointer).unwrap() = json!({"schemaVersion": "0.2.0"});
        let source = request.to_string();
        assert!(
            resolve(binary, &source)
                .unwrap_err()
                .contains("schemaVersion must be 0.1.0")
        );
        let output = run(binary, &source);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("schemaVersion must be 0.1.0"));
        let duplicate = source.replace(
            "\"schemaVersion\":\"0.2.0\"",
            "\"schemaVersion\":\"0.2.0\",\"schemaVersion\":\"0.1.0\"",
        );
        for source in [
            duplicate.clone(),
            duplicate.replace("schemaVersion\":\"0.1.0", "schema\\u0056ersion\":\"0.1.0"),
        ] {
            assert!(
                resolve(binary, &source)
                    .unwrap_err()
                    .contains("duplicate JSON member")
            );
            let output = run(binary, &source);
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("duplicate JSON member"));
        }
    }
}
