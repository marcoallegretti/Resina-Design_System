use resina_color::{OpaqueSrgbRange, resolve_srgb_fallback};
use resina_resolver::{
    SliderPartPaintInput, compile_theme_source, resolve_slider_part_paint,
    resolve_slider_part_paint_source,
};
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn request() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/ir/slider-part-paint-request.json"
    ))
    .unwrap()
}

fn ranges(value: &Value) -> Vec<OpaqueSrgbRange> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|range| {
            OpaqueSrgbRange::try_new(
                resolve_srgb_fallback(&range["lower"]).unwrap(),
                resolve_srgb_fallback(&range["upper"]).unwrap(),
            )
            .unwrap()
        })
        .collect()
}

fn run(source: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resina-slider-part-paint"))
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(source).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn source_matches_complete_typed_ir_across_public_phases_parts_and_families() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-phase-cases.json"
    ))
    .unwrap();
    for family in ["cast", "frost", "elastomer"] {
        for part in ["track", "thumb"] {
            for case in &cases {
                let mut request = request();
                let mut theme: Value =
                    serde_json::from_str(request["theme"]["themeSource"].as_str().unwrap())
                        .unwrap();
                theme["materialAssignments"]["control"]["interactive"] = json!(family);
                request["theme"]["themeSource"] = json!(theme.to_string());
                request["part"] = json!(part);
                request["readOnly"] = case["readOnly"].clone();
                request["surface"]["states"] = case["states"].clone();
                let compiled = compile_theme_source(&theme.to_string()).unwrap();
                let environment =
                    serde_json::from_value(request["theme"]["environment"].clone()).unwrap();
                let surface = serde_json::from_value(request["surface"].clone()).unwrap();
                let appearance = serde_json::from_value(request["appearance"].clone()).unwrap();
                let interaction =
                    serde_json::from_value(request["interactionAppearance"].clone()).unwrap();
                let backdrop = resolve_srgb_fallback(&request["postTreatmentBackdrop"]).unwrap();
                let adjacent = ranges(&request["adjacentRanges"]);
                let surrounding = ranges(&request["surroundingRanges"]);
                let typed = resolve_slider_part_paint(
                    &compiled,
                    &environment,
                    SliderPartPaintInput {
                        part: serde_json::from_value(json!(part)).unwrap(),
                        read_only: case["readOnly"].as_bool().unwrap(),
                        surface: &surface,
                        size: serde_json::from_value(request["size"].clone()).unwrap(),
                        appearance: &appearance,
                        interaction_appearance: &interaction,
                        foreground_role: serde_json::from_value(request["foregroundRole"].clone())
                            .unwrap(),
                        post_treatment_backdrop: Some(&backdrop),
                        adjacent_ranges: &adjacent,
                        surrounding_ranges: Some(&surrounding),
                        minimum_content_contrast: 1.0,
                        minimum_edge_contrast: 3.0,
                    },
                );
                let source = resolve_slider_part_paint_source(&request.to_string());
                if let Some(expected) = case.get("expected") {
                    assert_eq!(
                        source.as_ref().unwrap(),
                        typed.as_ref().unwrap(),
                        "{} {family} {part}",
                        case["name"]
                    );
                    assert_eq!(
                        serde_json::to_value(source.unwrap().phase()).unwrap(),
                        *expected
                    );
                } else {
                    let diagnostic = case["error"].as_str().unwrap();
                    assert!(source.unwrap_err().to_string().contains(diagnostic));
                    assert!(typed.unwrap_err().to_string().contains(diagnostic));
                }
            }
        }
    }
}

#[test]
fn public_cases_preserve_capability_fallback_and_atomic_failure() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/ir/slider-part-paint-cases.json"
    ))
    .unwrap();
    for case in cases {
        let mut request = request();
        for change in case["requestChanges"].as_array().unwrap() {
            *request
                .pointer_mut(change["path"].as_str().unwrap())
                .unwrap() = change["value"].clone();
        }
        let source = request.to_string();
        let result = resolve_slider_part_paint_source(&source);
        let output = run(source.as_bytes());
        if let Some(phase) = case.get("expectedPhase") {
            let result = result.unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
            assert_eq!(serde_json::to_value(result.phase()).unwrap(), *phase);
            assert_eq!(output.status.code(), Some(0));
            assert!(output.stderr.is_empty());
            assert_eq!(
                serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                serde_json::to_value(result).unwrap()
            );
        } else {
            assert!(result.is_err(), "{}", case["name"]);
            assert_eq!(output.status.code(), Some(1), "{}", case["name"]);
            assert!(output.stdout.is_empty());
            assert!(!output.stderr.is_empty());
        }
    }
}

#[test]
fn named_records_reject_positional_null_unknown_and_missing_members() {
    let baseline = request();
    for pointer in [
        "",
        "/theme",
        "/surface",
        "/size",
        "/appearance",
        "/appearance/keyLight",
        "/interactionAppearance",
        "/adjacentRanges/0",
        "/adjacentRanges/0/lower",
    ] {
        let record = baseline.pointer(pointer).unwrap().as_object().unwrap();
        let mut extra = record.clone();
        extra.insert("renderer".into(), json!("native"));
        for invalid in [
            json!(record.values().collect::<Vec<_>>()),
            json!([]),
            json!(null),
            json!(extra),
        ] {
            let mut changed = baseline.clone();
            *changed.pointer_mut(pointer).unwrap() = invalid;
            assert!(
                resolve_slider_part_paint_source(&changed.to_string()).is_err(),
                "{pointer}"
            );
        }
    }
    for field in baseline.as_object().unwrap().keys() {
        if matches!(
            field.as_str(),
            "postTreatmentBackdrop" | "surroundingRanges"
        ) {
            continue;
        }
        let mut changed = baseline.clone();
        changed.as_object_mut().unwrap().remove(field);
        assert!(
            resolve_slider_part_paint_source(&changed.to_string()).is_err(),
            "{field}"
        );
    }
}

#[test]
fn escaped_names_preserve_ir_and_duplicates_fail_before_mapping() {
    let source = request().to_string();
    let expected = resolve_slider_part_paint_source(&source).unwrap();
    for (name, escaped) in [
        ("part", "\\u0070art"),
        ("lower", "\\u006cower"),
        ("alpha", "\\u0061lpha"),
    ] {
        let changed = source.replace(&format!("\"{name}\""), &format!("\"{escaped}\""));
        assert_ne!(changed, source);
        assert_eq!(
            resolve_slider_part_paint_source(&changed).unwrap(),
            expected
        );
    }
    for (member, escaped) in [
        ("\"part\":\"track\"", "\"\\u0070art\":\"track\""),
        ("\"alpha\":1", "\"\\u0061lpha\":1"),
    ] {
        assert!(source.contains(member));
        let duplicate = source.replacen(member, &format!("{member},{escaped}"), 1);
        assert!(
            resolve_slider_part_paint_source(&duplicate)
                .unwrap_err()
                .to_string()
                .contains("duplicate JSON member")
        );
    }
}

#[test]
fn optional_ranges_are_actual_inputs_even_when_the_track_does_not_draw_focus() {
    let mut request = request();
    request
        .as_object_mut()
        .unwrap()
        .remove("postTreatmentBackdrop");
    request.as_object_mut().unwrap().remove("surroundingRanges");
    request["surface"]["states"]["states"] = json!(["rest", "focused"]);
    assert!(
        resolve_slider_part_paint_source(&request.to_string())
            .unwrap()
            .paint()
            .focus()
            .is_none()
    );
    request["part"] = json!("thumb");
    assert!(resolve_slider_part_paint_source(&request.to_string()).is_err());
    request["part"] = json!("track");
    for invalid in [json!(null), json!([])] {
        request["surroundingRanges"] = invalid;
        assert!(resolve_slider_part_paint_source(&request.to_string()).is_err());
    }
}

#[test]
fn command_enforces_transport_limit_utf8_and_usage_without_partial_results() {
    let source = request().to_string();
    let stdin = run(source.as_bytes());
    let path = std::env::temp_dir().join(format!(
        "resina-slider-part-paint-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, &source).unwrap();
    let file = Command::new(env!("CARGO_BIN_EXE_resina-slider-part-paint"))
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(file.status.code(), Some(0));
    assert_eq!(file.stdout, stdin.stdout);
    assert!(file.stderr.is_empty());
    let mut limit = source.into_bytes();
    limit.resize(1024 * 1024, b' ');
    assert_eq!(run(&limit).status.code(), Some(0));
    limit.push(b' ');
    for invalid in [limit, vec![0xff], b"{}{}".to_vec()] {
        let output = run(&invalid);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    for args in [vec![], vec!["-", "extra"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_resina-slider-part-paint"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}
