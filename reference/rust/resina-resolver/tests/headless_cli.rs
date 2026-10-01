use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

const SOURCE: &str = include_str!("../../../../conformance/headless/valid-request.json");
const EXPECTED: &str = include_str!("../../../../conformance/headless/expected-resolution.json");

fn run_stdin(source: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resina-headless"))
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
fn file_and_stdin_resolve_to_the_same_complete_output() {
    let file = Command::new(env!("CARGO_BIN_EXE_resina-headless"))
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../conformance/headless/valid-request.json"
        ))
        .output()
        .unwrap();
    let stdin = run_stdin(SOURCE);
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
fn invalid_inputs_produce_diagnostics_without_partial_output() {
    let duplicate = SOURCE.replacen(
        "\"schemaVersion\": \"0.1.0\"",
        "\"schemaVersion\": \"0.1.0\", \"schemaVersion\": \"0.1.0\"",
        1,
    );
    let mut invalid_bindings: Value = serde_json::from_str(SOURCE).unwrap();
    invalid_bindings["colorAssignments"]["roles"]["focus"] = json!("missing.color");
    invalid_bindings["spatialAssignments"]["roles"]["space.page"] = json!("missing.space");
    invalid_bindings["typographyAssignments"]["roles"]["body"]["fontSize"] = json!("missing.type");
    let mut invalid_tokens: Value = serde_json::from_str(SOURCE).unwrap();
    invalid_tokens["tokens"]["type"]["size"]["$value"]["unit"] = json!("em");
    let mut invalid_request: Value = serde_json::from_str(SOURCE).unwrap();
    invalid_request["schemaVersion"] = json!("0.2.0");
    for (source, diagnostics) in [
        (duplicate, vec!["duplicate JSON member"]),
        (
            invalid_bindings.to_string(),
            vec![
                "color: MissingToken",
                "space: MissingToken",
                "typography: MissingToken",
            ],
        ),
        (invalid_tokens.to_string(), vec!["token resolution failed"]),
        (
            invalid_request.to_string(),
            vec!["schemaVersion must be 0.1.0"],
        ),
    ] {
        let output = run_stdin(&source);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut previous = 0;
        for diagnostic in diagnostics {
            let index = stderr
                .find(diagnostic)
                .unwrap_or_else(|| panic!("{stderr}"));
            assert!(index >= previous, "{stderr}");
            previous = index + diagnostic.len();
        }
    }
}

#[test]
fn accessibility_and_input_capabilities_change_only_their_outputs() {
    let mut request: Value = serde_json::from_str(SOURCE).unwrap();
    request["environment"]["accessibilityPreferences"]["highContrast"] = json!(true);
    request["environment"]["inputCapabilities"] = json!(["keyboard"]);
    let output = run_stdin(&request.to_string());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
    let mut expected: Value = serde_json::from_str(EXPECTED).unwrap();
    expected["frostRepresentation"] = json!("opaqueDimensional");
    expected["minimumHitTarget"] = json!({"minimumWidth":24,"minimumHeight":24});
    assert_eq!(actual, expected);
}

#[test]
fn usage_error_has_distinct_exit_status() {
    let output = Command::new(env!("CARGO_BIN_EXE_resina-headless"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
}
