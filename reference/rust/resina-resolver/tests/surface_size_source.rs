use resina_resolver::{
    resolve_extruded_contour_source, resolve_focus_ir_source, resolve_hit_region_source,
    resolve_inset_contour_source, resolve_opaque_surface_source, resolve_shape_fallback_source,
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
    } else if binary == env!("CARGO_BIN_EXE_resina-inset-contour") {
        result(resolve_inset_contour_source(source))
    } else if binary == env!("CARGO_BIN_EXE_resina-extruded-contour") {
        result(resolve_extruded_contour_source(source))
    } else if binary == env!("CARGO_BIN_EXE_resina-hit-region") {
        result(resolve_hit_region_source(source))
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

fn malformed_size(original: &Value, case: &Value) -> Value {
    let label = case["name"]
        .as_str()
        .unwrap()
        .strip_prefix("surface size form ")
        .unwrap();
    if label == "positional" {
        return json!([original["width"], original["height"]]);
    }
    let mut value = original.clone();
    if let Some(field) = label.strip_prefix("missing ") {
        value.as_object_mut().unwrap().remove(field);
    } else if label == "unknown member" {
        value["depth"] = json!(1);
    } else if label.starts_with("width ") || label.starts_with("height ") {
        let field = label.split_once(' ').unwrap().0;
        value[field] = case["document"][field].clone();
    } else {
        return case["document"].clone();
    }
    value
}

#[test]
fn malformed_sizes_fail_before_geometry_or_hit_regions_publish() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/surface-size-vectors.json"
    ))
    .unwrap();
    let cases: Vec<_> = cases
        .into_iter()
        .filter(|c| c.get("error").is_some())
        .collect();
    assert_eq!(cases.len(), 19);
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
    let inset: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/inset-contour-vectors.json"
    ))
    .unwrap();
    let extruded: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/extruded-contour-vectors.json"
    ))
    .unwrap();
    let hit: Value = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/hit-region-request.json"
    ))
    .unwrap();
    let consumers = [
        (env!("CARGO_BIN_EXE_resina-shape-fallback"), shape, "/size"),
        (env!("CARGO_BIN_EXE_resina-focus-ir"), focus, "/size"),
        (
            env!("CARGO_BIN_EXE_resina-opaque-surface"),
            opaque.clone(),
            "/size",
        ),
        (
            env!("CARGO_BIN_EXE_resina-surface-paint"),
            json!({"schemaVersion":"0.1.0","body":opaque,
            "surroundingColor":{"colorSpace":"srgb","components":[0,0,0],"alpha":1}}),
            "/body/size",
        ),
        (
            env!("CARGO_BIN_EXE_resina-inset-contour"),
            inset[0]["request"].clone(),
            "/size",
        ),
        (
            env!("CARGO_BIN_EXE_resina-extruded-contour"),
            extruded[0]["request"].clone(),
            "/size",
        ),
        (
            env!("CARGO_BIN_EXE_resina-hit-region"),
            hit,
            "/componentMinimum",
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
        let original = baseline.pointer(pointer).unwrap();
        let object = original.to_string();
        assert_eq!(source.matches(&object).count(), 1, "{binary}");
        for field in ["width", "height"] {
            let escaped = format!("\\u{:04x}{}", field.as_bytes()[0], &field[1..]);
            let changed = source.replace(
                &object,
                &object.replace(&format!("\"{field}\""), &format!("\"{escaped}\"")),
            );
            assert_eq!(resolve(binary, &changed).unwrap(), expected);
            let valid = run(binary, &changed);
            assert_eq!(valid.status.code(), Some(0));
            assert!(valid.stderr.is_empty());
            assert_eq!(valid.stdout, output.stdout);
            for duplicate in [field, &escaped] {
                let object_duplicate = format!(
                    "{},\"{duplicate}\":{}}}",
                    &object[..object.len() - 1],
                    original[field]
                );
                let changed = source.replace(&object, &object_duplicate);
                assert!(resolve(binary, &changed).unwrap_err().contains("duplicate"));
                let rejected = run(binary, &changed);
                assert_eq!(rejected.status.code(), Some(1));
                assert!(rejected.stdout.is_empty());
                assert!(String::from_utf8_lossy(&rejected.stderr).contains("duplicate"));
            }
        }
        for case in &cases {
            let mut request = baseline.clone();
            *request.pointer_mut(pointer).unwrap() = malformed_size(original, case);
            let source = request.to_string();
            let error = resolve(binary, &source).unwrap_err();
            let fragment = if binary == env!("CARGO_BIN_EXE_resina-hit-region")
                && !case["document"].is_object()
            {
                "must be a JSON object"
            } else {
                case["error"].as_str().unwrap()
            };
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
