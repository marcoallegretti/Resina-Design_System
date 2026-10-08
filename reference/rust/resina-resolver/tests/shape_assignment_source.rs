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
fn shape_assignments_fail_before_any_consumer_publishes() {
    let assignment_cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/shape-fallback-assignment-vectors.json"
    ))
    .unwrap();
    let shape = json!({"schemaVersion":"0.1.0",
        "tokens":serde_json::from_str::<Value>(include_str!("../../../../tokens/foundation.json")).unwrap(),
        "assignments":assignment_cases[0]["document"],"shape":"structural",
        "size":{"width":200,"height":80}});
    let focus: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/focus-ir-request.json"
    ))
    .unwrap();
    let opaque: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json"
    ))
    .unwrap();
    let requests = [
        (
            env!("CARGO_BIN_EXE_resina-shape-fallback"),
            shape,
            include_str!("../../../../conformance/geometry/shape-fallback-assignment-vectors.json"),
        ),
        (
            env!("CARGO_BIN_EXE_resina-focus-ir"),
            focus,
            include_str!("../../../../conformance/ir/focus-ir-cases.json"),
        ),
        (
            env!("CARGO_BIN_EXE_resina-opaque-surface"),
            opaque.clone(),
            include_str!("../../../../conformance/ir/opaque-surface-cases.json"),
        ),
        (
            env!("CARGO_BIN_EXE_resina-surface-paint"),
            json!({"schemaVersion":"0.1.0",
         "body":opaque,"surroundingColor":{"colorSpace":"srgb","components":[0,0,0],"alpha":1}}),
            include_str!("../../../../conformance/ir/surface-paint-cases.json"),
        ),
    ];
    for (binary, baseline, source_cases) in requests {
        let expected = resolve(binary, &baseline.to_string()).unwrap();
        let output = run(binary, &baseline.to_string());
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            expected
        );
        let all: Vec<Value> = serde_json::from_str(source_cases).unwrap();
        let cases: Vec<_> = all
            .into_iter()
            .filter(|c| {
                c["name"]
                    .as_str()
                    .unwrap()
                    .starts_with("shape source form ")
            })
            .collect();
        assert_eq!(cases.len(), 54);
        for case in cases {
            let mut request = baseline.clone();
            if let Some(value) = case.get("document") {
                request["assignments"] = value.clone();
            } else {
                for change in case["requestChanges"].as_array().unwrap() {
                    *request
                        .pointer_mut(change["path"].as_str().unwrap())
                        .unwrap() = change["value"].clone();
                }
            }
            let source = request.to_string();
            let error = resolve(binary, &source).unwrap_err();
            assert!(
                error.contains("assignment object"),
                "{binary}: {}: {error}",
                case["name"]
            );
            let output = run(binary, &source);
            assert_eq!(output.status.code(), Some(1), "{binary}: {}", case["name"]);
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("assignment object"));
        }
    }
}
