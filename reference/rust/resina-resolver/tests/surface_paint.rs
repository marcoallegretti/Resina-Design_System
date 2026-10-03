use resina_resolver::{
    FocusIndicatorError, FocusIrError, SurfacePaintResolutionError, resolve_opaque_surface_source,
    resolve_surface_paint_source,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    io::Write,
    process::{Command, Stdio},
};

fn request() -> Value {
    json!({"schemaVersion": "0.1.0", "body": serde_json::from_str::<Value>(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json")).unwrap(),
        "surroundingColor": {"colorSpace": "srgb", "components": [0, 0, 0], "alpha": 1}})
}

fn apply(document: &mut Value, changes: &Value) {
    for change in changes.as_array().unwrap() {
        *document
            .pointer_mut(change["path"].as_str().unwrap())
            .unwrap() = change["value"].clone();
    }
}

#[test]
fn all_public_body_appearances_compose_with_matching_navigation() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/ir/opaque-surface-cases.json"
    ))
    .unwrap();
    let mut families = BTreeSet::new();
    for case in cases
        .into_iter()
        .filter(|case| case.get("errorContains").is_none())
    {
        let mut request = request();
        apply(&mut request["body"], &case["requestChanges"]);
        for states in [
            json!(["rest"]),
            json!(["focused"]),
            json!(["focused", "rest"]),
        ] {
            request["body"]["surface"]["states"]["states"] = states.clone();
            let paint = resolve_surface_paint_source(&request.to_string()).unwrap();
            assert_eq!(
                paint.body(),
                &resolve_opaque_surface_source(&request["body"].to_string()).unwrap()
            );
            let value = serde_json::to_value(&paint).unwrap();
            families.insert(value["body"]["materialFamily"].as_str().unwrap().to_owned());
            if states == json!(["rest"]) {
                assert!(paint.focus().is_none());
                assert!(value.get("focus").is_none());
            } else {
                let focus = paint.focus().unwrap();
                assert_eq!(
                    paint.body().geometry().silhouette(),
                    focus.geometry().silhouette()
                );
                for field in [
                    "states",
                    "materialRole",
                    "colorRole",
                    "materialFamily",
                    "form",
                ] {
                    assert_eq!(
                        value["body"][field],
                        value["focus"]["indicator"]["binding"][field]
                    );
                }
            }
        }
    }
    assert_eq!(
        families,
        ["cast", "elastomer", "frost", "gel"]
            .map(String::from)
            .into()
    );
}

#[test]
fn public_source_cases_keep_diagnostic_failures_explicit() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/ir/surface-paint-cases.json"
    ))
    .unwrap();
    for case in cases {
        let mut request = request();
        apply(&mut request, &case["requestChanges"]);
        if case["omitSurroundingColor"] == true {
            request.as_object_mut().unwrap().remove("surroundingColor");
        }
        let result = resolve_surface_paint_source(&request.to_string());
        if let Some(message) = case["errorContains"].as_str() {
            let error = result.unwrap_err().to_string();
            assert!(error.contains(message), "{}: {error}", case["name"]);
        } else {
            assert_eq!(
                result.unwrap().focus().is_some(),
                case["expectedFocus"] == true
            );
        }
    }
}

fn run(source: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resina-surface-paint"))
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(source).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn focus_failure_publishes_no_body_and_cli_rejects_invalid_utf8_and_usage() {
    let mut request = request();
    request["body"]["surface"]["states"]["states"] = json!(["focused"]);
    let mut theme: Value =
        serde_json::from_str(request["body"]["theme"]["themeSource"].as_str().unwrap()).unwrap();
    theme["opaqueColorAssignments"]["roles"]["focus"] = json!("palette.black");
    request["body"]["theme"]["themeSource"] = json!(theme.to_string());
    assert!(resolve_opaque_surface_source(&request["body"].to_string()).is_ok());
    assert!(matches!(
        resolve_surface_paint_source(&request.to_string()),
        Err(SurfacePaintResolutionError::Focus(FocusIrError::Indicator(
            FocusIndicatorError::InsufficientContrast { .. }
        )))
    ));
    for source in [request.to_string().into_bytes(), vec![0xff]] {
        let output = run(&source);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    let output = Command::new(env!("CARGO_BIN_EXE_resina-surface-paint"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}
