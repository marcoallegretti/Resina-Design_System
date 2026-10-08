use resina_resolver::{resolve_command_motion_source, resolve_command_paint_source};
use serde_json::{Value, json};
fn request() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap()
}
fn resolve(value: &Value) -> resina_resolver::CommandMotionIr {
    resolve_command_motion_source(&value.to_string()).unwrap()
}
#[test]
fn public_motion_cases_are_valid_or_diagnostic() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-motion-cases.json"
    ))
    .unwrap();
    for case in cases {
        let mut input = request();
        for change in case["requestChanges"].as_array().unwrap() {
            *input.pointer_mut(change["path"].as_str().unwrap()).unwrap() = change["value"].clone();
        }
        let result = resolve_command_motion_source(&input.to_string());
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
    assert_eq!(before.command().response().depth_scale(), 1.0);
    assert_eq!(before.command().response().body_mix(), 1.0);
    assert_eq!(before.depth_scale().state().position(), 1.5);
    assert_eq!(before.body_mix().state().velocity(), -3.0);
    input["surface"]["body"]["surface"]["states"]["states"] = json!(["rest", "focused"]);
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
        let endpoint = resolve_command_paint_source(&endpoint.to_string()).unwrap();
        assert_eq!(motion.command(), &endpoint);
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
        let paint = serde_json::to_value(motion.command().paint()).unwrap();
        assert_eq!(
            paint["body"]["geometry"]["silhouette"],
            paint["focus"]["geometry"]["silhouette"]
        );
        assert!(paint["body"]["contentContrastRatio"].as_f64().unwrap() >= 4.5);
    }
    input["time"] = json!(0);
    input["channels"]["bodyMix"]["initial"] = json!({"position":-0.9,"velocity":0});
    assert!(
        resolve_command_motion_source(&input.to_string())
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
    assert!(resolve_command_motion_source(&duplicate).is_err());
    let overflow = source.replacen("\"time\":0.25", "\"time\":1e400", 1);
    assert!(resolve_command_motion_source(&overflow).is_err());
    for field in ["channels", "time", "commandAppearance"] {
        let mut missing = input.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(resolve_command_motion_source(&missing.to_string()).is_err());
    }
}

#[test]
fn scope_sampling_and_paint_failures_keep_their_order() {
    use resina_resolver::CommandPaintError;

    let mut input = request();
    input["time"] = json!(-1);
    input["surface"]["body"]["minimumContentContrast"] = json!(21);
    input["surface"]["body"]["surface"]["materialRole"] = json!("surface.base");
    assert!(matches!(
        resolve_command_motion_source(&input.to_string()),
        Err(CommandPaintError::Scope(_))
    ));
    input["surface"]["body"]["surface"]["materialRole"] = json!("control.interactive");
    assert!(matches!(
        resolve_command_motion_source(&input.to_string()),
        Err(CommandPaintError::Motion(
            resina_motion::SpringError::InvalidTime
        ))
    ));
    input["time"] = json!(0.25);
    assert!(matches!(
        resolve_command_motion_source(&input.to_string()),
        Err(CommandPaintError::Paint(_))
    ));
}

#[test]
fn motion_cli_has_atomic_utf8_and_bounded_protocol() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let program = env!("CARGO_BIN_EXE_resina-command-motion");
    let source = include_bytes!("../../../../conformance/ir/command-motion-request.json");
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../conformance/ir/command-motion-request.json"
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
        CommandMotionInput, CommandPaintInput, OpaqueSurfaceInput, SurfacePaintInput,
        compile_theme_source, resolve_command_motion, resolve_srgb_fallback,
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
    let channels = serde_json::from_value(request["channels"].clone()).unwrap();
    let typed = resolve_command_motion(
        &theme,
        &environment,
        CommandMotionInput {
            command: CommandPaintInput {
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
            channels: &channels,
            time: request["time"].as_f64().unwrap(),
        },
    )
    .unwrap();
    assert_eq!(
        typed,
        resolve_command_motion_source(&request.to_string()).unwrap()
    );
}
