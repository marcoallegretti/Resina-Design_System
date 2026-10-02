use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

const RESOLUTION: &str = include_str!("../../../../conformance/headless/valid-request.json");
const VECTORS: &str = include_str!("../../../../conformance/surfaces/binding-vectors.json");

fn run_stdin(source: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resina-surface-bind"))
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

fn scenario() -> Value {
    let vectors: Vec<Value> = serde_json::from_str(VECTORS).unwrap();
    json!({
        "schemaVersion": "0.1.0",
        "resolution": serde_json::from_str::<Value>(RESOLUTION).unwrap(),
        "surface": vectors[0]["document"]
    })
}

#[test]
fn command_publishes_complete_bound_surface() {
    let output = run_stdin(&scenario().to_string());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let vectors: Vec<Value> = serde_json::from_str(VECTORS).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        vectors[0]["expected"]
    );
}

#[test]
fn command_rejects_invalid_scenarios_without_partial_output() {
    let mut missing_role = scenario();
    missing_role["resolution"]["colorAssignments"]["roles"]["focus"] = json!("missing.color");
    let mut invalid_surface = scenario();
    invalid_surface["surface"]["materialRole"] = json!("surface.unknown");
    let mut invalid_version = scenario();
    invalid_version["schemaVersion"] = json!("0.2.0");
    let duplicate = scenario().to_string().replacen(
        "\"schemaVersion\":\"0.1.0\"",
        "\"schemaVersion\":\"0.1.0\",\"schemaVersion\":\"0.1.0\"",
        1,
    );
    for (source, diagnostic) in [
        (missing_role.to_string(), "color: MissingToken"),
        (invalid_surface.to_string(), "unknown variant"),
        (invalid_version.to_string(), "schemaVersion must be 0.1.0"),
        (duplicate, "duplicate JSON member"),
    ] {
        let output = run_stdin(&source);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
    }
}

#[test]
fn usage_error_has_distinct_exit_status() {
    let output = Command::new(env!("CARGO_BIN_EXE_resina-surface-bind"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
}
