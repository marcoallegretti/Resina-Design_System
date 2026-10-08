use resina_resolver::{
    resolve_focus_ir_source, resolve_opaque_surface_source, resolve_surface_paint_source,
    resolve_surface_scenario_source,
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
    if binary == env!("CARGO_BIN_EXE_resina-surface-bind") {
        value(resolve_surface_scenario_source(source))
    } else if binary == env!("CARGO_BIN_EXE_resina-focus-ir") {
        value(resolve_focus_ir_source(source))
    } else if binary == env!("CARGO_BIN_EXE_resina-opaque-surface") {
        value(resolve_opaque_surface_source(source))
    } else {
        assert_eq!(binary, env!("CARGO_BIN_EXE_resina-surface-paint"));
        value(resolve_surface_paint_source(source))
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

#[test]
fn surface_intent_forms_are_checked_before_publication() {
    let vectors: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/surfaces/binding-vectors.json"
    ))
    .unwrap();
    let resolution: Value = serde_json::from_str(include_str!(
        "../../../../conformance/headless/valid-request.json"
    ))
    .unwrap();
    let focus: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/focus-ir-request.json"
    ))
    .unwrap();
    let opaque: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json"
    ))
    .unwrap();
    let consumers = [
        (
            env!("CARGO_BIN_EXE_resina-surface-bind"),
            json!({
                "schemaVersion":"0.4.0", "resolution":resolution, "surface":vectors[0]["document"]
            }),
            "/surface",
        ),
        (env!("CARGO_BIN_EXE_resina-focus-ir"), focus, "/surface"),
        (
            env!("CARGO_BIN_EXE_resina-opaque-surface"),
            opaque.clone(),
            "/surface",
        ),
        (
            env!("CARGO_BIN_EXE_resina-surface-paint"),
            json!({
                "schemaVersion":"0.1.0", "body":opaque,
                "surroundingColor":{"colorSpace":"srgb","components":[0,0,0],"alpha":1}
            }),
            "/body/surface",
        ),
    ];
    for (binary, baseline, pointer) in consumers {
        let source = baseline.to_string();
        let expected = resolve(binary, &source).unwrap();
        let output = run(binary, &source);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            expected
        );

        let surface = baseline.pointer(pointer).unwrap();
        let surface_source = surface.to_string();
        assert_eq!(source.matches(&surface_source).count(), 1);
        let mut escaped_surface = surface_source.clone();
        for field in ["materialRole", "colorRole"] {
            let role = surface[field].as_str().unwrap();
            let member = format!("\"{field}\":\"{role}\"");
            assert_eq!(escaped_surface.matches(&member).count(), 1);
            escaped_surface = escaped_surface.replace(
                &member,
                &format!(
                    "\"\\u{:04x}{}\":\"\\u{:04x}{}\"",
                    field.as_bytes()[0],
                    &field[1..],
                    role.as_bytes()[0],
                    &role[1..]
                ),
            );
        }
        let escaped = source.replace(&surface_source, &escaped_surface);
        assert_eq!(resolve(binary, &escaped).unwrap(), expected);
        let escaped_output = run(binary, &escaped);
        assert_eq!(escaped_output.status.code(), Some(0));
        assert!(escaped_output.stderr.is_empty());
        assert_eq!(escaped_output.stdout, output.stdout);

        for case in vectors.iter().filter(|case| {
            case["name"]
                .as_str()
                .unwrap()
                .starts_with("surface intent form ")
        }) {
            let mut request = baseline.clone();
            *request.pointer_mut(pointer).unwrap() = case["document"].clone();
            let source = request.to_string();
            let diagnostic = case["error"].as_str().unwrap();
            let error = resolve(binary, &source).unwrap_err();
            assert!(
                error.contains(diagnostic),
                "{binary}: {}: {error}",
                case["name"]
            );
            let output = run(binary, &source);
            assert_eq!(output.status.code(), Some(1), "{binary}: {}", case["name"]);
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
        }
    }
}
