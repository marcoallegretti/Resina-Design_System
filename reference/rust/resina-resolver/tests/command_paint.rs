use resina_resolver::{
    opaque_contrast_ratio, resolve_command_paint_source, resolve_surface_paint_source,
};
use serde_json::{Value, json};

fn request() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-paint-request.json"
    ))
    .unwrap()
}

#[test]
fn public_command_cases_keep_failures_diagnostic() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-paint-cases.json"
    ))
    .unwrap();
    for case in cases {
        let mut request = request();
        for change in case["requestChanges"].as_array().unwrap() {
            *request
                .pointer_mut(change["path"].as_str().unwrap())
                .unwrap() = change["value"].clone();
        }
        let result = resolve_command_paint_source(&request.to_string());
        if let Some(expected) = case.get("expectedPhase") {
            assert_eq!(
                serde_json::to_value(result.unwrap().phase()).unwrap(),
                *expected,
                "{}",
                case["name"]
            );
        } else {
            let error = result.unwrap_err().to_string();
            assert!(
                error.contains(case["errorContains"].as_str().unwrap()),
                "{}: {error}",
                case["name"]
            );
        }
    }
}

#[test]
fn every_command_state_combination_preserves_signals_and_matches_focus_geometry() {
    let state_names = ["rest", "hover", "focused", "pressed", "disabled"];
    for family in ["cast", "frost", "elastomer"] {
        for bits in 1..32 {
            let states: Vec<_> = state_names
                .iter()
                .enumerate()
                .filter(|(i, _)| bits & (1 << i) != 0)
                .map(|(_, state)| *state)
                .collect();
            let mut request = request();
            let mut theme: Value = serde_json::from_str(
                request["surface"]["body"]["theme"]["themeSource"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
            theme["materialAssignments"]["control"]["interactive"] = json!(family);
            request["surface"]["body"]["theme"]["themeSource"] = json!(theme.to_string());
            request["surface"]["body"]["surface"]["states"]["states"] = json!(states);
            if family == "frost" {
                request["surface"]["body"]["postTreatmentBackdrop"] =
                    json!({"colorSpace":"srgb","components":[1,1,1],"alpha":1});
            }
            let result = resolve_command_paint_source(&request.to_string()).unwrap();
            let actual = serde_json::to_value(&result).unwrap();
            assert_eq!(actual["paint"]["body"]["states"]["states"], json!(states));
            let phase = ["disabled", "pressed", "hover"]
                .into_iter()
                .find(|s| states.contains(s))
                .unwrap_or("rest");
            assert_eq!(actual["phase"], phase);
            let (mix, scale) = if phase == "rest" {
                (0.0, 1.0)
            } else {
                let response = &request["commandAppearance"]["profiles"][family][phase];
                (
                    response["bodyMix"].as_f64().unwrap(),
                    response["depthScale"].as_f64().unwrap(),
                )
            };
            let expected = [0.8, 0.7, 0.6].map(|c| {
                if mix < 0.0 {
                    c * (1.0 + mix)
                } else {
                    c + (1.0 - c) * mix
                }
            });
            assert_eq!(
                result.paint().body().pigment().body().components(),
                expected
            );
            let offset = result.paint().body().lighting().side_offset();
            assert!((offset.x.hypot(offset.y) - 2.0 * scale).abs() < 1e-12);
            assert_eq!(
                result.paint().body().content_contrast_ratio(),
                opaque_contrast_ratio(
                    result.paint().body().foreground(),
                    result.paint().body().pigment().body()
                )
                .unwrap()
            );
            if states.contains(&"focused") {
                let focus = result.paint().focus().unwrap();
                assert_eq!(
                    result.paint().body().geometry().silhouette(),
                    focus.geometry().silhouette()
                );
                assert_eq!(
                    actual["paint"]["body"]["states"],
                    actual["paint"]["focus"]["indicator"]["binding"]["states"]
                );
            } else {
                assert!(result.paint().focus().is_none());
            }
            if phase == "rest" {
                assert_eq!(
                    result.paint(),
                    &resolve_surface_paint_source(&request["surface"].to_string()).unwrap()
                );
            }
        }
    }
}

#[test]
fn frost_uses_actual_capabilities_and_state_adjusted_legibility_fallback() {
    let mut request = request();
    let mut theme: Value = serde_json::from_str(
        request["surface"]["body"]["theme"]["themeSource"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    theme["materialAssignments"]["control"]["interactive"] = json!("frost");
    request["surface"]["body"]["theme"]["themeSource"] = json!(theme.to_string());
    let environment = &mut request["surface"]["body"]["theme"]["environment"];
    environment["accessibilityPreferences"]["highContrast"] = json!(false);
    environment["accessibilityPreferences"]["reducedTransparency"] = json!(false);
    environment["rendererCapabilities"]["translucentSurfaces"] = json!(true);
    request["surface"]["body"]["postTreatmentBackdrop"] =
        json!({"colorSpace":"srgb","components":[0,0,0],"alpha":1});
    request["surface"]["body"]["surface"]["states"]["states"] = json!(["pressed", "focused"]);
    request["surface"]["body"]["minimumContentContrast"] = json!(1);
    let error = resolve_command_paint_source(&request.to_string())
        .unwrap_err()
        .to_string();
    assert!(error.contains("resolved opaque body"), "{error}");
    request["surface"]["body"]["minimumContentContrast"] = json!(7);
    let result = resolve_command_paint_source(&request.to_string()).unwrap();
    assert!(result.paint().body().content_fallback_applied());
    assert_eq!(
        serde_json::to_value(result.paint().body()).unwrap()["frostRepresentation"],
        "opaqueDimensional"
    );
    assert!(result.paint().body().content_contrast_ratio() >= 7.0);
}

#[test]
fn source_rejects_duplicate_profiles_and_cli_publishes_no_partial_paint() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let source = request().to_string();
    let duplicate = source.replacen(
        "\"bodyMix\":-0.035",
        "\"bodyMix\":-0.035,\"bodyMix\":-0.035",
        1,
    );
    assert_ne!(source, duplicate);
    assert!(
        resolve_command_paint_source(&duplicate)
            .unwrap_err()
            .to_string()
            .contains("duplicate JSON member")
    );
    for source in [duplicate.into_bytes(), vec![0xff]] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_resina-command-paint"))
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&source).unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn typed_and_source_boundaries_share_the_actual_environment() {
    use resina_resolver::{
        CommandPaintInput, OpaqueSurfaceInput, SurfacePaintInput, compile_theme_source,
        resolve_command_paint, resolve_srgb_fallback,
    };
    let request = request();
    let body = &request["surface"]["body"];
    let theme = compile_theme_source(body["theme"]["themeSource"].as_str().unwrap()).unwrap();
    let environment = serde_json::from_value(body["theme"]["environment"].clone()).unwrap();
    let surface = serde_json::from_value(body["surface"].clone()).unwrap();
    let appearance = serde_json::from_value(body["appearance"].clone()).unwrap();
    let command_appearance = serde_json::from_value(request["commandAppearance"].clone()).unwrap();
    let adjacent = resolve_srgb_fallback(&body["adjacentColor"]).unwrap();
    let surrounding = resolve_srgb_fallback(&request["surface"]["surroundingColor"]).unwrap();
    let typed = resolve_command_paint(
        &theme,
        &environment,
        CommandPaintInput {
            surface: SurfacePaintInput {
                body: OpaqueSurfaceInput {
                    surface: &surface,
                    size: serde_json::from_value(body["size"].clone()).unwrap(),
                    appearance: &appearance,
                    foreground_role: serde_json::from_value(body["foregroundRole"].clone())
                        .unwrap(),
                    post_treatment_backdrop: None,
                    adjacent_color: &adjacent,
                    minimum_content_contrast: body["minimumContentContrast"].as_f64().unwrap(),
                    minimum_edge_contrast: body["minimumEdgeContrast"].as_f64().unwrap(),
                },
                surrounding_color: Some(&surrounding),
            },
            command_appearance: &command_appearance,
        },
    )
    .unwrap();
    assert_eq!(
        typed,
        resolve_command_paint_source(&request.to_string()).unwrap()
    );
}
