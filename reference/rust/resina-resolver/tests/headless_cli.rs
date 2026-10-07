use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

const SOURCE: &str = include_str!("../../../../conformance/headless/valid-request.json");
const EXPECTED: &str = include_str!("../../../../conformance/headless/expected-resolution.json");

#[test]
fn non_object_envelopes_fail_without_panicking_or_publishing() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/headless/backend-cases.json"
    ))
    .unwrap();
    let invalid: Vec<_> = cases
        .iter()
        .filter(|case| {
            case["name"]
                .as_str()
                .unwrap()
                .starts_with("non-object headless request ")
        })
        .collect();
    assert_eq!(invalid.len(), 7);
    for case in invalid {
        let source = case["sourceText"].as_str().unwrap();
        assert!(matches!(
            resina_resolver::resolve_headless_source(source),
            Err(resina_resolver::HeadlessResolutionError::InvalidRequestShape)
        ));
        let output = run_stdin(source);
        assert_eq!(output.status.code(), Some(1), "{}", case["name"]);
        assert!(output.stdout.is_empty(), "{}", case["name"]);
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("headless request must be a JSON object"),
            "{}",
            case["name"]
        );
    }
}

fn run_stdin(source: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resina-headless"))
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
fn file_and_stdin_resolve_to_the_same_complete_output() {
    let file = Command::new(env!("CARGO_BIN_EXE_resina-headless"))
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../conformance/headless/valid-request.json"
        ))
        .output()
        .unwrap();
    let stdin = run_stdin(SOURCE);
    assert!(
        file.status.success(),
        "{}",
        String::from_utf8_lossy(&file.stderr)
    );
    assert!(
        stdin.status.success(),
        "{}",
        String::from_utf8_lossy(&stdin.stderr)
    );
    assert!(file.stderr.is_empty());
    assert!(stdin.stderr.is_empty());
    assert_eq!(file.stdout, stdin.stdout);
    assert_eq!(
        serde_json::from_slice::<Value>(&file.stdout).unwrap(),
        serde_json::from_str::<Value>(EXPECTED).unwrap()
    );
}

#[test]
fn invalid_inputs_produce_diagnostics_without_partial_output() {
    let duplicate = SOURCE.replacen(
        "\"schemaVersion\": \"0.3.0\"",
        "\"schemaVersion\": \"0.3.0\", \"schemaVersion\": \"0.3.0\"",
        1,
    );
    let mut invalid_bindings: Value = serde_json::from_str(SOURCE).unwrap();
    invalid_bindings["colorAssignments"]["roles"]["focus"] = json!("missing.color");
    invalid_bindings["spatialAssignments"]["roles"]["space.page"] = json!("missing.space");
    invalid_bindings["typographyAssignments"]["roles"]["body"]["fontSize"] = json!("missing.type");
    let mut invalid_tokens: Value = serde_json::from_str(SOURCE).unwrap();
    invalid_tokens["tokens"]["type"]["size"]["$value"]["unit"] = json!("em");
    let mut invalid_request: Value = serde_json::from_str(SOURCE).unwrap();
    invalid_request["schemaVersion"] = json!("0.1.0");
    let mut missing_fallback: Value = serde_json::from_str(SOURCE).unwrap();
    missing_fallback["tokens"]["palette"]["base"]["$value"]["colorSpace"] = json!("display-p3");
    missing_fallback["tokens"]["palette"]["base"]["$value"]["components"] = json!([1, 0, 0]);
    missing_fallback["spatialAssignments"]["roles"]["space.page"] = json!("missing.space");
    missing_fallback["typographyAssignments"]["roles"]["body"]["fontSize"] = json!("missing.type");
    for (source, diagnostics) in [
        (duplicate, vec!["duplicate JSON member"]),
        (
            invalid_bindings.to_string(),
            vec![
                "color: MissingToken",
                "space: MissingToken",
                "typography: MissingToken",
            ],
        ),
        (invalid_tokens.to_string(), vec!["token resolution failed"]),
        (
            missing_fallback.to_string(),
            vec![
                "color fallback: AccentPrimary",
                "space: MissingToken",
                "typography: MissingToken",
            ],
        ),
        (
            invalid_request.to_string(),
            vec!["schemaVersion must be 0.3.0"],
        ),
    ] {
        let output = run_stdin(&source);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut previous = 0;
        for diagnostic in diagnostics {
            let index = stderr
                .find(diagnostic)
                .unwrap_or_else(|| panic!("{stderr}"));
            assert!(index >= previous, "{stderr}");
            previous = index + diagnostic.len();
        }
    }
}

#[test]
fn translucent_role_without_authored_opaque_fallback_has_no_result() {
    let mut request: Value = serde_json::from_str(SOURCE).unwrap();
    request["colorAssignments"]["roles"]["focus"] = json!("palette.translucent");
    request["opaqueColorAssignments"]["roles"]
        .as_object_mut()
        .unwrap()
        .remove("focus");
    let output = run_stdin(&request.to_string());
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("MissingAuthoredFallback"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn accessibility_and_input_capabilities_change_only_their_outputs() {
    let mut request: Value = serde_json::from_str(SOURCE).unwrap();
    request["environment"]["accessibilityPreferences"]["highContrast"] = json!(true);
    request["environment"]["inputCapabilities"] = json!(["keyboard"]);
    let output = run_stdin(&request.to_string());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
    let mut expected: Value = serde_json::from_str(EXPECTED).unwrap();
    expected["frostRepresentation"] = json!("opaqueDimensional");
    expected["minimumHitTarget"] = json!({"minimumWidth":24,"minimumHeight":24});
    assert_eq!(actual, expected);
}

#[test]
fn output_preserves_source_color_and_publishes_its_portable_fallback() {
    let mut request: Value = serde_json::from_str(SOURCE).unwrap();
    let source_color = json!({
        "colorSpace": "display-p3",
        "components": [0.2, 0.4, 0.6],
        "hex": "#336699",
        "alpha": 0.35
    });
    request["tokens"]["palette"]["wide"] = json!({"$value": source_color});
    request["colorAssignments"]["roles"]["focus"] = json!("palette.wide");

    let output = run_stdin(&request.to_string());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["colors"]["focus"], source_color);
    assert_eq!(
        result["colorFallbacks"]["focus"],
        json!({"colorSpace":"srgb","components":[0.2,0.4,0.6],"alpha":0.35})
    );
    assert_eq!(
        result["colorFallbacks"].as_object().unwrap().len(),
        result["colors"].as_object().unwrap().len()
    );
}

#[test]
fn numeric_oklab_color_resolves_without_authored_hex() {
    let mut request: Value = serde_json::from_str(SOURCE).unwrap();
    let source_color = json!({"colorSpace":"oklab","components":[0.5,0,0],"alpha":0.4});
    request["tokens"]["palette"]["neutral"] = json!({"$value": source_color});
    request["colorAssignments"]["roles"]["focus"] = json!("palette.neutral");

    let output = run_stdin(&request.to_string());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["colors"]["focus"], source_color);
    let fallback = &result["colorFallbacks"]["focus"];
    assert_eq!(fallback["colorSpace"], "srgb");
    assert_eq!(fallback["alpha"].as_f64(), Some(0.4));
    for channel in fallback["components"].as_array().unwrap() {
        assert!((channel.as_f64().unwrap() - 0.388572859046334).abs() < 1e-12);
    }
    assert_eq!(
        result["colorFallbacks"].as_object().unwrap().len(),
        result["colors"].as_object().unwrap().len()
    );
}

#[test]
fn numeric_oklch_color_resolves_without_authored_hex() {
    let mut request: Value = serde_json::from_str(SOURCE).unwrap();
    let source_color = json!({"colorSpace":"oklch","components":[0.6,0.1,90],"alpha":0.7});
    request["tokens"]["palette"]["hued"] = json!({"$value": source_color});
    request["colorAssignments"]["roles"]["focus"] = json!("palette.hued");

    let output = run_stdin(&request.to_string());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["colors"]["focus"], source_color);
    let fallback = &result["colorFallbacks"]["focus"];
    assert_eq!(fallback["colorSpace"], "srgb");
    assert_eq!(fallback["alpha"].as_f64(), Some(0.7));
    let expected = [0.59371851432602, 0.49082753894920433, 0.19011614138497857];
    for (index, channel) in fallback["components"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        assert!((channel.as_f64().unwrap() - expected[index]).abs() < 1e-12);
    }
}

#[test]
fn direct_srgb_spaces_resolve_without_authored_hex() {
    for (source_color, expected) in [
        (
            json!({"colorSpace":"srgb-linear","components":[0.0031308,0.18,1],"alpha":0.6}),
            [0.040449936, 0.461356129500442, 1.0],
        ),
        (
            json!({"colorSpace":"hsl","components":[30,100,50],"alpha":0.6}),
            [1.0, 0.5, 0.0],
        ),
        (
            json!({"colorSpace":"hwb","components":[120,20,30],"alpha":0.6}),
            [0.2, 0.7, 0.2],
        ),
    ] {
        let mut request: Value = serde_json::from_str(SOURCE).unwrap();
        request["tokens"]["palette"]["direct"] = json!({"$value": source_color});
        request["colorAssignments"]["roles"]["focus"] = json!("palette.direct");
        let output = run_stdin(&request.to_string());
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["colors"]["focus"], source_color);
        let fallback = &result["colorFallbacks"]["focus"];
        assert_eq!(fallback["colorSpace"], "srgb");
        assert_eq!(fallback["alpha"].as_f64(), Some(0.6));
        for (index, channel) in fallback["components"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            assert!((channel.as_f64().unwrap() - expected[index]).abs() < 1e-12);
        }
    }
}

#[test]
fn numeric_xyz_spaces_resolve_without_authored_hex() {
    for (source_color, expected) in [
        (
            json!({"colorSpace":"xyz-d65","components":[0.11865530579242775,0.12505925609252708,0.31926610717393133],"alpha":0.6}),
            [0.2, 0.4, 0.6],
        ),
        (
            json!({"colorSpace":"xyz-d50","components":[0.11118745908211884,0.12192740396619164,0.24083403014953822],"alpha":0.6}),
            [0.2, 0.4, 0.6],
        ),
    ] {
        let mut request: Value = serde_json::from_str(SOURCE).unwrap();
        request["tokens"]["palette"]["xyz"] = json!({"$value": source_color});
        request["colorAssignments"]["roles"]["focus"] = json!("palette.xyz");
        let output = run_stdin(&request.to_string());
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        let parsed_source: Value = serde_json::from_str(&source_color.to_string()).unwrap();
        assert_eq!(result["colors"]["focus"], parsed_source);
        let fallback = &result["colorFallbacks"]["focus"];
        assert_eq!(fallback["colorSpace"], "srgb");
        assert_eq!(fallback["alpha"].as_f64(), Some(0.6));
        for (index, channel) in fallback["components"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            assert!((channel.as_f64().unwrap() - expected[index]).abs() < 1e-12);
        }
    }
}

#[test]
fn numeric_predefined_rgb_spaces_resolve_without_authored_hex() {
    let vectors: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/color/srgb-fallback-vectors.json"
    ))
    .unwrap();
    let cases: Vec<&Value> = vectors
        .iter()
        .filter(|vector| {
            matches!(
                vector["value"]["colorSpace"].as_str(),
                Some("display-p3" | "a98-rgb" | "prophoto-rgb" | "rec2020")
            ) && vector.get("expected").is_some()
                && vector["value"].get("hex").is_none()
        })
        .collect();
    assert_eq!(cases.len(), 4);
    for vector in cases {
        let source_color = &vector["value"];
        let mut request: Value = serde_json::from_str(SOURCE).unwrap();
        request["tokens"]["palette"]["wide"] = json!({"$value": source_color});
        request["colorAssignments"]["roles"]["focus"] = json!("palette.wide");
        let output = run_stdin(&request.to_string());
        assert!(
            output.status.success(),
            "{}: {}",
            vector["name"],
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        let parsed_source: Value = serde_json::from_str(&source_color.to_string()).unwrap();
        assert_eq!(
            result["colors"]["focus"], parsed_source,
            "{}",
            vector["name"]
        );
        let actual = &result["colorFallbacks"]["focus"];
        let expected = &vector["expected"];
        assert_eq!(actual["colorSpace"], expected["colorSpace"]);
        assert_eq!(actual["alpha"], expected["alpha"]);
        for index in 0..3 {
            assert!(
                (actual["components"][index].as_f64().unwrap()
                    - expected["components"][index].as_f64().unwrap())
                .abs()
                    < 1e-12,
                "{}: channel {index}",
                vector["name"]
            );
        }
    }
}

#[test]
fn numeric_lab_spaces_resolve_without_authored_hex() {
    let vectors: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/color/srgb-fallback-vectors.json"
    ))
    .unwrap();
    let cases: Vec<&Value> = vectors
        .iter()
        .filter(|vector| {
            matches!(vector["value"]["colorSpace"].as_str(), Some("lab" | "lch"))
                && vector.get("expected").is_some()
                && vector["value"].get("hex").is_none()
        })
        .collect();
    assert_eq!(cases.len(), 3);
    for vector in cases {
        let source_color = &vector["value"];
        let mut request: Value = serde_json::from_str(SOURCE).unwrap();
        request["tokens"]["palette"]["lab"] = json!({"$value": source_color});
        request["colorAssignments"]["roles"]["focus"] = json!("palette.lab");
        let output = run_stdin(&request.to_string());
        assert!(
            output.status.success(),
            "{}: {}",
            vector["name"],
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            result["colors"]["focus"], *source_color,
            "{}",
            vector["name"]
        );
        let actual = &result["colorFallbacks"]["focus"];
        let expected = &vector["expected"];
        assert_eq!(actual["colorSpace"], expected["colorSpace"]);
        assert_eq!(actual["alpha"], expected["alpha"]);
        for index in 0..3 {
            assert!(
                (actual["components"][index].as_f64().unwrap()
                    - expected["components"][index].as_f64().unwrap())
                .abs()
                    < 1e-12,
                "{}: channel {index}",
                vector["name"]
            );
        }
    }
}

#[test]
fn usage_error_has_distinct_exit_status() {
    let output = Command::new(env!("CARGO_BIN_EXE_resina-headless"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
}
