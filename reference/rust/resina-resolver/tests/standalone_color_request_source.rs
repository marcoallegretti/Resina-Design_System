use resina_resolver::{
    resolve_edge_contrast_source, resolve_frost_legibility_source, resolve_opaque_pigment_source,
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
    version_precheck: bool,
    resolve: fn(&str) -> Result<Value, String>,
}

fn resolved<T: Serialize, E: Display>(result: Result<T, E>) -> Result<Value, String> {
    result
        .map(|value| serde_json::to_value(value).unwrap())
        .map_err(|error| error.to_string())
}

fn endpoints() -> Vec<Endpoint> {
    let request = |source: &str| {
        let cases: Vec<Value> = serde_json::from_str(source).unwrap();
        cases
            .into_iter()
            .find(|case| case.get("expected").is_some())
            .unwrap()["request"]
            .clone()
    };
    vec![
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-frost-legibility"),
            request: request(include_str!(
                "../../../../conformance/materials/frost-legibility-vectors.json"
            )),
            fields: &[
                "schemaVersion",
                "representation",
                "portableBody",
                "opaqueBody",
                "foreground",
                "postTreatmentBackdrop",
                "minimumContrast",
            ],
            diagnostic: "Frost legibility request must be a JSON object",
            version_precheck: true,
            resolve: |source| resolved(resolve_frost_legibility_source(source)),
        },
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-edge-contrast"),
            request: request(include_str!(
                "../../../../conformance/color/edge-contrast-vectors.json"
            )),
            fields: &[
                "schemaVersion",
                "outline",
                "outlineStrong",
                "adjacentColor",
                "minimumContrast",
            ],
            diagnostic: "edge contrast request must be a JSON object",
            version_precheck: true,
            resolve: |source| resolved(resolve_edge_contrast_source(source)),
        },
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-opaque-pigment"),
            request: request(include_str!(
                "../../../../conformance/materials/opaque-pigment-vectors.json"
            )),
            fields: &["schemaVersion", "materialFamily", "body", "profiles"],
            diagnostic: "opaque pigment request must be a JSON object",
            version_precheck: false,
            resolve: |source| resolved(resolve_opaque_pigment_source(source)),
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
            failure(&endpoint, &missing.to_string(), "missing field");
        }
        let mut unknown = endpoint.request.clone();
        unknown["extra"] = json!(false);
        failure(&endpoint, &unknown.to_string(), "unknown field");
    }
}

#[test]
fn object_request_versions_preserve_each_decoders_priority() {
    for endpoint in endpoints() {
        for version in ["9.9.9", "0.0.0"] {
            let mut request = endpoint.request.clone();
            request["schemaVersion"] = json!(version);
            failure(
                &endpoint,
                &request.to_string(),
                "schemaVersion must be 0.1.0",
            );
        }
        failure(
            &endpoint,
            "{\"schemaVersion\":\"9.9.9\"}",
            "schemaVersion must be 0.1.0",
        );
        let mut request = endpoint.request.clone();
        request["schemaVersion"] = json!("9.9.9");
        request[endpoint.fields[1]] = json!(null);
        failure(
            &endpoint,
            &request.to_string(),
            if endpoint.version_precheck {
                "schemaVersion must be 0.1.0"
            } else {
                "invalid type"
            },
        );
        for version in [json!(null), json!(1), json!(true), json!([]), json!({})] {
            let mut request = endpoint.request.clone();
            request["schemaVersion"] = version;
            failure(&endpoint, &request.to_string(), "invalid type");
        }
    }
}
