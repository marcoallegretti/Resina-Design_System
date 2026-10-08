use resina_model::StateSet;
use resina_resolver::{resolve_command_states_source, resolve_toggle_states_source};
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn resolve(toggle: bool, source: &str) -> Result<StateSet, String> {
    if toggle {
        resolve_toggle_states_source(source).map_err(|error| error.to_string())
    } else {
        resolve_command_states_source(source).map_err(|error| error.to_string())
    }
}

fn run(toggle: bool, source: &str) -> std::process::Output {
    let binary = if toggle {
        env!("CARGO_BIN_EXE_resina-toggle-states")
    } else {
        env!("CARGO_BIN_EXE_resina-command-states")
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

fn cases(toggle: bool) -> Vec<Value> {
    serde_json::from_str(if toggle {
        include_str!("../../../../conformance/interaction/toggle-states-cases.json")
    } else {
        include_str!("../../../../conformance/interaction/command-states-cases.json")
    })
    .unwrap()
}

fn fields(toggle: bool) -> &'static [&'static str] {
    if toggle {
        &["schemaVersion", "activation", "hovered", "checked"]
    } else {
        &["schemaVersion", "activation", "hovered"]
    }
}

fn success(toggle: bool, source: &str, expected: &Value) {
    assert_eq!(
        serde_json::to_value(resolve(toggle, source).unwrap()).unwrap(),
        *expected
    );
    let output = run(toggle, source);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        *expected
    );
}

fn failure(toggle: bool, source: &str) {
    assert!(resolve(toggle, source).is_err(), "{source}");
    let output = run(toggle, source);
    assert_eq!(output.status.code(), Some(1), "{source}");
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn state_sources_require_object_requests_for_every_projection() {
    for toggle in [false, true] {
        let valid: Vec<_> = cases(toggle)
            .into_iter()
            .filter(|case| case.get("expected").is_some())
            .collect();
        assert!(!valid.is_empty());
        for case in valid {
            let request = &case["request"];
            success(toggle, &request.to_string(), &case["expected"]);
            let positional = Value::Array(
                fields(toggle)
                    .iter()
                    .map(|field| request[*field].clone())
                    .collect(),
            );
            for shape in [
                positional,
                json!([]),
                Value::Null,
                json!(true),
                json!(1),
                json!("request"),
            ] {
                failure(toggle, &shape.to_string());
            }
        }
    }
}

#[test]
fn state_request_members_remain_strict_and_support_decoded_names() {
    for toggle in [false, true] {
        let baseline = &cases(toggle)[0];
        let request = &baseline["request"];
        let members: Vec<_> = fields(toggle)
            .iter()
            .map(|field| format!("\"{field}\":{}", request[*field]))
            .collect();
        let mut reordered = members.clone();
        reordered.reverse();
        success(
            toggle,
            &format!("{{{}}}", reordered.join(",")),
            &baseline["expected"],
        );
        for (index, field) in fields(toggle).iter().enumerate() {
            let escaped = format!(
                "\"\\u{:04x}{}\":{}",
                field.as_bytes()[0],
                &field[1..],
                request[*field]
            );
            let mut changed = members.clone();
            changed[index] = escaped.clone();
            success(
                toggle,
                &format!("{{{}}}", changed.join(",")),
                &baseline["expected"],
            );
            for duplicate in [&members[index], &escaped] {
                changed[index] = format!("{},{}", members[index], duplicate);
                failure(toggle, &format!("{{{}}}", changed.join(",")));
            }
            let mut missing = request.clone();
            missing.as_object_mut().unwrap().remove(*field);
            failure(toggle, &missing.to_string());
        }
        let mut unknown = request.clone();
        unknown["unknown"] = json!(false);
        failure(toggle, &unknown.to_string());
    }
}
