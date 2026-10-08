use resina_resolver::{
    resolve_command_motion_source, resolve_command_paint_source, resolve_toggle_part_motion_source,
    resolve_toggle_part_paint_source,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fmt::Display,
    io::Write,
    process::{Command, Stdio},
};

fn value<T: Serialize, E: Display>(result: Result<T, E>) -> Result<Value, String> {
    result
        .map(|result| serde_json::to_value(result).unwrap())
        .map_err(|error| error.to_string())
}

fn resolve(binary: &str, source: &str) -> Result<Value, String> {
    match binary {
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

fn reject(binary: &str, source: &str, diagnostic: &str) {
    let error = resolve(binary, source).expect_err("malformed request published IR");
    assert!(error.contains(diagnostic), "{binary}: {error}");
    let output = run(binary, source);
    assert_eq!(output.status.code(), Some(1), "{binary}");
    assert!(output.stdout.is_empty(), "{binary}");
    assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
}

fn consumers() -> Vec<(&'static str, Value, &'static [&'static str])> {
    vec![
        (
            env!("CARGO_BIN_EXE_resina-command-paint"),
            serde_json::from_str(include_str!(
                "../../../../conformance/ir/command-paint-request.json"
            ))
            .unwrap(),
            &["schemaVersion", "surface", "commandAppearance"],
        ),
        (
            env!("CARGO_BIN_EXE_resina-command-motion"),
            serde_json::from_str(include_str!(
                "../../../../conformance/ir/command-motion-request.json"
            ))
            .unwrap(),
            &[
                "schemaVersion",
                "surface",
                "commandAppearance",
                "channels",
                "time",
            ],
        ),
        (
            env!("CARGO_BIN_EXE_resina-toggle-part-paint"),
            serde_json::from_str(include_str!(
                "../../../../conformance/ir/toggle-part-paint-request.json"
            ))
            .unwrap(),
            &[
                "schemaVersion",
                "part",
                "surface",
                "checkedColorRole",
                "interactionAppearance",
            ],
        ),
        (
            env!("CARGO_BIN_EXE_resina-toggle-part-motion"),
            serde_json::from_str(include_str!(
                "../../../../conformance/ir/toggle-part-motion-request.json"
            ))
            .unwrap(),
            &[
                "schemaVersion",
                "surface",
                "interactionAppearance",
                "part",
                "checkedColorRole",
                "channels",
                "time",
            ],
        ),
    ]
}

#[test]
fn positional_control_requests_fail_before_publication() {
    for (binary, baseline, members) in consumers() {
        let positional = Value::Array(
            members
                .iter()
                .map(|member| baseline[*member].clone())
                .collect(),
        );
        for invalid in [
            positional,
            json!([]),
            Value::Null,
            json!(true),
            json!(1),
            json!("control"),
        ] {
            reject(binary, &invalid.to_string(), "request object");
        }
    }
}

#[test]
fn object_encoded_toggle_names_fail_before_publication() {
    for (binary, baseline, _) in consumers()
        .into_iter()
        .filter(|(_, request, _)| request.get("part").is_some())
    {
        for member in ["part", "checkedColorRole"] {
            let mut invalid = baseline.clone();
            invalid[member] = json!({baseline[member].as_str().unwrap():null});
            reject(binary, &invalid.to_string(), "a string");
        }
    }
}

#[test]
fn canonical_and_escaped_members_preserve_output_and_duplicate_diagnostics() {
    for (binary, baseline, names) in consumers() {
        let members: Vec<String> = names
            .iter()
            .map(|member| format!("\"{member}\":{}", baseline[*member]))
            .collect();
        let source = format!("{{{}}}", members.join(","));
        let expected = resolve(binary, &source).unwrap();
        for (index, member) in names.iter().enumerate() {
            let original = format!("\"{member}\":{}", baseline[*member]);
            let escaped_name = format!("\\u{:04x}{}", member.as_bytes()[0], &member[1..]);
            let escaped_value = if matches!(*member, "part" | "checkedColorRole") {
                let name = baseline[*member].as_str().unwrap();
                format!("\"\\u{:04x}{}\"", name.as_bytes()[0], &name[1..])
            } else {
                baseline[*member].to_string()
            };
            let escaped = format!("\"{escaped_name}\":{escaped_value}");
            let mut changed_members = members.clone();
            changed_members[index] = escaped.clone();
            let changed = format!("{{{}}}", changed_members.join(","));
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
                reject(
                    binary,
                    &format!("{{{}}}", changed_members.join(",")),
                    "duplicate",
                );
            }
        }
    }
}

#[test]
fn required_and_unknown_members_keep_their_contract() {
    for (binary, baseline, names) in consumers() {
        for member in names {
            let mut missing = baseline.clone();
            missing.as_object_mut().unwrap().remove(*member);
            reject(binary, &missing.to_string(), "missing field");
        }
        let mut unknown = baseline;
        unknown["unexpected"] = json!(true);
        reject(binary, &unknown.to_string(), "unknown field");
    }
}
