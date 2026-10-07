use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    process::{Command, Output, Stdio},
};

const THEME: &str = include_str!("../../../../conformance/themes/valid-source.json");
const HEADLESS: &str = include_str!("../../../../conformance/headless/valid-request.json");
const EXPECTED: &str = include_str!("../../../../conformance/headless/expected-resolution.json");

#[test]
fn non_object_frost_pigment_fails_compilation_without_publishing() {
    let vectors: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/materials/frost-pigment-vectors.json"
    ))
    .unwrap();
    let invalid: Vec<_> = vectors
        .iter()
        .filter(|vector| {
            vector["name"]
                .as_str()
                .unwrap()
                .starts_with("non-object Frost pigment ")
        })
        .collect();
    assert_eq!(invalid.len(), 6);
    for vector in invalid {
        let mut theme: Value = serde_json::from_str(THEME).unwrap();
        theme["frostPigment"] = vector["document"].clone();
        let source = theme.to_string();
        assert!(matches!(
            resina_resolver::compile_theme_source(&source),
            Err(resina_resolver::ThemeCompilationError::Source(_))
        ));
        let mut input: Value = serde_json::from_str(&request()).unwrap();
        input["themeSource"] = json!(source);
        let input = input.to_string();
        assert!(matches!(
            resina_resolver::resolve_theme_request_source(&input),
            Err(resina_resolver::ThemeResolutionError::Compile(
                resina_resolver::ThemeCompilationError::Source(_)
            ))
        ));
        let output = run_stdin(&input);
        assert_eq!(output.status.code(), Some(1), "{}", vector["name"]);
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("Frost pigment object")
        );
    }
}

fn request() -> String {
    let headless: Value = serde_json::from_str(HEADLESS).unwrap();
    json!({
        "schemaVersion": "0.1.0",
        "themeSource": THEME,
        "externalSources": {},
        "environment": headless["environment"]
    })
    .to_string()
}

fn run_stdin(source: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resina-theme-resolve"))
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
fn file_and_stdin_produce_the_same_semantic_result() {
    let source = request();
    let path =
        std::env::temp_dir().join(format!("resina-theme-resolve-{}.json", std::process::id()));
    fs::write(&path, &source).unwrap();
    let file = Command::new(env!("CARGO_BIN_EXE_resina-theme-resolve"))
        .arg(&path)
        .output()
        .unwrap();
    fs::remove_file(&path).unwrap();
    let stdin = run_stdin(&source);
    assert!(
        file.status.success(),
        "{}",
        String::from_utf8_lossy(&file.stderr)
    );
    assert!(
        stdin.status.success(),
        "{}",
        String::from_utf8_lossy(&stdin.stderr)
    );
    assert!(file.stderr.is_empty());
    assert!(stdin.stderr.is_empty());
    assert_eq!(file.stdout, stdin.stdout);
    assert_eq!(
        serde_json::from_slice::<Value>(&file.stdout).unwrap(),
        serde_json::from_str::<Value>(EXPECTED).unwrap()
    );
}

#[test]
fn invalid_theme_and_environment_publish_no_partial_result() {
    let duplicate_request = request().replacen(
        "\"schemaVersion\":\"0.1.0\"",
        "\"schemaVersion\":\"0.1.0\",\"schemaVersion\":\"0.1.0\"",
        1,
    );
    let mut duplicate: Value = serde_json::from_str(&request()).unwrap();
    duplicate["themeSource"] = json!(THEME.replacen(
        "\"schemaVersion\": \"0.1.0\"",
        "\"schemaVersion\": \"0.1.0\", \"schemaVersion\": \"0.1.0\"",
        1,
    ));
    let mut invalid_environment: Value = serde_json::from_str(&request()).unwrap();
    invalid_environment["environment"]["inputCapabilities"] = json!(["keyboard", "keyboard"]);
    for (source, diagnostic) in [
        (duplicate_request, "duplicate JSON member"),
        (duplicate.to_string(), "duplicate JSON member"),
        (invalid_environment.to_string(), "duplicate"),
    ] {
        let result = run_stdin(&source);
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(diagnostic),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn usage_error_has_distinct_exit_status() {
    let result = Command::new(env!("CARGO_BIN_EXE_resina-theme-resolve"))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Usage:"));
}
