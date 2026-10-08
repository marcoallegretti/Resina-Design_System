use resina_resolver::resolve_toggle_layout_source;
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn run(source: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resina-toggle-layout"))
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

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../../conformance/geometry/toggle-layout-cases.json"
    ))
    .unwrap()
}

fn success(source: &str, expected: &Value) {
    assert_eq!(
        serde_json::to_value(resolve_toggle_layout_source(source).unwrap()).unwrap(),
        *expected
    );
    let output = run(source);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        *expected
    );
}

fn failure(source: &str) {
    assert!(resolve_toggle_layout_source(source).is_err(), "{source}");
    let output = run(source);
    assert_eq!(output.status.code(), Some(1), "{source}");
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn layout_source_requires_objects_and_string_direction_before_allocation() {
    let valid: Vec<_> = cases()
        .into_iter()
        .filter(|case| case.get("expected").is_some())
        .collect();
    assert!(!valid.is_empty());
    for case in valid {
        let request = &case["request"];
        success(&request.to_string(), &case["expected"]);
        let root = Value::Array(
            [
                "schemaVersion",
                "trackSize",
                "thumbSize",
                "insets",
                "layoutDirection",
                "checked",
            ]
            .iter()
            .map(|field| request[*field].clone())
            .collect(),
        );
        let insets = Value::Array(
            ["start", "end", "top", "bottom"]
                .iter()
                .map(|field| request["insets"][*field].clone())
                .collect(),
        );
        let direction = json!({request["layoutDirection"].as_str().unwrap(): null});
        for (pointer, malformed) in [
            ("", root),
            ("/insets", insets),
            ("/layoutDirection", direction),
        ] {
            for shape in [
                malformed,
                json!([]),
                Value::Null,
                json!(true),
                json!(1),
                json!("invalid"),
            ] {
                let mut invalid = request.clone();
                *invalid.pointer_mut(pointer).unwrap() = shape;
                failure(&invalid.to_string());
            }
        }
    }
}

#[test]
fn layout_source_preserves_required_strict_and_decoded_members() {
    let baseline = &cases()[0];
    let request = &baseline["request"];
    for (pointer, fields) in [
        (
            "",
            &[
                "schemaVersion",
                "trackSize",
                "thumbSize",
                "insets",
                "layoutDirection",
                "checked",
            ] as &[&str],
        ),
        ("/insets", &["start", "end", "top", "bottom"]),
        ("/trackSize", &["width", "height"]),
        ("/thumbSize", &["width", "height"]),
    ] {
        let object = request.pointer(pointer).unwrap();
        let members: Vec<_> = fields
            .iter()
            .map(|field| format!("\"{field}\":{}", object[*field]))
            .collect();
        let replace = |members: &[String]| {
            let literal = format!("{{{}}}", members.join(","));
            if pointer.is_empty() {
                literal
            } else {
                let mut marker = request.clone();
                *marker.pointer_mut(pointer).unwrap() = json!("member-marker");
                marker.to_string().replace("\"member-marker\"", &literal)
            }
        };
        let mut reordered = members.clone();
        reordered.reverse();
        success(&replace(&reordered), &baseline["expected"]);
        for (index, field) in fields.iter().enumerate() {
            let escaped = format!(
                "\"\\u{:04x}{}\":{}",
                field.as_bytes()[0],
                &field[1..],
                object[*field]
            );
            let mut changed = members.clone();
            changed[index] = escaped.clone();
            success(&replace(&changed), &baseline["expected"]);
            for duplicate in [&members[index], &escaped] {
                changed[index] = format!("{},{}", members[index], duplicate);
                failure(&replace(&changed));
            }
            let mut missing = request.clone();
            missing
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(*field);
            failure(&missing.to_string());
        }
        let mut unknown = request.clone();
        unknown.pointer_mut(pointer).unwrap()["unknown"] = json!(false);
        failure(&unknown.to_string());
    }
}
