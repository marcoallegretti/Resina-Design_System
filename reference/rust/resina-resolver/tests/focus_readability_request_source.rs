use resina_resolver::{
    resolve_focus_indicator_source, resolve_frost_surface_readability_source,
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
    diagnostic: &'static str,
    resolve: fn(&str) -> Result<Value, String>,
}

fn resolved<T: Serialize, E: Display>(result: Result<T, E>) -> Result<Value, String> {
    result
        .map(|value| serde_json::to_value(value).unwrap())
        .map_err(|error| error.to_string())
}

fn endpoints() -> Vec<Endpoint> {
    let resolution: Value = serde_json::from_str(include_str!(
        "../../../../conformance/headless/valid-request.json"
    ))
    .unwrap();
    let bindings: Value = serde_json::from_str(include_str!(
        "../../../../conformance/surfaces/binding-vectors.json"
    ))
    .unwrap();
    let scenario = json!({
        "schemaVersion": "0.4.0", "resolution": resolution, "surface": bindings[0]["document"],
    });
    let white = json!({"colorSpace":"srgb","components":[1,1,1],"alpha":1});
    let focus = json!({"schemaVersion":"0.1.0","scenario":scenario,"surroundingColor":white});
    let mut readability_scenario = scenario.clone();
    readability_scenario["resolution"]["colorAssignments"]["roles"]["content.primary"] =
        json!("palette.opaqueAlt");
    readability_scenario["resolution"]["opaqueColorAssignments"]["roles"]["outline.strong"] =
        json!("palette.opaqueAlt");
    readability_scenario["surface"]["states"]["states"] = json!(["rest"]);
    readability_scenario["surface"]["treatmentStack"]["treatments"] = json!(["none"]);
    let frost = json!({
        "schemaVersion":"0.1.0", "scenario":readability_scenario, "foregroundRole":"content.primary",
        "postTreatmentBackdrop":white, "adjacentColor":white,
        "minimumContentContrast":3, "minimumEdgeContrast":3,
    });
    let mut readability = frost.clone();
    readability["scenario"]["resolution"]["opaqueColorAssignments"]["roles"]["content.primary"] =
        json!("palette.opaqueAlt");
    vec![
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-focus-indicator"),
            request: focus,
            fields: &["schemaVersion", "scenario", "surroundingColor"],
            diagnostic: "focus indicator request must be a JSON object",
            resolve: |source| resolved(resolve_focus_indicator_source(source)),
        },
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-frost-surface-readability"),
            request: frost,
            fields: &[
                "schemaVersion",
                "scenario",
                "foregroundRole",
                "postTreatmentBackdrop",
                "adjacentColor",
                "minimumContentContrast",
                "minimumEdgeContrast",
            ],
            diagnostic: "Frost surface readability request must be a JSON object",
            resolve: |source| resolved(resolve_frost_surface_readability_source(source)),
        },
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-surface-readability"),
            request: readability,
            fields: &[
                "schemaVersion",
                "scenario",
                "foregroundRole",
                "postTreatmentBackdrop",
                "adjacentColor",
                "minimumContentContrast",
                "minimumEdgeContrast",
            ],
            diagnostic: "surface readability request must be a JSON object",
            resolve: |source| resolved(resolve_surface_readability_source(source)),
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
fn request_records_require_objects() {
    for endpoint in endpoints() {
        let expected = (endpoint.resolve)(&endpoint.request.to_string()).unwrap();
        success(&endpoint, &endpoint.request.to_string(), &expected);
        let positional: Vec<_> = endpoint
            .fields
            .iter()
            .map(|field| endpoint.request[*field].clone())
            .collect();
        let mut unsupported = positional.clone();
        unsupported[0] = json!("9.9.9");
        let mut extended = positional.clone();
        extended.push(json!(false));
        for shape in [
            json!(positional),
            json!(unsupported),
            json!(["0.1.0"]),
            json!(extended),
            json!([]),
            json!(null),
            json!(true),
            json!(1),
            json!("request"),
        ] {
            failure(&endpoint, &shape.to_string(), endpoint.diagnostic);
        }
    }
}

#[test]
fn request_records_preserve_required_and_decoded_members() {
    for endpoint in endpoints() {
        let expected = (endpoint.resolve)(&endpoint.request.to_string()).unwrap();
        let members: Vec<_> = endpoint
            .fields
            .iter()
            .map(|name| format!("\"{name}\":{}", endpoint.request[*name]))
            .collect();
        let source = |members: &[String]| format!("{{{}}}", members.join(","));
        let mut reordered = members.clone();
        reordered.reverse();
        success(&endpoint, &source(&reordered), &expected);
        for (index, name) in endpoint.fields.iter().enumerate() {
            let escaped = format!(
                "\"\\u{:04x}{}\":{}",
                name.as_bytes()[0],
                &name[1..],
                endpoint.request[*name]
            );
            let mut changed = members.clone();
            changed[index] = escaped.clone();
            success(&endpoint, &source(&changed), &expected);
            for duplicate in [&members[index], &escaped] {
                changed[index] = format!("{},{}", members[index], duplicate);
                failure(&endpoint, &source(&changed), "duplicate JSON member");
            }
            let mut missing = endpoint.request.clone();
            missing.as_object_mut().unwrap().remove(*name);
            let diagnostic = if *name == "postTreatmentBackdrop"
                && endpoint.binary == env!("CARGO_BIN_EXE_resina-surface-readability")
            {
                "Frost requires postTreatmentBackdrop"
            } else {
                "missing field"
            };
            failure(&endpoint, &missing.to_string(), diagnostic);
        }
        let mut unknown = endpoint.request.clone();
        unknown["extra"] = json!(false);
        failure(&endpoint, &unknown.to_string(), "unknown field");
    }
}

#[test]
fn object_request_versions_precede_member_and_nested_contracts() {
    for endpoint in endpoints() {
        for request in [
            json!({"schemaVersion":"9.9.9"}),
            json!({"schemaVersion":"0.0.0", "scenario":null, "extra":false}),
        ] {
            failure(
                &endpoint,
                &request.to_string(),
                "schemaVersion must be 0.1.0",
            );
        }
        for version in [json!(null), json!(1), json!(true), json!([]), json!({})] {
            let mut request = endpoint.request.clone();
            request["schemaVersion"] = version;
            failure(&endpoint, &request.to_string(), "invalid type");
        }
    }
}

#[test]
fn opaque_readability_preserves_optional_backdrop_admission() {
    let endpoint = endpoints().pop().unwrap();
    for family in ["cast", "elastomer", "gel"] {
        let mut request = endpoint.request.clone();
        if family == "gel" {
            request["scenario"]["surface"]["materialRole"] = json!("feedback.selection");
        } else {
            request["scenario"]["resolution"]["materialAssignments"]["surface"]["chrome"] =
                json!(family);
        }
        let expected = (endpoint.resolve)(&request.to_string()).unwrap();
        success(&endpoint, &request.to_string(), &expected);
        request
            .as_object_mut()
            .unwrap()
            .remove("postTreatmentBackdrop");
        success(&endpoint, &request.to_string(), &expected);
        request["postTreatmentBackdrop"] = json!(null);
        failure(&endpoint, &request.to_string(), "invalid type");
    }
}
