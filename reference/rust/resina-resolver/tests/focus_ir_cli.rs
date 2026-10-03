use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

const REQUEST: &str = include_str!("../../../../conformance/ir/focus-ir-request.json");

#[test]
fn file_and_stdin_publish_identical_focus_ir() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "resina-focus-ir-{}-{unique}.json",
        std::process::id()
    ));
    fs::write(&path, REQUEST).unwrap();
    let file = Command::new(env!("CARGO_BIN_EXE_resina-focus-ir"))
        .arg(&path)
        .output()
        .unwrap();
    fs::remove_file(&path).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_resina-focus-ir"))
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
        .write_all(REQUEST.as_bytes())
        .unwrap();
    let stdin = child.wait_with_output().unwrap();
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
    assert!(file.stderr.is_empty() && stdin.stderr.is_empty());
    assert_eq!(file.stdout, stdin.stdout);
    let output: serde_json::Value = serde_json::from_slice(&file.stdout).unwrap();
    assert_eq!(
        output["indicator"]["binding"]["states"]["states"],
        serde_json::json!(["focused", "selected"])
    );
    assert_eq!(
        output["geometry"]["outer"]["offset"],
        serde_json::json!({"x":-4.0,"y":-4.0})
    );
}

#[test]
fn usage_has_distinct_status_and_no_partial_result() {
    let output = Command::new(env!("CARGO_BIN_EXE_resina-focus-ir"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
}
