use resina_resolver::{
    resolve_focus_indicator_source, resolve_frost_surface_readability_source,
    resolve_surface_readability_source, resolve_surface_scenario_source,
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
    nested: bool,
    resolve: fn(&str) -> Result<Value, String>,
}

impl Endpoint {
    fn scenario(&self) -> &Value {
        if self.nested {
            &self.request["scenario"]
        } else {
            &self.request
        }
    }

    fn source(&self, scenario: Value) -> String {
        if self.nested {
            let mut request = self.request.clone();
            request["scenario"] = scenario;
            request.to_string()
        } else {
            scenario.to_string()
        }
    }

    fn raw_source(&self, scenario: &str) -> String {
        if self.nested {
            let mut request = self.request.clone();
            request["scenario"] = json!("scenario-member-marker");
            let source = request.to_string();
            assert_eq!(source.matches("\"scenario-member-marker\"").count(), 1);
            source.replace("\"scenario-member-marker\"", scenario)
        } else {
            scenario.to_owned()
        }
    }
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
            binary: env!("CARGO_BIN_EXE_resina-surface-bind"),
            request: scenario,
            nested: false,
            resolve: |source| resolved(resolve_surface_scenario_source(source)),
        },
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-focus-indicator"),
            request: focus,
            nested: true,
            resolve: |source| resolved(resolve_focus_indicator_source(source)),
        },
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-frost-surface-readability"),
            request: frost,
            nested: true,
            resolve: |source| resolved(resolve_frost_surface_readability_source(source)),
        },
        Endpoint {
            binary: env!("CARGO_BIN_EXE_resina-surface-readability"),
            request: readability,
            nested: true,
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
fn scenario_records_require_objects_across_all_consumers() {
    for endpoint in endpoints() {
        let expected = (endpoint.resolve)(&endpoint.request.to_string()).unwrap();
        success(&endpoint, &endpoint.request.to_string(), &expected);
        let scenario = endpoint.scenario();
        for shape in [
            json!(["0.4.0", scenario["resolution"], scenario["surface"]]),
            json!(["9.9.9", scenario["resolution"], scenario["surface"]]),
            json!(["0.4.0"]),
            json!(["0.4.0", scenario["resolution"], scenario["surface"], false]),
            json!([]),
            json!(null),
            json!(true),
            json!(1),
            json!("scenario"),
        ] {
            failure(
                &endpoint,
                &endpoint.source(shape),
                "surface scenario must be a JSON object",
            );
        }
    }
}

#[test]
fn scenario_records_preserve_strict_required_and_decoded_members() {
    for endpoint in endpoints() {
        let expected = (endpoint.resolve)(&endpoint.request.to_string()).unwrap();
        let scenario = endpoint.scenario();
        let fields = ["schemaVersion", "resolution", "surface"];
        let members: Vec<_> = fields
            .iter()
            .map(|name| format!("\"{name}\":{}", scenario[*name]))
            .collect();
        let source =
            |members: &[String]| endpoint.raw_source(&format!("{{{}}}", members.join(",")));
        let mut reordered = members.clone();
        reordered.reverse();
        success(&endpoint, &source(&reordered), &expected);
        for (index, name) in fields.iter().enumerate() {
            let escaped = format!(
                "\"\\u{:04x}{}\":{}",
                name.as_bytes()[0],
                &name[1..],
                scenario[*name]
            );
            let mut changed = members.clone();
            changed[index] = escaped.clone();
            success(&endpoint, &source(&changed), &expected);
            for duplicate in [&members[index], &escaped] {
                changed[index] = format!("{},{}", members[index], duplicate);
                failure(&endpoint, &source(&changed), "duplicate JSON member");
            }
            let mut missing = scenario.clone();
            missing.as_object_mut().unwrap().remove(*name);
            failure(&endpoint, &endpoint.source(missing), "missing field");
        }
        let mut unknown = scenario.clone();
        unknown["extra"] = json!(false);
        failure(&endpoint, &endpoint.source(unknown), "unknown field");
    }
}

#[test]
fn unsupported_object_scenario_versions_precede_nested_contracts() {
    for endpoint in endpoints() {
        for scenario in [
            json!({"schemaVersion":"9.9.9"}),
            json!({"schemaVersion":"0.1.0", "resolution":null, "surface":null}),
        ] {
            failure(
                &endpoint,
                &endpoint.source(scenario),
                "schemaVersion must be 0.4.0",
            );
        }
    }
}
