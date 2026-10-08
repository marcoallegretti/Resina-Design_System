use resina_resolver::{
    compile_theme_source, compile_theme_source_with_sources, resolve_focus_ir_source,
    resolve_headless_source, resolve_opaque_surface_source, resolve_surface_paint_source,
    resolve_surface_scenario_source, resolve_theme_request_source,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fmt::Display,
    io::Write,
    process::{Command, Stdio},
};

const HEADLESS: &str = include_str!("../../../../conformance/headless/valid-request.json");
const THEME: &str = include_str!("../../../../conformance/themes/valid-source.json");

fn vectors() -> Vec<Value> {
    let all: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/materials/role-assignment-vectors.json"
    ))
    .unwrap();
    let invalid: Vec<_> = all
        .into_iter()
        .filter(|v| {
            v["name"]
                .as_str()
                .unwrap()
                .starts_with("material source shape ")
        })
        .collect();
    assert_eq!(invalid.len(), 20);
    invalid
}

#[test]
fn material_shapes_fail_at_both_compilation_entries() {
    let original: Value = serde_json::from_str(THEME).unwrap();
    for vector in vectors() {
        let mut theme = original.clone();
        theme["materialAssignments"] = vector["document"].clone();
        for result in [
            compile_theme_source(&theme.to_string()),
            compile_theme_source_with_sources(&theme.to_string(), &BTreeMap::new()),
        ] {
            let error = result
                .err()
                .expect("schema-invalid material must not compile");
            assert!(matches!(
                error,
                resina_resolver::ThemeCompilationError::Source(_)
            ));
            assert!(
                error
                    .to_string()
                    .contains(vector["error"].as_str().unwrap())
            );
        }
    }
}

fn requests() -> Vec<(&'static str, Value, &'static str, bool)> {
    let headless: Value = serde_json::from_str(HEADLESS).unwrap();
    let opaque: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json"
    ))
    .unwrap();
    let surface: Value = serde_json::from_str(include_str!(
        "../../../../conformance/surfaces/binding-vectors.json"
    ))
    .unwrap();
    vec![
        (
            env!("CARGO_BIN_EXE_resina-headless"),
            headless.clone(),
            "/materialAssignments",
            false,
        ),
        (
            env!("CARGO_BIN_EXE_resina-surface-bind"),
            json!({"schemaVersion":"0.4.0", "resolution":headless, "surface":surface[0]["document"]}),
            "/resolution/materialAssignments",
            false,
        ),
        (
            env!("CARGO_BIN_EXE_resina-theme-resolve"),
            json!({"schemaVersion":"0.1.0", "themeSource":THEME, "externalSources":{}, "environment":headless["environment"]}),
            "/themeSource",
            true,
        ),
        (
            env!("CARGO_BIN_EXE_resina-focus-ir"),
            serde_json::from_str(include_str!(
                "../../../../conformance/ir/focus-ir-request.json"
            ))
            .unwrap(),
            "/theme/themeSource",
            true,
        ),
        (
            env!("CARGO_BIN_EXE_resina-opaque-surface"),
            opaque.clone(),
            "/theme/themeSource",
            true,
        ),
        (
            env!("CARGO_BIN_EXE_resina-surface-paint"),
            json!({"schemaVersion":"0.1.0", "body":opaque, "surroundingColor":{"colorSpace":"srgb", "components":[0,0,0], "alpha":1}}),
            "/body/theme/themeSource",
            true,
        ),
    ]
}

fn result<T: Serialize, E: Display>(value: Result<T, E>) -> Result<Value, String> {
    value
        .map(|v| serde_json::to_value(v).unwrap())
        .map_err(|e| e.to_string())
}

fn resolve(binary: &str, source: &str) -> Result<Value, String> {
    if binary == env!("CARGO_BIN_EXE_resina-headless") {
        result(resolve_headless_source(source))
    } else if binary == env!("CARGO_BIN_EXE_resina-surface-bind") {
        result(resolve_surface_scenario_source(source))
    } else if binary == env!("CARGO_BIN_EXE_resina-theme-resolve") {
        result(resolve_theme_request_source(source))
    } else if binary == env!("CARGO_BIN_EXE_resina-focus-ir") {
        result(resolve_focus_ir_source(source))
    } else if binary == env!("CARGO_BIN_EXE_resina-opaque-surface") {
        result(resolve_opaque_surface_source(source))
    } else {
        assert_eq!(binary, env!("CARGO_BIN_EXE_resina-surface-paint"));
        result(resolve_surface_paint_source(source))
    }
}

fn invalid_assignment(original: &Value, document: &Value) -> Value {
    if document.as_array().is_some_and(|v| v.len() == 4) {
        return json!([
            original["schemaVersion"],
            original["surface"],
            original["control"],
            original["feedback"]
        ]);
    }
    if !document.is_object() {
        return document.clone();
    }
    let mut changed = original.clone();
    for (group, fields) in [
        (
            "surface",
            &["base", "content", "chrome", "raised", "transient"][..],
        ),
        ("control", &["passive", "interactive", "primary"][..]),
        ("feedback", &["focus", "selection", "drag"][..]),
    ] {
        if document[group].is_array() {
            changed[group] =
                Value::Array(fields.iter().map(|f| original[group][f].clone()).collect());
        } else {
            for field in fields {
                if document[group][field].is_object() {
                    changed[group][field] = json!({original[group][field].as_str().unwrap():null});
                }
            }
        }
    }
    changed
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
fn material_shapes_fail_before_any_composing_consumer_publishes() {
    for (binary, baseline, pointer, embedded) in requests() {
        let expected = resolve(binary, &baseline.to_string()).unwrap();
        let valid = run(binary, &baseline.to_string());
        assert_eq!(valid.status.code(), Some(0));
        assert!(valid.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<Value>(&valid.stdout).unwrap(),
            expected
        );
        let theme = embedded.then(|| {
            serde_json::from_str::<Value>(baseline.pointer(pointer).unwrap().as_str().unwrap())
                .unwrap()
        });
        let material = if embedded {
            &theme.as_ref().unwrap()["materialAssignments"]
        } else {
            baseline.pointer(pointer).unwrap()
        };
        for vector in vectors() {
            let mut request = baseline.clone();
            let assignment = invalid_assignment(material, &vector["document"]);
            if embedded {
                let mut source = theme.clone().unwrap();
                source["materialAssignments"] = assignment;
                *request.pointer_mut(pointer).unwrap() = Value::String(source.to_string());
            } else {
                *request.pointer_mut(pointer).unwrap() = assignment;
            }
            let source = request.to_string();
            let diagnostic = vector["error"].as_str().unwrap();
            assert!(
                resolve(binary, &source).unwrap_err().contains(diagnostic),
                "{binary}: {}",
                vector["name"]
            );
            let output = run(binary, &source);
            assert_eq!(
                output.status.code(),
                Some(1),
                "{binary}: {}",
                vector["name"]
            );
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
        }
    }
}

fn color_vectors() -> Vec<(&'static str, Vec<Value>)> {
    [
        (
            "colorAssignments",
            include_str!("../../../../conformance/color/role-assignment-vectors.json"),
        ),
        (
            "opaqueColorAssignments",
            include_str!("../../../../conformance/color/opaque-assignment-vectors.json"),
        ),
    ]
    .into_iter()
    .map(|(key, source)| {
        let all: Vec<Value> = serde_json::from_str(source).unwrap();
        let invalid: Vec<_> = all
            .into_iter()
            .filter(|v| {
                v["name"]
                    .as_str()
                    .unwrap()
                    .starts_with("color source shape ")
            })
            .collect();
        assert_eq!(invalid.len(), 12);
        (key, invalid)
    })
    .collect()
}

fn color_shape(original: &Value, document: &Value, key: &str) -> Value {
    if document.as_array().is_some_and(|v| v.len() == 2) {
        return json!([original["schemaVersion"], original["roles"]]);
    }
    if !document.is_object() {
        return document.clone();
    }
    let mut changed = original.clone();
    changed["roles"] = if document["roles"].as_array().is_some_and(|v| !v.is_empty()) {
        if key == "colorAssignments" {
            Value::Array(
                resina_model::ColorRole::ALL
                    .into_iter()
                    .map(|role| {
                        let name = serde_json::to_value(role).unwrap();
                        original["roles"][name.as_str().unwrap()].clone()
                    })
                    .collect(),
            )
        } else {
            Value::Array(
                original["roles"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(role, path)| json!([role, path]))
                    .collect(),
            )
        }
    } else {
        document["roles"].clone()
    };
    changed
}

#[test]
fn color_shapes_fail_at_both_compilation_entries() {
    let original: Value = serde_json::from_str(THEME).unwrap();
    for (key, vectors) in color_vectors() {
        for vector in vectors {
            let mut theme = original.clone();
            theme[key] = color_shape(&original[key], &vector["document"], key);
            for result in [
                compile_theme_source(&theme.to_string()),
                compile_theme_source_with_sources(&theme.to_string(), &BTreeMap::new()),
            ] {
                let error = result
                    .err()
                    .expect("schema-invalid assignment must not compile");
                assert!(matches!(
                    error,
                    resina_resolver::ThemeCompilationError::Source(_)
                ));
                assert!(
                    error
                        .to_string()
                        .contains(vector["error"].as_str().unwrap()),
                    "{}: {error}",
                    vector["name"]
                );
            }
        }
    }
}

#[test]
fn color_shapes_fail_before_any_composing_consumer_publishes() {
    for (binary, baseline, pointer, embedded) in requests() {
        let expected = resolve(binary, &baseline.to_string()).unwrap();
        let valid = run(binary, &baseline.to_string());
        assert_eq!(valid.status.code(), Some(0));
        assert!(valid.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<Value>(&valid.stdout).unwrap(),
            expected
        );
        let owner = if embedded {
            serde_json::from_str::<Value>(baseline.pointer(pointer).unwrap().as_str().unwrap())
                .unwrap()
        } else {
            baseline
                .pointer(pointer.strip_suffix("/materialAssignments").unwrap())
                .unwrap()
                .clone()
        };
        for (key, vectors) in color_vectors() {
            for vector in vectors {
                let assignment = color_shape(&owner[key], &vector["document"], key);
                let mut request = baseline.clone();
                if embedded {
                    let mut source = owner.clone();
                    source[key] = assignment;
                    *request.pointer_mut(pointer).unwrap() = Value::String(source.to_string());
                } else {
                    request
                        .pointer_mut(pointer.strip_suffix("/materialAssignments").unwrap())
                        .unwrap()[key] = assignment;
                }
                let source = request.to_string();
                let diagnostic = vector["error"].as_str().unwrap();
                assert!(
                    resolve(binary, &source).unwrap_err().contains(diagnostic),
                    "{binary}: {}",
                    vector["name"]
                );
                let output = run(binary, &source);
                assert_eq!(
                    output.status.code(),
                    Some(1),
                    "{binary}: {}",
                    vector["name"]
                );
                assert!(output.stdout.is_empty());
                assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
            }
        }
    }
}

fn spatial_vectors() -> Vec<Value> {
    let all: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/spatial/assignment-vectors.json"
    ))
    .unwrap();
    let invalid: Vec<_> = all
        .into_iter()
        .filter(|v| {
            v["name"]
                .as_str()
                .unwrap()
                .starts_with("spatial source shape ")
        })
        .collect();
    assert_eq!(invalid.len(), 12);
    invalid
}

fn spatial_shape(original: &Value, document: &Value) -> Value {
    if document.as_array().is_some_and(|v| v.len() == 2) {
        return json!([original["schemaVersion"], original["roles"]]);
    }
    if !document.is_object() {
        return document.clone();
    }
    let mut changed = original.clone();
    changed["roles"] = if document["roles"].as_array().is_some_and(|v| !v.is_empty()) {
        Value::Array(
            original["roles"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(role, path)| json!([role, path]))
                .collect(),
        )
    } else {
        document["roles"].clone()
    };
    changed
}

#[test]
fn spatial_shapes_fail_at_both_compilation_entries() {
    let original: Value = serde_json::from_str(THEME).unwrap();
    for vector in spatial_vectors() {
        let mut theme = original.clone();
        theme["spatialAssignments"] =
            spatial_shape(&original["spatialAssignments"], &vector["document"]);
        for result in [
            compile_theme_source(&theme.to_string()),
            compile_theme_source_with_sources(&theme.to_string(), &BTreeMap::new()),
        ] {
            let error = result
                .err()
                .expect("schema-invalid spatial assignment must not compile");
            assert!(matches!(
                error,
                resina_resolver::ThemeCompilationError::Source(_)
            ));
            assert!(
                error
                    .to_string()
                    .contains(vector["error"].as_str().unwrap()),
                "{}: {error}",
                vector["name"]
            );
        }
    }
}

#[test]
fn spatial_shapes_fail_before_any_composing_consumer_publishes() {
    for (binary, baseline, pointer, embedded) in requests() {
        let expected = resolve(binary, &baseline.to_string()).unwrap();
        let valid = run(binary, &baseline.to_string());
        assert_eq!(valid.status.code(), Some(0));
        assert!(valid.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<Value>(&valid.stdout).unwrap(),
            expected
        );
        let owner = if embedded {
            serde_json::from_str::<Value>(baseline.pointer(pointer).unwrap().as_str().unwrap())
                .unwrap()
        } else {
            baseline
                .pointer(pointer.strip_suffix("/materialAssignments").unwrap())
                .unwrap()
                .clone()
        };
        for vector in spatial_vectors() {
            let assignment = spatial_shape(&owner["spatialAssignments"], &vector["document"]);
            let mut request = baseline.clone();
            if embedded {
                let mut source = owner.clone();
                source["spatialAssignments"] = assignment;
                *request.pointer_mut(pointer).unwrap() = Value::String(source.to_string());
            } else {
                request
                    .pointer_mut(pointer.strip_suffix("/materialAssignments").unwrap())
                    .unwrap()["spatialAssignments"] = assignment;
            }
            let source = request.to_string();
            let diagnostic = vector["error"].as_str().unwrap();
            assert!(
                resolve(binary, &source).unwrap_err().contains(diagnostic),
                "{binary}: {}",
                vector["name"]
            );
            let output = run(binary, &source);
            assert_eq!(
                output.status.code(),
                Some(1),
                "{binary}: {}",
                vector["name"]
            );
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
        }
    }
}

fn typography_vectors() -> Vec<Value> {
    let all: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/typography/assignment-vectors.json"
    ))
    .unwrap();
    let invalid: Vec<_> = all
        .into_iter()
        .filter(|v| {
            v["name"]
                .as_str()
                .unwrap()
                .starts_with("typography source shape ")
        })
        .collect();
    assert_eq!(invalid.len(), 82);
    invalid
}

fn typography_shape(original: &Value, document: &Value) -> Value {
    if document.as_array().is_some_and(|v| v.len() == 2) {
        return json!([original["schemaVersion"], original["roles"]]);
    }
    if !document.is_object() {
        return document.clone();
    }
    let mut changed = original.clone();
    if !document["roles"].is_object() {
        changed["roles"] = if document["roles"].as_array().is_some_and(|v| !v.is_empty()) {
            Value::Array(
                original["roles"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(role, spec)| json!([role, spec]))
                    .collect(),
            )
        } else {
            document["roles"].clone()
        };
        return changed;
    }
    for (role, spec) in document["roles"].as_object().unwrap() {
        if spec.as_array().is_some_and(|v| !v.is_empty()) {
            changed["roles"][role] = Value::Array(
                [
                    "familyRole",
                    "fontSize",
                    "fontWeight",
                    "lineHeight",
                    "letterSpacing",
                    "minimumTextScale",
                ]
                .into_iter()
                .map(|field| original["roles"][role][field].clone())
                .collect(),
            );
        } else if !spec.is_object() {
            changed["roles"][role] = spec.clone();
        } else if spec["familyRole"].is_object() {
            changed["roles"][role]["familyRole"] =
                json!({original["roles"][role]["familyRole"].as_str().unwrap():null});
        }
    }
    changed
}

#[test]
fn typography_shapes_fail_at_both_compilation_entries() {
    let original: Value = serde_json::from_str(THEME).unwrap();
    for vector in typography_vectors() {
        let mut theme = original.clone();
        theme["typographyAssignments"] =
            typography_shape(&original["typographyAssignments"], &vector["document"]);
        for result in [
            compile_theme_source(&theme.to_string()),
            compile_theme_source_with_sources(&theme.to_string(), &BTreeMap::new()),
        ] {
            let error = result
                .err()
                .expect("schema-invalid typography assignment must not compile");
            assert!(matches!(
                error,
                resina_resolver::ThemeCompilationError::Source(_)
            ));
            assert!(
                error
                    .to_string()
                    .contains(vector["error"].as_str().unwrap()),
                "{}: {error}",
                vector["name"]
            );
        }
    }
}

#[test]
fn typography_shapes_fail_before_any_composing_consumer_publishes() {
    for (binary, baseline, pointer, embedded) in requests() {
        let expected = resolve(binary, &baseline.to_string()).unwrap();
        let valid = run(binary, &baseline.to_string());
        assert_eq!(valid.status.code(), Some(0));
        assert!(valid.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<Value>(&valid.stdout).unwrap(),
            expected
        );
        let owner = if embedded {
            serde_json::from_str::<Value>(baseline.pointer(pointer).unwrap().as_str().unwrap())
                .unwrap()
        } else {
            baseline
                .pointer(pointer.strip_suffix("/materialAssignments").unwrap())
                .unwrap()
                .clone()
        };
        for vector in typography_vectors() {
            let assignment = typography_shape(&owner["typographyAssignments"], &vector["document"]);
            let mut request = baseline.clone();
            if embedded {
                let mut source = owner.clone();
                source["typographyAssignments"] = assignment;
                *request.pointer_mut(pointer).unwrap() = Value::String(source.to_string());
            } else {
                request
                    .pointer_mut(pointer.strip_suffix("/materialAssignments").unwrap())
                    .unwrap()["typographyAssignments"] = assignment;
            }
            let source = request.to_string();
            let diagnostic = vector["error"].as_str().unwrap();
            assert!(
                resolve(binary, &source).unwrap_err().contains(diagnostic),
                "{binary}: {}",
                vector["name"]
            );
            let output = run(binary, &source);
            assert_eq!(
                output.status.code(),
                Some(1),
                "{binary}: {}",
                vector["name"]
            );
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
        }
    }
}
