use resina_resolver::{ElevationDepthError, resolve_elevation_depth_source};
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../../conformance/elevation/backend-cases.json"
    ))
    .unwrap()
}

fn run(source: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resina-elevation-depth"))
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
fn public_depth_requests_validate_before_publishing() {
    let cases = cases();
    assert_eq!(
        cases
            .iter()
            .filter(|c| c["name"]
                .as_str()
                .unwrap()
                .starts_with("elevation request "))
            .count(),
        16
    );
    for case in cases {
        let source = case["request"].to_string();
        let result = resolve_elevation_depth_source(&source);
        let output = run(&source);
        if let Some(expected) = case.get("expected") {
            assert_eq!(serde_json::to_value(result.unwrap()).unwrap(), *expected);
            assert_eq!(output.status.code(), Some(0));
            assert!(output.stderr.is_empty());
            assert_eq!(
                serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                *expected
            );
        } else {
            let diagnostic = case["errorContains"].as_str().unwrap();
            let error = result.unwrap_err();
            assert!(
                error.to_string().contains(diagnostic),
                "{}: {error}",
                case["name"]
            );
            if case["name"]
                .as_str()
                .unwrap()
                .starts_with("elevation request shape ")
            {
                assert!(matches!(error, ElevationDepthError::Request(_)));
            }
            assert_eq!(output.status.code(), Some(1), "{}", case["name"]);
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
        }
    }
}

#[test]
fn raw_request_duplicates_and_escaped_names_preserve_validation() {
    let original = cases()[0]["request"].clone();
    let source = original.to_string();
    let expected = serde_json::to_value(resolve_elevation_depth_source(&source).unwrap()).unwrap();
    for field in ["schemaVersion", "tokens", "assignments"] {
        let member = format!("\"{field}\":{}", original[field]);
        let escaped = format!("\\u{:04x}{}", field.as_bytes()[0], &field[1..]);
        let escaped_member = format!("\"{escaped}\":{}", original[field]);
        for duplicate in [&member, &escaped_member] {
            let changed = source.replacen(&member, &format!("{member},{duplicate}"), 1);
            assert!(changed.len() > source.len());
            let error = resolve_elevation_depth_source(&changed).unwrap_err();
            assert!(matches!(error, ElevationDepthError::Parse(_)));
            assert!(error.to_string().contains("duplicate"));
            let output = run(&changed);
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("duplicate"));
        }
        let changed = source.replacen(&member, &escaped_member, 1);
        assert_ne!(source, changed);
        assert_eq!(
            serde_json::to_value(resolve_elevation_depth_source(&changed).unwrap()).unwrap(),
            expected
        );
        let output = run(&changed);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            expected
        );
    }
}

#[test]
fn unsupported_version_fails_before_token_resolution() {
    let original = cases()[0]["request"].clone();
    for version in ["9", "", "0.1.0 "] {
        let mut request = original.clone();
        request["schemaVersion"] = version.into();
        request["tokens"] = Value::Null;
        assert!(matches!(
            resolve_elevation_depth_source(&request.to_string()),
            Err(ElevationDepthError::UnsupportedVersion)
        ));
        let output = run(&request.to_string());
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("schemaVersion"));
    }
}
