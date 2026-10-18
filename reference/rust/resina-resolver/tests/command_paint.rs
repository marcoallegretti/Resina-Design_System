#[path = "support/diagnostics.rs"]
mod diagnostics;

use resina_model::CommandPhase;
use resina_resolver::{
    CommandPaintError, ImperceptibleResponse, SrgbFallback, opaque_contrast_ratio,
    resolve_command_paint_source, resolve_surface_paint_source,
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
                error.contains(&diagnostics::expected("command_paint", &case["name"])),
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
        "\"bodyMix\":-0.075",
        "\"bodyMix\":-0.075,\"bodyMix\":-0.075",
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

#[test]
fn selected_responses_must_change_body_color_or_depth_perceivably() {
    let resolve = |elevation: &str, phase: &str, response: Value| {
        let mut request = request();
        request["surface"]["body"]["surface"]["form"]["elevation"] = json!(elevation);
        request["surface"]["body"]["surface"]["states"]["states"] = json!([phase, "focused"]);
        request["commandAppearance"]["profiles"]["elastomer"][phase] = response;
        resolve_command_paint_source(&request.to_string())
    };
    for (elevation, phase, response) in [
        ("raised", "hover", json!({"bodyMix": 0, "depthScale": 1.5})),
        (
            "raised",
            "pressed",
            json!({"bodyMix": 0, "depthScale": 0.5}),
        ),
        (
            "raised",
            "disabled",
            json!({"bodyMix": -0.1, "depthScale": 1}),
        ),
        (
            "embedded",
            "hover",
            json!({"bodyMix": -0.1, "depthScale": 1}),
        ),
    ] {
        let paint = resolve(elevation, phase, response.clone())
            .unwrap_or_else(|error| panic!("{elevation} {phase} {response}: {error}"));
        assert_eq!(serde_json::to_value(paint.phase()).unwrap(), phase);
    }
    for (elevation, phase, response, depth_change) in [
        (
            "raised",
            "hover",
            json!({"bodyMix": 0.06, "depthScale": 1}),
            0.0,
        ),
        (
            "raised",
            "pressed",
            json!({"bodyMix": 0, "depthScale": 0.51}),
            0.98,
        ),
        (
            "raised",
            "disabled",
            json!({"bodyMix": -0.02, "depthScale": 0.7}),
            0.6,
        ),
        (
            "embedded",
            "hover",
            json!({"bodyMix": 0, "depthScale": 2}),
            0.0,
        ),
    ] {
        let Err(CommandPaintError::Imperceptible(ImperceptibleResponse {
            phase: measured,
            reference: CommandPhase::Rest,
            color_difference,
            depth_change: measured_depth,
        })) = resolve(elevation, phase, response.clone())
        else {
            panic!("{elevation} {phase} {response} must be imperceptible");
        };
        assert_eq!(serde_json::to_value(measured).unwrap(), phase);
        assert!(color_difference < 0.04, "{color_difference}");
        assert!(
            (measured_depth - depth_change).abs() < 1e-12,
            "{measured_depth}"
        );
        assert_ne!(measured, CommandPhase::Rest);
    }
    let error = resolve("raised", "hover", json!({"bodyMix": 0.06, "depthScale": 1}))
        .unwrap_err()
        .to_string();
    assert!(error.contains("imperceptible from rest"), "{error}");
}

#[test]
fn every_authored_response_of_the_bound_family_is_validated_in_any_state() {
    for (state, phase, response) in [
        ("rest", "hover", json!({"bodyMix": 0.06, "depthScale": 1})),
        ("hover", "pressed", json!({"bodyMix": 0, "depthScale": 0.7})),
        (
            "pressed",
            "disabled",
            json!({"bodyMix": -0.02, "depthScale": 0.7}),
        ),
    ] {
        let mut request = request();
        request["surface"]["body"]["surface"]["states"]["states"] = json!([state]);
        request["commandAppearance"]["profiles"]["elastomer"][phase] = response;
        let Err(CommandPaintError::Imperceptible(ImperceptibleResponse {
            phase: measured, ..
        })) = resolve_command_paint_source(&request.to_string())
        else {
            panic!("{state} must reject the imperceptible {phase} response");
        };
        assert_eq!(serde_json::to_value(measured).unwrap(), phase);
        request["commandAppearance"] = self::request()["commandAppearance"].clone();
        resolve_command_paint_source(&request.to_string()).unwrap();
    }
}

#[test]
fn disabled_must_differ_perceivably_from_pressed() {
    for state in ["rest", "pressed", "disabled"] {
        let mut request = request();
        request["surface"]["body"]["surface"]["states"]["states"] = json!([state]);
        let pressed = request["commandAppearance"]["profiles"]["elastomer"]["pressed"].clone();
        request["commandAppearance"]["profiles"]["elastomer"]["disabled"] =
            json!({"bodyMix": pressed["bodyMix"], "depthScale": 0.3});
        let Err(CommandPaintError::Imperceptible(imperceptible)) =
            resolve_command_paint_source(&request.to_string())
        else {
            panic!("{state}: disabled within 1 px of pressed must fail");
        };
        assert_eq!(imperceptible.phase, CommandPhase::Disabled);
        assert_eq!(imperceptible.reference, CommandPhase::Pressed);
        assert_eq!(imperceptible.color_difference, 0.0);
        assert!((imperceptible.depth_change - 0.4).abs() < 1e-12);
        request["commandAppearance"]["profiles"]["elastomer"]["disabled"]["depthScale"] = json!(0);
        resolve_command_paint_source(&request.to_string()).unwrap();
    }
}

fn linear(color: &SrgbFallback) -> [f64; 3] {
    color.components().map(|channel| {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    })
}

#[test]
fn disabled_label_mixes_toward_the_body_down_to_its_bound() {
    for (high_contrast, bound) in [(false, 3.0), (true, 4.5)] {
        let mut request = request();
        request["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]["highContrast"] =
            json!(high_contrast);
        for state in ["rest", "hover", "pressed"] {
            request["surface"]["body"]["surface"]["states"]["states"] = json!([state]);
            let paint = resolve_command_paint_source(&request.to_string()).unwrap();
            assert_eq!(paint.paint().body().foreground().components(), [0.0; 3]);
            assert!(paint.paint().body().content_contrast_ratio() > 4.5);
        }
        request["surface"]["body"]["surface"]["states"]["states"] = json!(["disabled", "focused"]);
        let paint = resolve_command_paint_source(&request.to_string()).unwrap();
        let body = paint.paint().body();
        let ratio = opaque_contrast_ratio(body.foreground(), body.pigment().body()).unwrap();
        assert_eq!(body.content_contrast_ratio(), ratio);
        assert!(
            (bound..bound + 1e-9).contains(&ratio),
            "{high_contrast}: {ratio}"
        );
        let label = linear(body.foreground());
        let background = linear(body.pigment().body());
        let fraction = label[0] / background[0];
        for channel in 0..3 {
            assert!((label[channel] - fraction * background[channel]).abs() < 1e-12);
        }
        request["surface"]["body"]["minimumContentContrast"] = json!(2);
        let paint = resolve_command_paint_source(&request.to_string()).unwrap();
        assert!((bound..bound + 1e-9).contains(&paint.paint().body().content_contrast_ratio()));
    }
}
