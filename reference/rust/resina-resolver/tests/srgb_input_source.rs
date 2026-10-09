use resina_resolver::{
    resolve_edge_contrast_source, resolve_focus_indicator_source, resolve_focus_ir_source,
    resolve_frost_legibility_source, resolve_frost_surface_readability_source,
    resolve_opaque_pigment_source, resolve_opaque_surface_source, resolve_surface_paint_source,
    resolve_surface_readability_source,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fmt::Display,
    io::Write,
    process::{Command, Stdio},
};

struct Endpoint {
    binary: &'static str,
    request: Value,
    fields: &'static [&'static str],
    resolve: fn(&str) -> Result<Value, String>,
}

fn resolved<T: Serialize, E: Display>(result: Result<T, E>) -> Result<Value, String> {
    result
        .map(|value| serde_json::to_value(value).unwrap())
        .map_err(|error| error.to_string())
}

fn document(source: &str) -> Value {
    serde_json::from_str(source).unwrap()
}

fn color(components: [u8; 3]) -> Value {
    json!({"colorSpace": "srgb", "components": components, "alpha": 1})
}

fn focus_request() -> Value {
    let binding = document(include_str!(
        "../../../../conformance/surfaces/binding-vectors.json"
    ));
    json!({
        "schemaVersion": "0.1.0",
        "scenario": {
            "schemaVersion": "0.4.0",
            "resolution": document(include_str!("../../../../conformance/headless/valid-request.json")),
            "surface": binding[0]["document"],
        },
        "surroundingColor": color([1, 1, 1]),
    })
}

fn readability_request() -> Value {
    let mut scenario = focus_request()["scenario"].clone();
    scenario["resolution"]["colorAssignments"]["roles"]["content.primary"] =
        json!("palette.opaqueAlt");
    scenario["resolution"]["opaqueColorAssignments"]["roles"]["outline.strong"] =
        json!("palette.opaqueAlt");
    scenario["surface"]["states"]["states"] = json!(["rest"]);
    scenario["surface"]["treatmentStack"]["treatments"] = json!(["none"]);
    json!({
        "schemaVersion": "0.1.0", "scenario": scenario, "foregroundRole": "content.primary",
        "postTreatmentBackdrop": color([1, 1, 1]), "adjacentColor": color([1, 1, 1]),
        "minimumContentContrast": 3, "minimumEdgeContrast": 3,
    })
}

fn endpoints() -> Vec<Endpoint> {
    let opaque = document(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json"
    ));
    let mut readability = readability_request();
    readability["scenario"]["resolution"]["opaqueColorAssignments"]["roles"]["content.primary"] =
        json!("palette.opaqueAlt");
    vec![
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-opaque-pigment"),
            request: document(include_str!(
                "../../../../conformance/materials/opaque-pigment-vectors.json"
            ))[0]["request"]
                .clone(),
            fields: &["body"],
            resolve: |source| resolved(resolve_opaque_pigment_source(source)),
        },
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-frost-legibility"),
            request: document(include_str!(
                "../../../../conformance/materials/frost-legibility-vectors.json"
            ))[0]["request"]
                .clone(),
            fields: &[
                "portableBody",
                "opaqueBody",
                "foreground",
                "postTreatmentBackdrop",
            ],
            resolve: |source| resolved(resolve_frost_legibility_source(source)),
        },
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-edge-contrast"),
            request: document(include_str!(
                "../../../../conformance/color/edge-contrast-vectors.json"
            ))[0]["request"]
                .clone(),
            fields: &["outline", "outlineStrong", "adjacentColor"],
            resolve: |source| resolved(resolve_edge_contrast_source(source)),
        },
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-focus-indicator"),
            request: focus_request(),
            fields: &["surroundingColor"],
            resolve: |source| resolved(resolve_focus_indicator_source(source)),
        },
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-frost-surface-readability"),
            request: readability_request(),
            fields: &["postTreatmentBackdrop", "adjacentColor"],
            resolve: |source| resolved(resolve_frost_surface_readability_source(source)),
        },
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-surface-readability"),
            request: readability,
            fields: &["postTreatmentBackdrop", "adjacentColor"],
            resolve: |source| resolved(resolve_surface_readability_source(source)),
        },
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-opaque-surface"),
            request: opaque.clone(),
            fields: &["postTreatmentBackdrop", "adjacentColor"],
            resolve: |source| resolved(resolve_opaque_surface_source(source)),
        },
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-surface-paint"),
            request: json!({"schemaVersion":"0.1.0","body":opaque,"surroundingColor":color([0,0,0])}),
            fields: &["surroundingColor"],
            resolve: |source| resolved(resolve_surface_paint_source(source)),
        },
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-focus-ir"),
            request: document(include_str!(
                "../../../../conformance/ir/focus-ir-request.json"
            )),
            fields: &["surroundingColor"],
            resolve: |source| resolved(resolve_focus_ir_source(source)),
        },
    ]
}

fn run(endpoint: &Endpoint, source: &str) -> std::process::Output {
    let mut child = Command::new(endpoint.binary)
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

fn success(endpoint: &Endpoint, source: &str, expected: &Value) {
    assert_eq!((endpoint.resolve)(source).unwrap(), *expected);
    let output = run(endpoint, source);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}: {source}",
        endpoint.binary
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        *expected
    );
}

fn failure(endpoint: &Endpoint, source: &str, diagnostic: &str) {
    let error = (endpoint.resolve)(source).unwrap_err();
    assert!(error.contains(diagnostic), "{}: {error}", endpoint.binary);
    let output = run(endpoint, source);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}: {source}",
        endpoint.binary
    );
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains(diagnostic)
    );
}

#[test]
fn srgb_source_records_require_objects_across_all_consumers() {
    for endpoint in endpoints() {
        let expected = (endpoint.resolve)(&endpoint.request.to_string()).unwrap();
        success(&endpoint, &endpoint.request.to_string(), &expected);
        for field in endpoint.fields {
            let original = &endpoint.request[*field];
            let positional = json!([
                original["colorSpace"],
                original["components"],
                original["alpha"]
            ]);
            for shape in [
                positional,
                json!([]),
                Value::Null,
                json!(true),
                json!(1),
                json!("srgb"),
            ] {
                let mut invalid = endpoint.request.clone();
                invalid[*field] = shape;
                failure(&endpoint, &invalid.to_string(), "sRGB color object");
            }
        }
    }
}

#[test]
fn srgb_source_records_preserve_strict_required_and_decoded_members() {
    for endpoint in endpoints() {
        let expected = (endpoint.resolve)(&endpoint.request.to_string()).unwrap();
        for field in endpoint.fields {
            let original = &endpoint.request[*field];
            let fields = ["colorSpace", "components", "alpha"];
            let members: Vec<_> = fields
                .iter()
                .map(|name| format!("\"{name}\":{}", original[*name]))
                .collect();
            let replace = |members: &[String]| {
                let mut request = endpoint.request.clone();
                request[*field] = json!("color-member-marker");
                let source = request.to_string();
                assert_eq!(source.matches("\"color-member-marker\"").count(), 1);
                source.replace(
                    "\"color-member-marker\"",
                    &format!("{{{}}}", members.join(",")),
                )
            };
            let mut reordered = members.clone();
            reordered.reverse();
            success(&endpoint, &replace(&reordered), &expected);
            for (index, name) in fields.iter().enumerate() {
                let escaped = format!(
                    "\"\\u{:04x}{}\":{}",
                    name.as_bytes()[0],
                    &name[1..],
                    original[*name]
                );
                let mut changed = members.clone();
                changed[index] = escaped.clone();
                success(&endpoint, &replace(&changed), &expected);
                for duplicate in [&members[index], &escaped] {
                    changed[index] = format!("{},{}", members[index], duplicate);
                    failure(&endpoint, &replace(&changed), "duplicate JSON member");
                }
                let mut missing = endpoint.request.clone();
                missing[*field].as_object_mut().unwrap().remove(*name);
                failure(&endpoint, &missing.to_string(), "missing field");
            }
            let mut unknown = endpoint.request.clone();
            unknown[*field]["extra"] = json!(false);
            failure(&endpoint, &unknown.to_string(), "unknown field");
        }
    }
}
