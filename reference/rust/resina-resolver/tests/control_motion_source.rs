use resina_resolver::{
    CommandMotionChannel, CommandMotionChannels, resolve_command_motion_source,
    resolve_toggle_part_motion_source,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fmt::{Debug, Display},
    io::Write,
    process::{Command, Stdio},
};

fn record<T: DeserializeOwned + PartialEq + Debug>(baseline: Value, fields: &[&str]) {
    let expected: T = serde_json::from_value(baseline.clone()).unwrap();
    let positional = Value::Array(
        fields
            .iter()
            .map(|field| baseline[*field].clone())
            .collect(),
    );
    for invalid in [
        positional,
        json!([]),
        Value::Null,
        json!(true),
        json!(1),
        json!("record"),
    ] {
        assert!(serde_json::from_str::<T>(&invalid.to_string()).is_err());
        assert!(serde_json::from_value::<T>(invalid).is_err());
    }
    let members: Vec<_> = fields
        .iter()
        .map(|field| format!("\"{field}\":{}", baseline[*field]))
        .collect();
    for (index, field) in fields.iter().enumerate() {
        let escaped = format!(
            "\"\\u{:04x}{}\":{}",
            field.as_bytes()[0],
            &field[1..],
            baseline[*field]
        );
        let mut changed = members.clone();
        changed[index] = escaped.clone();
        assert_eq!(
            serde_json::from_str::<T>(&format!("{{{}}}", changed.join(","))).unwrap(),
            expected
        );
        for duplicate in [&members[index], &escaped] {
            changed[index] = format!("{},{}", members[index], duplicate);
            let error = serde_json::from_str::<T>(&format!("{{{}}}", changed.join(",")))
                .err()
                .unwrap();
            assert!(error.to_string().contains("duplicate"), "{error}");
        }
        let mut missing = baseline.clone();
        missing.as_object_mut().unwrap().remove(*field);
        assert!(serde_json::from_value::<T>(missing).is_err());
    }
    let mut unknown = baseline;
    unknown["unknown"] = json!(false);
    assert!(serde_json::from_value::<T>(unknown).is_err());
}

#[test]
fn motion_channel_requires_an_object_without_changing_members() {
    let baseline: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap();
    record::<CommandMotionChannel>(
        baseline["channels"]["bodyMix"].clone(),
        &["dynamics", "initial"],
    );
}

#[test]
fn motion_channels_require_an_object_without_changing_members() {
    let baseline: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap();
    record::<CommandMotionChannels>(baseline["channels"].clone(), &["bodyMix", "depthScale"]);
}

fn value<T: Serialize, E: Display>(result: Result<T, E>) -> Result<Value, String> {
    result
        .map(|result| serde_json::to_value(result).unwrap())
        .map_err(|error| error.to_string())
}

fn resolve(toggle: bool, source: &str) -> Result<Value, String> {
    if toggle {
        value(resolve_toggle_part_motion_source(source))
    } else {
        value(resolve_command_motion_source(source))
    }
}

fn run(toggle: bool, source: &str) -> std::process::Output {
    let binary = if toggle {
        env!("CARGO_BIN_EXE_resina-toggle-part-motion")
    } else {
        env!("CARGO_BIN_EXE_resina-command-motion")
    };
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

#[test]
fn motion_sources_require_channel_objects_before_every_policy() {
    for toggle in [false, true] {
        let fixture = if toggle {
            include_str!("../../../../conformance/ir/toggle-part-motion-request.json")
        } else {
            include_str!("../../../../conformance/ir/command-motion-request.json")
        };
        let baseline: Value = serde_json::from_str(fixture).unwrap();
        for policy in ["spring", "castImmediate", "reducedMotion"] {
            let mut valid = baseline.clone();
            let theme = &mut valid["surface"]["body"]["theme"];
            theme["environment"]["accessibilityPreferences"]["reducedMotion"] =
                json!(policy == "reducedMotion");
            if policy == "castImmediate" {
                let mut document: Value =
                    serde_json::from_str(theme["themeSource"].as_str().unwrap()).unwrap();
                document["materialAssignments"]["control"]["interactive"] = json!("cast");
                theme["themeSource"] = json!(document.to_string());
            }
            let canonical = resolve(toggle, &valid.to_string()).unwrap();
            assert_eq!(canonical["policy"], json!(policy));
            let output = run(toggle, &valid.to_string());
            assert_eq!(output.status.code(), Some(0));
            assert!(output.stderr.is_empty());
            assert_eq!(
                serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                canonical
            );
            for (pointer, fields) in [
                ("/channels", &["bodyMix", "depthScale"] as &[&str]),
                ("/channels/bodyMix", &["dynamics", "initial"]),
                ("/channels/depthScale", &["dynamics", "initial"]),
            ] {
                let object = valid.pointer(pointer).unwrap();
                let positional =
                    Value::Array(fields.iter().map(|field| object[*field].clone()).collect());
                for shape in [
                    positional,
                    json!([]),
                    Value::Null,
                    json!(true),
                    json!(1),
                    json!("record"),
                ] {
                    let mut invalid = valid.clone();
                    *invalid.pointer_mut(pointer).unwrap() = shape;
                    let source = invalid.to_string();
                    assert!(resolve(toggle, &source).is_err(), "{pointer} {policy}");
                    let output = run(toggle, &source);
                    assert_eq!(output.status.code(), Some(1), "{pointer} {policy}");
                    assert!(output.stdout.is_empty());
                    assert!(!output.stderr.is_empty());
                }
            }
        }
    }
}
