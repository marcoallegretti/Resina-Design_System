use resina_resolver::{
    resolve_command_motion_source, resolve_command_paint_source, resolve_opaque_surface_source,
    resolve_surface_paint_source, resolve_toggle_part_motion_source,
    resolve_toggle_part_paint_source,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fmt::Display,
    io::Write,
    process::{Command, Stdio},
};

const MEMBERS: [&str; 10] = [
    "schemaVersion",
    "theme",
    "surface",
    "size",
    "appearance",
    "foregroundRole",
    "postTreatmentBackdrop",
    "adjacentColor",
    "minimumContentContrast",
    "minimumEdgeContrast",
];

fn value<T: Serialize, E: Display>(result: Result<T, E>) -> Result<Value, String> {
    result
        .map(|result| serde_json::to_value(result).unwrap())
        .map_err(|error| error.to_string())
}

fn resolve(binary: &str, source: &str) -> Result<Value, String> {
    match binary {
        env!("CARGO_BIN_EXE_resina-opaque-surface") => value(resolve_opaque_surface_source(source)),
        env!("CARGO_BIN_EXE_resina-surface-paint") => value(resolve_surface_paint_source(source)),
        env!("CARGO_BIN_EXE_resina-command-paint") => value(resolve_command_paint_source(source)),
        env!("CARGO_BIN_EXE_resina-command-motion") => value(resolve_command_motion_source(source)),
        env!("CARGO_BIN_EXE_resina-toggle-part-paint") => {
            value(resolve_toggle_part_paint_source(source))
        }
        b => {
            assert_eq!(b, env!("CARGO_BIN_EXE_resina-toggle-part-motion"));
            value(resolve_toggle_part_motion_source(source))
        }
    }
}

fn run(binary: &str, source: &str) -> std::process::Output {
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
    child.wait_with_output().unwrap()
}

fn consumers() -> Vec<(&'static str, Value, &'static str)> {
    let opaque: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json"
    ))
    .unwrap();
    vec![
        (
            env!("CARGO_BIN_EXE_resina-opaque-surface"),
            opaque.clone(),
            "",
        ),
        (
            env!("CARGO_BIN_EXE_resina-surface-paint"),
            json!({"schemaVersion":"0.1.0", "body":opaque, "surroundingColor":{"colorSpace":"srgb","components":[0,0,0],"alpha":1}}),
            "/body",
        ),
        (
            env!("CARGO_BIN_EXE_resina-command-paint"),
            serde_json::from_str(include_str!(
                "../../../../conformance/ir/command-paint-request.json"
            ))
            .unwrap(),
            "/surface/body",
        ),
        (
            env!("CARGO_BIN_EXE_resina-command-motion"),
            serde_json::from_str(include_str!(
                "../../../../conformance/ir/command-motion-request.json"
            ))
            .unwrap(),
            "/surface/body",
        ),
        (
            env!("CARGO_BIN_EXE_resina-toggle-part-paint"),
            serde_json::from_str(include_str!(
                "../../../../conformance/ir/toggle-part-paint-request.json"
            ))
            .unwrap(),
            "/surface/body",
        ),
        (
            env!("CARGO_BIN_EXE_resina-toggle-part-motion"),
            serde_json::from_str(include_str!(
                "../../../../conformance/ir/toggle-part-motion-request.json"
            ))
            .unwrap(),
            "/surface/body",
        ),
    ]
}

#[test]
fn malformed_opaque_requests_fail_before_publication_in_every_consumer() {
    for (binary, baseline, pointer) in consumers() {
        let body = baseline.pointer(pointer).unwrap();
        let mut tagged = body.clone();
        tagged["foregroundRole"] = json!({body["foregroundRole"].as_str().unwrap():null});
        let positional = Value::Array(MEMBERS.iter().map(|member| body[*member].clone()).collect());
        for (invalid, diagnostic) in [
            (positional, "opaque surface request object"),
            (tagged, "a string"),
        ] {
            let mut request = baseline.clone();
            *request.pointer_mut(pointer).unwrap() = invalid;
            let source = request.to_string();
            let error = resolve(binary, &source).unwrap_err();
            assert!(error.contains(diagnostic), "{binary}: {error}");
            let output = run(binary, &source);
            assert_eq!(output.status.code(), Some(1), "{binary}");
            assert!(output.stdout.is_empty(), "{binary}");
            assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
        }
    }
}

#[test]
fn canonical_and_escaped_members_preserve_output_and_duplicate_diagnostics() {
    for (binary, baseline, pointer) in consumers() {
        let body = baseline.pointer(pointer).unwrap();
        let members: Vec<String> = MEMBERS
            .iter()
            .map(|member| format!("\"{member}\":{}", body[*member]))
            .collect();
        let body_source = format!("{{{}}}", members.join(","));
        let baseline_source = baseline.to_string();
        let source = baseline_source.replace(&body.to_string(), &body_source);
        assert_eq!(baseline_source.matches(&body.to_string()).count(), 1);
        let expected = resolve(binary, &source).unwrap();
        for (index, member) in MEMBERS.into_iter().enumerate() {
            let original = format!("\"{member}\":{}", body[member]);
            let escaped_name = format!("\\u{:04x}{}", member.as_bytes()[0], &member[1..]);
            let mut escaped_value = body[member].to_string();
            if member == "foregroundRole" {
                let role = body[member].as_str().unwrap();
                escaped_value = format!("\"\\u{:04x}{}\"", role.as_bytes()[0], &role[1..]);
            }
            let escaped = format!("\"{escaped_name}\":{escaped_value}");
            let mut changed_members = members.clone();
            changed_members[index] = escaped.clone();
            let changed_body = format!("{{{}}}", changed_members.join(","));
            let changed = source.replace(&body_source, &changed_body);
            assert_ne!(source, changed);
            assert_eq!(resolve(binary, &changed).unwrap(), expected);
            let output = run(binary, &changed);
            assert_eq!(output.status.code(), Some(0));
            assert!(output.stderr.is_empty());
            assert_eq!(
                serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                expected
            );
            for duplicate in [&original, &escaped] {
                changed_members[index] = format!("{original},{duplicate}");
                let changed_body = format!("{{{}}}", changed_members.join(","));
                let changed = source.replace(&body_source, &changed_body);
                assert!(resolve(binary, &changed).unwrap_err().contains("duplicate"));
                let output = run(binary, &changed);
                assert_eq!(output.status.code(), Some(1));
                assert!(output.stdout.is_empty());
                assert!(String::from_utf8_lossy(&output.stderr).contains("duplicate"));
            }
        }
    }
}
