use resina_resolver::{KeyLightError, resolve_key_light_source};
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn alternate_json_shapes_fail_before_source_and_cli_publication() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/lighting/key-light-vectors.json"
    ))
    .unwrap();
    let cases: Vec<_> = cases
        .iter()
        .filter(|case| {
            case["name"]
                .as_str()
                .unwrap()
                .starts_with("invalid JSON shape")
        })
        .collect();
    assert_eq!(cases.len(), 5);
    let binary = env!("CARGO_BIN_EXE_resina-key-light");
    for case in cases {
        let source = case["request"].to_string();
        assert!(matches!(
            resolve_key_light_source(&source),
            Err(KeyLightError::InvalidShape(_))
        ));
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
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1), "{}", case["name"]);
        assert!(output.stdout.is_empty(), "{}", case["name"]);
        assert!(!output.stderr.is_empty(), "{}", case["name"]);
    }
}

#[test]
fn escaped_members_and_decoded_duplicate_detection_remain_strict() {
    let source = r#"{"schemaVersion":"0.1.0","keyLight":{"schemaVersion":"0.1.0","direction":{"x":0,"y":-1}},"depth":2,"normals":[{"x":0,"y":-1}]}"#;
    let result = resolved_light(source);
    for (member, escaped) in [
        ("keyLight", "\\u006beyLight"),
        ("direction", "\\u0064irection"),
        ("normals", "\\u006eormals"),
        ("x", "\\u0078"),
    ] {
        let escaped = source.replace(&format!("\"{member}\""), &format!("\"{escaped}\""));
        assert_eq!(resolved_light(&escaped), result);
    }
    let duplicate = source.replacen("\"x\":0", "\"x\":0,\"\\u0078\":0", 1);
    assert!(
        resolve_key_light_source(&duplicate)
            .unwrap_err()
            .to_string()
            .contains("duplicate JSON member")
    );
}

fn resolved_light(source: &str) -> Value {
    serde_json::to_value(resolve_key_light_source(source).unwrap()).unwrap()
}
