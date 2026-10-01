use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    process::{Command, Output, Stdio},
};

const SOURCE: &str = r#"{"z":{"$type":"number","$value":2},"a":{"$type":"number","$value":"{z}"}}"#;

fn run_stdin(source: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resina-token-resolve"))
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
fn resolves_file_and_stdin_to_the_same_deterministic_json() {
    let path =
        std::env::temp_dir().join(format!("resina-token-resolve-{}.json", std::process::id()));
    fs::write(&path, SOURCE).unwrap();
    let file = Command::new(env!("CARGO_BIN_EXE_resina-token-resolve"))
        .arg(&path)
        .output()
        .unwrap();
    fs::remove_file(&path).unwrap();
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
    assert_eq!(file.stdout, stdin.stdout);
    assert!(file.stderr.is_empty());
    assert!(stdin.stderr.is_empty());
    let value: Value = serde_json::from_slice(&file.stdout).unwrap();
    assert_eq!(
        value,
        json!({
            "a": {"token_type": "number", "value": 2},
            "z": {"token_type": "number", "value": 2}
        })
    );
    let output = String::from_utf8(file.stdout).unwrap();
    assert!(output.find("\"a\"").unwrap() < output.find("\"z\"").unwrap());
}

#[test]
fn invalid_source_has_no_partial_output() {
    for (source, error) in [
        (
            r#"{"token":{"$type":"number","$value":1,"$value":2}}"#,
            "duplicate JSON member at #/token/$value",
        ),
        (r#"{"plain":{"$value":2}}"#, "MissingType at #/plain"),
    ] {
        let result = run_stdin(source);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(error),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn usage_error_has_distinct_exit_status() {
    let result = Command::new(env!("CARGO_BIN_EXE_resina-token-resolve"))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Usage:"));
}
