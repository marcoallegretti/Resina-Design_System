use resina_resolver::{
    resolve_focus_ir_source, resolve_opaque_surface_source, resolve_shape_fallback_source,
    resolve_surface_paint_source,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fmt::Display,
    io::Write,
    process::{Command, Stdio},
};

fn result<T: Serialize, E: Display>(value: Result<T, E>) -> Result<Value, String> {
    value
        .map(|v| serde_json::to_value(v).unwrap())
        .map_err(|e| e.to_string())
}

fn resolve(binary: &str, source: &str) -> Result<Value, String> {
    if binary == env!("CARGO_BIN_EXE_resina-shape-fallback") {
        result(resolve_shape_fallback_source(source))
    } else if binary == env!("CARGO_BIN_EXE_resina-focus-ir") {
        result(resolve_focus_ir_source(source))
    } else if binary == env!("CARGO_BIN_EXE_resina-opaque-surface") {
        result(resolve_opaque_surface_source(source))
    } else {
        assert_eq!(binary, env!("CARGO_BIN_EXE_resina-surface-paint"));
        result(resolve_surface_paint_source(source))
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
fn malformed_shape_intents_fail_before_consumers_publish() {
    let vectors: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/surface-form-vectors.json"
    ))
    .unwrap();
    let cases: Vec<_> = vectors
        .iter()
        .filter(|v| {
            v["name"]
                .as_str()
                .unwrap()
                .starts_with("shape intent form ")
        })
        .collect();
    assert_eq!(cases.len(), 14);
    let shape = json!({"schemaVersion":"0.1.0",
        "tokens":serde_json::from_str::<Value>(include_str!("../../../../tokens/foundation.json")).unwrap(),
        "assignments":serde_json::from_str::<Value>(include_str!("../../../../definitions/tier0-shapes.json")).unwrap(),
        "shape":"structural","size":{"width":200,"height":80}});
    let focus: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/focus-ir-request.json"
    ))
    .unwrap();
    let opaque: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json"
    ))
    .unwrap();
    let consumers = [
        (env!("CARGO_BIN_EXE_resina-shape-fallback"), shape, "/shape"),
        (
            env!("CARGO_BIN_EXE_resina-focus-ir"),
            focus,
            "/surface/form/shape",
        ),
        (
            env!("CARGO_BIN_EXE_resina-opaque-surface"),
            opaque.clone(),
            "/surface/form/shape",
        ),
        (
            env!("CARGO_BIN_EXE_resina-surface-paint"),
            json!({"schemaVersion":"0.1.0",
            "body":opaque,"surroundingColor":{"colorSpace":"srgb","components":[0,0,0],"alpha":1}}),
            "/body/surface/form/shape",
        ),
    ];
    for (binary, baseline, pointer) in consumers {
        for name in ["structural", "soft", "rounded", "capsule", "organic"] {
            let mut request = baseline.clone();
            *request.pointer_mut(pointer).unwrap() = json!(name);
            let source = request.to_string();
            let expected = resolve(binary, &source).unwrap();
            let output = run(binary, &source);
            assert_eq!(output.status.code(), Some(0), "{binary}: {name}");
            assert!(output.stderr.is_empty());
            assert_eq!(
                serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                expected
            );
            let member = format!("\"shape\":\"{name}\"");
            assert_eq!(source.matches(&member).count(), 1);
            let escaped = source.replace(
                &member,
                &format!(
                    "\"\\u0073hape\":\"\\u{:04x}{}\"",
                    name.as_bytes()[0],
                    &name[1..]
                ),
            );
            assert_eq!(resolve(binary, &escaped).unwrap(), expected);
            let escaped_output = run(binary, &escaped);
            assert_eq!(escaped_output.status.code(), Some(0));
            assert!(escaped_output.stderr.is_empty());
            assert_eq!(escaped_output.stdout, output.stdout);
            for field in ["shape", r"\u0073hape"] {
                let duplicate =
                    source.replace(&member, &format!("{member},\"{field}\":\"{name}\""));
                assert!(
                    resolve(binary, &duplicate)
                        .unwrap_err()
                        .contains("duplicate")
                );
                let output = run(binary, &duplicate);
                assert_eq!(output.status.code(), Some(1));
                assert!(output.stdout.is_empty());
                assert!(String::from_utf8_lossy(&output.stderr).contains("duplicate"));
            }
        }
        for case in &cases {
            let mut request = baseline.clone();
            *request.pointer_mut(pointer).unwrap() = case["document"]["shape"].clone();
            let source = request.to_string();
            let fragment = case["error"].as_str().unwrap();
            let error = resolve(binary, &source).unwrap_err();
            assert!(
                error.contains(fragment),
                "{binary}: {}: {error}",
                case["name"]
            );
            let output = run(binary, &source);
            assert_eq!(output.status.code(), Some(1), "{binary}: {}", case["name"]);
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains(fragment));
        }
    }
}
