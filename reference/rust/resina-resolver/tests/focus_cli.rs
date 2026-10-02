use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    process::{Command, Output, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

fn request() -> String {
    let resolution: Value = serde_json::from_str(include_str!(
        "../../../../conformance/headless/valid-request.json"
    ))
    .unwrap();
    let vectors: Value = serde_json::from_str(include_str!(
        "../../../../conformance/surfaces/binding-vectors.json"
    ))
    .unwrap();
    json!({
        "schemaVersion": "0.1.0",
        "scenario": {
            "schemaVersion": "0.4.0",
            "resolution": resolution,
            "surface": vectors[0]["document"]
        },
        "surroundingColor": {"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1}
    })
    .to_string()
}

fn run_stdin(source: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resina-focus-indicator"))
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
fn file_and_stdin_publish_the_same_focus_indicator() {
    let source = request();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("resina-focus-{}-{unique}.json", std::process::id()));
    fs::write(&path, &source).unwrap();
    let file = Command::new(env!("CARGO_BIN_EXE_resina-focus-indicator"))
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
    assert_eq!(output["colorRole"], "focus");
    assert_eq!(
        output["binding"]["states"]["states"],
        json!(["rest", "focused"])
    );
}

#[test]
fn usage_error_has_distinct_exit_status() {
    let output = Command::new(env!("CARGO_BIN_EXE_resina-focus-indicator"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
}
