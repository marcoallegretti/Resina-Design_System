use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

const MIB: usize = 1024 * 1024;
const COMMANDS: &[(&str, &str, usize)] = &[
    (
        "resina-activation",
        env!("CARGO_BIN_EXE_resina-activation"),
        MIB,
    ),
    (
        "resina-command-motion",
        env!("CARGO_BIN_EXE_resina-command-motion"),
        MIB,
    ),
    (
        "resina-command-paint",
        env!("CARGO_BIN_EXE_resina-command-paint"),
        MIB,
    ),
    (
        "resina-command-states",
        env!("CARGO_BIN_EXE_resina-command-states"),
        MIB,
    ),
    (
        "resina-edge-contrast",
        env!("CARGO_BIN_EXE_resina-edge-contrast"),
        MIB,
    ),
    (
        "resina-elevation-depth",
        env!("CARGO_BIN_EXE_resina-elevation-depth"),
        MIB,
    ),
    (
        "resina-extruded-contour",
        env!("CARGO_BIN_EXE_resina-extruded-contour"),
        MIB,
    ),
    (
        "resina-focus-indicator",
        env!("CARGO_BIN_EXE_resina-focus-indicator"),
        MIB,
    ),
    (
        "resina-focus-ir",
        env!("CARGO_BIN_EXE_resina-focus-ir"),
        MIB,
    ),
    (
        "resina-focus-traversal",
        env!("CARGO_BIN_EXE_resina-focus-traversal"),
        MIB,
    ),
    (
        "resina-frost-legibility",
        env!("CARGO_BIN_EXE_resina-frost-legibility"),
        MIB,
    ),
    (
        "resina-frost-surface-readability",
        env!("CARGO_BIN_EXE_resina-frost-surface-readability"),
        MIB,
    ),
    (
        "resina-hit-region",
        env!("CARGO_BIN_EXE_resina-hit-region"),
        MIB,
    ),
    (
        "resina-inset-contour",
        env!("CARGO_BIN_EXE_resina-inset-contour"),
        MIB,
    ),
    (
        "resina-key-light",
        env!("CARGO_BIN_EXE_resina-key-light"),
        MIB,
    ),
    (
        "resina-opaque-pigment",
        env!("CARGO_BIN_EXE_resina-opaque-pigment"),
        MIB,
    ),
    (
        "resina-opaque-surface",
        env!("CARGO_BIN_EXE_resina-opaque-surface"),
        MIB,
    ),
    (
        "resina-shape-fallback",
        env!("CARGO_BIN_EXE_resina-shape-fallback"),
        MIB,
    ),
    (
        "resina-slider-adjustment",
        env!("CARGO_BIN_EXE_resina-slider-adjustment"),
        MIB,
    ),
    (
        "resina-slider-layout",
        env!("CARGO_BIN_EXE_resina-slider-layout"),
        MIB,
    ),
    (
        "resina-slider-position",
        env!("CARGO_BIN_EXE_resina-slider-position"),
        MIB,
    ),
    (
        "resina-slider-value",
        env!("CARGO_BIN_EXE_resina-slider-value"),
        MIB,
    ),
    (
        "resina-surface-paint",
        env!("CARGO_BIN_EXE_resina-surface-paint"),
        MIB,
    ),
    (
        "resina-surface-readability",
        env!("CARGO_BIN_EXE_resina-surface-readability"),
        MIB,
    ),
    (
        "resina-theme-resolve",
        env!("CARGO_BIN_EXE_resina-theme-resolve"),
        64 * MIB,
    ),
    (
        "resina-toggle-activation",
        env!("CARGO_BIN_EXE_resina-toggle-activation"),
        MIB,
    ),
    (
        "resina-toggle-layout",
        env!("CARGO_BIN_EXE_resina-toggle-layout"),
        MIB,
    ),
    (
        "resina-toggle-part-motion",
        env!("CARGO_BIN_EXE_resina-toggle-part-motion"),
        MIB,
    ),
    (
        "resina-toggle-part-paint",
        env!("CARGO_BIN_EXE_resina-toggle-part-paint"),
        MIB,
    ),
    (
        "resina-toggle-states",
        env!("CARGO_BIN_EXE_resina-toggle-states"),
        MIB,
    ),
];

struct RequestFile(PathBuf);
impl RequestFile {
    fn new(source: &[u8]) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "resina-bounded-cli-é-{}-{unique}.json",
            std::process::id()
        ));
        fs::write(&path, source).unwrap();
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for RequestFile {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}

fn stdin(binary: &str, source: &[u8]) -> Output {
    let mut child = Command::new(binary)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(source).unwrap();
    child.wait_with_output().unwrap()
}
fn failure(output: &Output, name: &str) {
    assert_eq!(output.status.code(), Some(1), "{name}: {output:?}");
    assert!(output.stdout.is_empty(), "{name}: {output:?}");
    assert!(!output.stderr.is_empty(), "{name}: {output:?}");
}

#[test]
fn every_bounded_command_preserves_help_and_usage() {
    for &(name, binary, _) in COMMANDS {
        let usage =
            format!("Usage: {name} <path|->\nPass - to read a UTF-8 JSON request from stdin.\n");
        let help = Command::new(binary).arg("--help").output().unwrap();
        assert_eq!(help.status.code(), Some(0), "{name}");
        assert_eq!(help.stdout, usage.as_bytes(), "{name}");
        assert!(help.stderr.is_empty(), "{name}");
        for args in [vec![], vec!["-", "extra"], vec!["--help", "extra"]] {
            let output = Command::new(binary).args(args).output().unwrap();
            assert_eq!(output.status.code(), Some(2), "{name}");
            assert!(output.stdout.is_empty(), "{name}");
            assert_eq!(output.stderr, usage.as_bytes(), "{name}");
        }
    }
}

#[test]
fn every_bounded_command_rejects_bad_source_and_missing_files() {
    let missing = std::env::temp_dir().join(format!("resina-missing-{}.json", std::process::id()));
    assert!(!missing.exists());
    for &(name, binary, _) in COMMANDS {
        let output = Command::new(binary).arg(&missing).output().unwrap();
        failure(&output, name);
        for source in [b"{".as_slice(), b"\xff".as_slice()] {
            let file = RequestFile::new(source);
            let file_output = Command::new(binary).arg(file.path()).output().unwrap();
            let stdin_output = stdin(binary, source);
            failure(&file_output, name);
            failure(&stdin_output, name);
            assert_eq!(file_output.stderr, stdin_output.stderr, "{name}");
            if source == b"\xff" {
                assert!(
                    String::from_utf8_lossy(&stdin_output.stderr).contains("invalid utf-8"),
                    "{name}"
                );
            }
        }
    }
}

#[test]
fn every_bounded_command_preserves_its_exact_byte_limit() {
    for &(name, binary, limit) in COMMANDS {
        let mut source = b"{}".to_vec();
        source.resize(limit, b' ');
        let file = RequestFile::new(&source);
        for output in [
            Command::new(binary).arg(file.path()).output().unwrap(),
            stdin(binary, &source),
        ] {
            failure(&output, name);
            assert!(
                !String::from_utf8_lossy(&output.stderr).contains("request size limit exceeded"),
                "{name}"
            );
        }
        source.push(b' ');
        fs::write(file.path(), &source).unwrap();
        for output in [
            Command::new(binary).arg(file.path()).output().unwrap(),
            stdin(binary, &source),
        ] {
            failure(&output, name);
            assert_eq!(output.stderr, b"request size limit exceeded\n", "{name}");
        }
    }
}

#[test]
fn valid_requests_preserve_file_stdin_and_pretty_json_publication() {
    let position = env!("CARGO_BIN_EXE_resina-slider-position");
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-position-protocol-cases.json"
    ))
    .unwrap();
    let source = serde_json::to_vec(&cases[0]["request"]).unwrap();
    let file = RequestFile::new(&source);
    for output in [
        stdin(position, &source),
        Command::new(position).arg(file.path()).output().unwrap(),
    ] {
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
            cases[0]["expected"]
        );
        assert!(output.stdout.ends_with(b"\n"));
    }

    let layout = env!("CARGO_BIN_EXE_resina-slider-layout");
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/slider-layout-cases.json"
    ))
    .unwrap();
    let mut source = cases["cases"][0]["input"].clone();
    source["schemaVersion"] = serde_json::json!("0.1.0");
    let source = serde_json::to_vec(&source).unwrap();
    let file = RequestFile::new(&source);
    for output in [
        stdin(layout, &source),
        Command::new(layout).arg(file.path()).output().unwrap(),
    ] {
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
            cases["cases"][0]["expected"]
        );
        assert!(output.stdout.ends_with(b"\n"));
    }

    let adjustment = env!("CARGO_BIN_EXE_resina-slider-adjustment");
    let adjustment_source = br#"{"schemaVersion":"0.1.0","current":{"schemaVersion":"0.1.0","minimum":0,"maximum":100,"value":25},"enabled":true,"readOnly":false,"adjustment":{"kind":"increase","amount":10}}"#;
    let file = RequestFile::new(adjustment_source);
    let expected = serde_json::to_value(
        resina_resolver::resolve_slider_adjustment_source(
            std::str::from_utf8(adjustment_source).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    for output in [
        stdin(adjustment, adjustment_source),
        Command::new(adjustment).arg(file.path()).output().unwrap(),
    ] {
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
            expected
        );
        assert!(output.stdout.ends_with(b"\n"));
    }

    let slider = env!("CARGO_BIN_EXE_resina-slider-value");
    let source = br#"{"schemaVersion":"0.1.0","minimum":0,"maximum":1,"value":0.5}"#;
    let expected =
        b"{\n  \"schemaVersion\": \"0.1.0\",\n  \"minimum\": 0.0,\n  \"maximum\": 1.0,\n  \"value\": 0.5,\n  \"progress\": 0.5\n}\n";
    let file = RequestFile::new(source);
    for output in [
        stdin(slider, source),
        Command::new(slider).arg(file.path()).output().unwrap(),
    ] {
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected);
    }

    let headless: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/headless/valid-request.json"
    ))
    .unwrap();
    let theme = include_str!("../../../../conformance/themes/valid-source.json");
    let mut source = serde_json::json!({
        "schemaVersion": "0.1.0", "themeSource": theme,
        "externalSources": {}, "environment": headless["environment"]
    })
    .to_string()
    .into_bytes();
    source.resize(MIB + 1, b' ');
    let binary = env!("CARGO_BIN_EXE_resina-theme-resolve");
    let file = RequestFile::new(&source);
    let from_file = Command::new(binary).arg(file.path()).output().unwrap();
    let from_stdin = stdin(binary, &source);
    assert!(from_file.status.success(), "{from_file:?}");
    assert!(from_stdin.status.success(), "{from_stdin:?}");
    assert!(from_file.stderr.is_empty() && from_stdin.stderr.is_empty());
    assert_eq!(from_file.stdout, from_stdin.stdout);
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/headless/expected-resolution.json"
    ))
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&from_file.stdout).unwrap(),
        expected
    );
    assert!(from_file.stdout.ends_with(b"\n"));
    assert!(String::from_utf8_lossy(&from_file.stdout).contains("\n  \"schemaVersion\": "));
}
