use resina_resolver::{resolve_toggle_part_motion_source, resolve_toggle_part_paint_source};
use serde_json::{Value, json};
fn request() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/ir/toggle-part-motion-request.json"
    ))
    .unwrap()
}
fn resolve(value: &Value) -> resina_resolver::TogglePartMotionIr {
    resolve_toggle_part_motion_source(&value.to_string()).unwrap()
}
#[test]
fn public_motion_cases_are_valid_or_diagnostic() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/ir/toggle-part-motion-cases.json"
    ))
    .unwrap();
    for case in cases {
        let mut input = request();
        for change in case["requestChanges"].as_array().unwrap() {
            *input.pointer_mut(change["path"].as_str().unwrap()).unwrap() = change["value"].clone();
        }
        let result = resolve_toggle_part_motion_source(&input.to_string());
        if let Some(policy) = case.get("expectedPolicy") {
            assert_eq!(
                serde_json::to_value(result.unwrap().policy()).unwrap(),
                *policy,
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
fn retargeting_uses_unprojected_state_and_preserves_both_velocities() {
    let mut input = request();
    input["time"] = json!(0);
    input["channels"]["depthScale"]["initial"] = json!({"position":1.5,"velocity":2});
    input["channels"]["bodyMix"]["initial"] = json!({"position":1.5,"velocity":-3});
    let before = resolve(&input);
    assert_eq!(before.part_paint().response().depth_scale(), 1.0);
    assert_eq!(before.part_paint().response().body_mix(), 1.0);
    assert_eq!(before.depth_scale().state().position(), 1.5);
    assert_eq!(before.body_mix().state().velocity(), -3.0);
    input["surface"]["body"]["surface"]["states"]["states"] = json!(["rest", "focused", "checked"]);
    input["channels"]["depthScale"]["initial"] =
        serde_json::to_value(before.depth_scale().state()).unwrap();
    input["channels"]["bodyMix"]["initial"] =
        serde_json::to_value(before.body_mix().state()).unwrap();
    let after = resolve(&input);
    assert_eq!(after.depth_scale().state(), before.depth_scale().state());
    assert_eq!(after.body_mix().state(), before.body_mix().state());
    assert_eq!(after.target().depth_scale(), 1.0);
    assert_eq!(after.target().body_mix(), 0.0);
    assert!(!after.body_mix().settled());
    assert!(!after.depth_scale().settled());
}
#[test]
fn reduced_motion_and_settled_frames_match_static_endpoints() {
    for reduced in [false, true] {
        let mut input = request();
        input["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]["reducedMotion"] =
            json!(reduced);
        input["time"] = json!(if reduced { 0.0 } else { 100.0 });
        let motion = resolve(&input);
        let mut endpoint = input;
        endpoint.as_object_mut().unwrap().remove("channels");
        endpoint.as_object_mut().unwrap().remove("time");
        let endpoint = resolve_toggle_part_paint_source(&endpoint.to_string()).unwrap();
        assert_eq!(motion.part_paint(), &endpoint);
        assert!(motion.body_mix().settled());
        assert!(motion.depth_scale().settled());
        assert_eq!(motion.body_mix().state().velocity(), 0.0);
        assert_eq!(motion.depth_scale().state().velocity(), 0.0);
    }
}
#[test]
fn every_sample_rechecks_contrast_and_navigation_matches_sample_geometry() {
    let mut input = request();
    for time in [0.0, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 100.0] {
        input["time"] = json!(time);
        let motion = resolve(&input);
        let paint = serde_json::to_value(motion.part_paint().paint()).unwrap();
        assert_eq!(
            paint["body"]["geometry"]["silhouette"],
            paint["focus"]["geometry"]["silhouette"]
        );
        assert!(paint["body"]["contentContrastRatio"].as_f64().unwrap() >= 4.5);
    }
    input["time"] = json!(0);
    input["channels"]["bodyMix"]["initial"] = json!({"position":-0.9,"velocity":0});
    assert!(
        resolve_toggle_part_motion_source(&input.to_string())
            .unwrap_err()
            .to_string()
            .contains("contrast")
    );
}
#[test]
fn strict_source_rejects_duplicate_missing_and_nonfinite_motion_inputs() {
    let input = request();
    let source = input.to_string();
    let duplicate = source.replacen("\"position\":0", "\"position\":0,\"position\":1", 1);
    assert_ne!(duplicate, source);
    assert!(resolve_toggle_part_motion_source(&duplicate).is_err());
    let overflow = source.replacen("\"time\":0.25", "\"time\":1e400", 1);
    assert!(resolve_toggle_part_motion_source(&overflow).is_err());
    for field in ["channels", "time", "interactionAppearance"] {
        let mut missing = input.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(resolve_toggle_part_motion_source(&missing.to_string()).is_err());
    }
}

#[test]
fn scope_sampling_and_paint_failures_keep_their_order() {
    use resina_resolver::TogglePartPaintError;

    let mut input = request();
    input["time"] = json!(-1);
    input["surface"]["body"]["minimumContentContrast"] = json!(21);
    input["surface"]["body"]["surface"]["materialRole"] = json!("surface.base");
    assert!(matches!(
        resolve_toggle_part_motion_source(&input.to_string()),
        Err(TogglePartPaintError::Scope(_))
    ));
    input["surface"]["body"]["surface"]["materialRole"] = json!("control.interactive");
    assert!(matches!(
        resolve_toggle_part_motion_source(&input.to_string()),
        Err(TogglePartPaintError::Motion(
            resina_motion::SpringError::InvalidTime
        ))
    ));
    input["time"] = json!(0.25);
    assert!(matches!(
        resolve_toggle_part_motion_source(&input.to_string()),
        Err(TogglePartPaintError::Paint(_))
    ));
}

#[test]
fn motion_cli_has_atomic_utf8_and_bounded_protocol() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let program = env!("CARGO_BIN_EXE_resina-toggle-part-motion");
    let source = include_bytes!("../../../../conformance/ir/toggle-part-motion-request.json");
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../conformance/ir/toggle-part-motion-request.json"
    );
    let file = Command::new(program).arg(path).output().unwrap();
    assert!(file.status.success());
    for bytes in [
        source.to_vec(),
        vec![0xff],
        vec![b' '; 1024 * 1024 + 1],
        b"{".to_vec(),
    ] {
        let mut child = Command::new(program)
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&bytes).unwrap();
        let result = child.wait_with_output().unwrap();
        if bytes == source {
            assert_eq!(result.stdout, file.stdout);
            assert!(result.stderr.is_empty());
            assert!(result.status.success());
        } else {
            assert_eq!(result.status.code(), Some(1));
            assert!(result.stdout.is_empty());
            assert!(!result.stderr.is_empty());
        }
    }
    assert_eq!(
        Command::new(program).output().unwrap().status.code(),
        Some(2)
    );
    assert_eq!(
        Command::new(program)
            .args(["-", "extra"])
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
    assert!(
        Command::new(program)
            .arg("--help")
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn typed_motion_matches_source_with_actual_environment() {
    use resina_resolver::{
        OpaqueSurfaceInput, SurfacePaintInput, TogglePartMotionInput, TogglePartPaintInput,
        compile_theme_source, resolve_srgb_fallback, resolve_toggle_part_motion,
    };
    let request = request();
    let body = &request["surface"]["body"];
    let theme = compile_theme_source(body["theme"]["themeSource"].as_str().unwrap()).unwrap();
    let environment = serde_json::from_value(body["theme"]["environment"].clone()).unwrap();
    let surface = serde_json::from_value(body["surface"].clone()).unwrap();
    let appearance = serde_json::from_value(body["appearance"].clone()).unwrap();
    let interaction_appearance =
        serde_json::from_value(request["interactionAppearance"].clone()).unwrap();
    let adjacent = resolve_srgb_fallback(&body["adjacentColor"]).unwrap();
    let surrounding = resolve_srgb_fallback(&request["surface"]["surroundingColor"]).unwrap();
    let channels = serde_json::from_value(request["channels"].clone()).unwrap();
    let backdrop = resolve_srgb_fallback(&body["postTreatmentBackdrop"]).unwrap();
    let typed = resolve_toggle_part_motion(
        &theme,
        &environment,
        TogglePartMotionInput {
            part: TogglePartPaintInput {
                surface: SurfacePaintInput {
                    body: OpaqueSurfaceInput {
                        surface: &surface,
                        size: serde_json::from_value(body["size"].clone()).unwrap(),
                        appearance: &appearance,
                        foreground_role: serde_json::from_value(body["foregroundRole"].clone())
                            .unwrap(),
                        post_treatment_backdrop: Some(&backdrop),
                        adjacent_color: &adjacent,
                        minimum_content_contrast: body["minimumContentContrast"].as_f64().unwrap(),
                        minimum_edge_contrast: body["minimumEdgeContrast"].as_f64().unwrap(),
                    },
                    surrounding_color: Some(&surrounding),
                },
                interaction_appearance: &interaction_appearance,
                part: serde_json::from_value(request["part"].clone()).unwrap(),
                checked_color_role: serde_json::from_value(request["checkedColorRole"].clone())
                    .unwrap(),
            },
            channels: &channels,
            time: request["time"].as_f64().unwrap(),
        },
    )
    .unwrap();
    assert_eq!(
        typed,
        resolve_toggle_part_motion_source(&request.to_string()).unwrap()
    );
}

#[test]
fn every_supported_state_set_keeps_selection_and_track_only_navigation_through_motion() {
    use resina_resolver::CommandMotionPolicy;
    let states = ["rest", "hover", "pressed", "checked", "disabled", "focused"];
    for family in ["cast", "frost", "elastomer"] {
        for part in ["track", "thumb"] {
            for mask in 1..(1 << states.len()) {
                let mut input = request();
                input["part"] = json!(part);
                let selected: Vec<_> = states
                    .iter()
                    .enumerate()
                    .filter(|(bit, _)| mask & (1 << bit) != 0)
                    .map(|(_, state)| *state)
                    .collect();
                input["surface"]["body"]["surface"]["states"]["states"] = json!(selected);
                let mut theme: Value = serde_json::from_str(
                    input["surface"]["body"]["theme"]["themeSource"]
                        .as_str()
                        .unwrap(),
                )
                .unwrap();
                theme["materialAssignments"]["control"]["interactive"] = json!(family);
                input["surface"]["body"]["theme"]["themeSource"] = json!(theme.to_string());
                let motion = resolve(&input);
                assert_eq!(
                    motion.policy(),
                    if family == "cast" {
                        CommandMotionPolicy::CastImmediate
                    } else {
                        CommandMotionPolicy::Spring
                    }
                );
                assert_eq!(motion.part_paint().checked(), selected.contains(&"checked"));
                assert_eq!(
                    motion.part_paint().paint().focus().is_some(),
                    part == "track" && selected.contains(&"focused")
                );
                let expected_states: resina_model::StateSet =
                    serde_json::from_value(input["surface"]["body"]["surface"]["states"].clone())
                        .unwrap();
                assert_eq!(
                    motion.part_paint().paint().body().states(),
                    &expected_states
                );
                let phase = ["disabled", "pressed", "hover"]
                    .into_iter()
                    .find(|state| selected.contains(state))
                    .unwrap_or("rest");
                assert_eq!(
                    serde_json::to_value(motion.part_paint().phase()).unwrap(),
                    phase
                );
                let expected_target = if phase == "rest" {
                    json!({"bodyMix":0,"depthScale":1})
                } else {
                    input["interactionAppearance"]["profiles"][family][phase].clone()
                };
                let expected_target: resina_model::CommandResponse =
                    serde_json::from_value(expected_target).unwrap();
                assert_eq!(motion.target(), expected_target);
                input["time"] = json!(100);
                let settled = resolve(&input);
                let mut endpoint = input;
                endpoint.as_object_mut().unwrap().remove("channels");
                endpoint.as_object_mut().unwrap().remove("time");
                assert_eq!(
                    settled.part_paint(),
                    &resolve_toggle_part_paint_source(&endpoint.to_string()).unwrap()
                );
            }
        }
    }
}

#[test]
fn sampled_frost_keeps_actual_capabilities_and_rechecks_legibility_fallback() {
    let mut r = request();
    let mut theme: Value = serde_json::from_str(
        r["surface"]["body"]["theme"]["themeSource"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    theme["materialAssignments"]["control"]["interactive"] = json!("frost");
    r["surface"]["body"]["theme"]["themeSource"] = json!(theme.to_string());
    let env = &mut r["surface"]["body"]["theme"]["environment"];
    env["accessibilityPreferences"]["highContrast"] = json!(false);
    env["accessibilityPreferences"]["reducedTransparency"] = json!(false);
    env["rendererCapabilities"]["translucentSurfaces"] = json!(true);
    r["surface"]["body"]["postTreatmentBackdrop"] =
        json!({"colorSpace":"srgb","components":[0,0,0],"alpha":1});
    r["surface"]["body"]["surface"]["states"]["states"] = json!(["focused", "pressed", "checked"]);
    r["surface"]["body"]["minimumContentContrast"] = json!(1);
    assert!(
        resolve_toggle_part_motion_source(&r.to_string())
            .unwrap_err()
            .to_string()
            .contains("resolved opaque body")
    );
    r["surface"]["body"]["minimumContentContrast"] = json!(4.5);
    let ir = resolve_toggle_part_motion_source(&r.to_string()).unwrap();
    assert!(ir.part_paint().paint().body().content_fallback_applied());
    assert!(ir.part_paint().paint().body().content_contrast_ratio() >= 4.5);
    assert!(ir.part_paint().checked());
    assert!(ir.part_paint().paint().focus().is_some());
    assert_eq!(
        serde_json::to_value(ir.part_paint().paint().body()).unwrap()["frostRepresentation"],
        "opaqueDimensional"
    );
    assert_eq!(
        r["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]["reducedTransparency"],
        false
    );
}
