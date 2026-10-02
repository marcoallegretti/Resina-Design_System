use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    process::{Command, Output, Stdio},
};

const RESOLVER: &str = r##"{
  "version": "2025.10",
  "resolutionOrder": [{
    "type": "set",
    "name": "base",
    "sources": [{"$ref": "base.json"}]
  }]
}"##;

fn run_stdin(source: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resina-token-compose"))
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

fn valid_request() -> String {
    json!({
        "schemaVersion": "0.1.0",
        "resolver": RESOLVER,
        "input": "{}",
        "externalSources": {
            "base.json": "{\"a\":{\"$type\":\"number\",\"$value\":2}}"
        }
    })
    .to_string()
}

#[test]
fn file_and_stdin_compose_to_identical_tokens() {
    let source = valid_request();
    let path =
        std::env::temp_dir().join(format!("resina-token-compose-{}.json", std::process::id()));
    fs::write(&path, &source).unwrap();
    let file = Command::new(env!("CARGO_BIN_EXE_resina-token-compose"))
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
    let output: Value = serde_json::from_slice(&file.stdout).unwrap();
    assert_eq!(output, json!({"a": {"token_type": "number", "value": 2}}));
}

#[test]
fn malformed_request_and_failed_composition_publish_no_tokens() {
    let duplicate = valid_request().replacen(
        "\"schemaVersion\":\"0.1.0\"",
        "\"schemaVersion\":\"0.1.0\",\"schemaVersion\":\"0.1.0\"",
        1,
    );
    let mut missing: Value = serde_json::from_str(&valid_request()).unwrap();
    missing["externalSources"] = json!({});
    for (source, diagnostic) in [
        (duplicate, "duplicate JSON member"),
        (missing.to_string(), "missing source"),
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
    let result = Command::new(env!("CARGO_BIN_EXE_resina-token-compose"))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Usage:"));
}
